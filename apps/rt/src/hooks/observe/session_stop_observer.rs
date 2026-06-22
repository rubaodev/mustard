//! `session_stop_observer` — `Stop` lifecycle observer (W9.T9.2).
//!
//! The harness fires `Stop` when the **main orchestrator** finishes a turn
//! (and on an explicit interrupt). This observer does two things, both
//! fail-open:
//!
//! 1. **Anti-spam marker.** It touches `.claude/.harness/.last-stop` so the
//!    Stop-adjacent double-fire bookkeeping other logic relies on keeps working
//!    (a 5-minute window absorbs a double-fire).
//! 2. **Orchestrator knowledge capture.** It scans the orchestrator's own
//!    final output for `<MEMORY>…</MEMORY>` blocks and persists each as a
//!    [`Knowledge`] via the unified [`KnowledgeStore`]. This is the missing
//!    capture point: light, direct work (`/task`, a small bugfix) runs straight
//!    in the main session — it dispatches no subagent (so
//!    [`super::agent_summary_observer`] never fires) and writes no formal spec
//!    `## Decisions` (so [`super::super::session::session_knowledge_observer`]
//!    finds nothing). Without this, the store stays empty on the most common
//!    workflow.
//!
//! ## Why only main `Stop`, never `SubagentStop`
//!
//! A dispatched subagent's `<MEMORY>` is already captured by
//! `agent_summary_observer` on `PostToolUse(Task)`. This observer is registered
//! ONLY for [`Trigger::Stop`] (the main session), and additionally skips any
//! input the harness marks as a subagent ([`HookInput::is_subagent`]), so the
//! subagent path is never double-captured.
//!
//! ## Scope
//!
//! A captured block is [`Scope::Spec`] when `MUSTARD_ACTIVE_SPEC` resolves
//! (the work belonged to a pipeline), else [`Scope::Global`] — project
//! knowledge not tied to any spec. The store's `is_substantive` gate is the
//! single quality filter: no `<MEMORY>`, an empty block, or a context-echo
//! body persists nothing.
//!
//! ## Fail-open
//!
//! Pure [`Observer`] — never blocks. Every IO step (marker, transcript read,
//! store write) degrades to a no-op on error; no `unwrap`/`expect` outside
//! tests.

use mustard_core::domain::model::contract::{Ctx, HookInput, Observer};
use mustard_core::domain::model::knowledge::{Kind, Knowledge, Origin, Scope, Status};
use mustard_core::io::fs;
use mustard_core::io::knowledge_store::KnowledgeStore;
use mustard_core::time::now_iso8601;
use mustard_core::ClaudePaths;
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use super::memory_block::extract_memory_blocks;

/// Anti-spam window between consecutive Stop fires. Five minutes — long enough
/// to absorb a double-fire, short enough that two distinct interrupts inside the
/// same long-running task each advance the window.
const STOP_ANTISPAM_SECS: u64 = 5 * 60;

/// Authoring role stamped on orchestrator-captured knowledge.
const ORCHESTRATOR_ROLE: &str = "orchestrator";

/// Env var Claude Code uses to point hooks at the active session's transcript.
/// The `Stop` payload also carries it inline as `transcript_path`; either
/// resolves the JSONL conversation log.
const CLAUDE_TRANSCRIPT_PATH_ENV: &str = "CLAUDE_TRANSCRIPT_PATH";

/// The `Stop` lifecycle observer.
pub struct SessionStopObserver;


/// Path to the anti-spam marker file under the project's harness directory.
fn marker_path(cwd: &str) -> PathBuf {
    ClaudePaths::for_project(cwd)
        .map(|p| p.harness_dir().join(".last-stop"))
        .unwrap_or_default()
}

/// `true` when the previous Stop fired less than [`STOP_ANTISPAM_SECS`] ago.
fn recently_stopped(marker: &Path, now: SystemTime) -> bool {
    let Ok(modified) = fs::modified(marker) else {
        return false;
    };
    let Ok(elapsed) = now.duration_since(modified) else {
        // Clock skew → treat as recent (fail closed against spam).
        return true;
    };
    elapsed < Duration::from_secs(STOP_ANTISPAM_SECS)
}

/// Persist the marker file (best-effort; missing dir → create).
fn touch_marker(marker: &Path) {
    if let Some(parent) = marker.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write_atomic(marker, b"");
}

/// Resolve the session transcript JSONL path the `Stop` hook points at.
///
/// Priority: the inline `transcript_path` field of the Stop payload, then the
/// `CLAUDE_TRANSCRIPT_PATH` env var. Both are the absolute path Claude Code
/// wrote; we never reconstruct the `~/.claude/projects/...` layout here (the
/// harness always supplies one on `Stop`). `None` when neither is present.
fn transcript_path(input: &HookInput) -> Option<PathBuf> {
    if let Some(p) = input
        .raw
        .get("transcript_path")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
    {
        return Some(PathBuf::from(p));
    }
    std::env::var(CLAUDE_TRANSCRIPT_PATH_ENV)
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}

/// Read the text of the **last assistant turn** from a Claude Code transcript
/// JSONL — the orchestrator's final output for this Stop.
///
/// Each line is one conversation entry; an assistant turn is
/// `{"type":"assistant","message":{"content":[{"type":"text","text":"…"}]}}`.
/// We walk the lines, keep the text of the latest assistant entry, and return
/// it. Malformed lines are skipped (fail-open). `None` when the file is
/// unreadable or carries no assistant text.
fn last_assistant_text(path: &Path) -> Option<String> {
    let contents = fs::read_to_string(path).ok()?;
    let mut latest: Option<String> = None;
    for line in contents.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        // Accept the v1 `type: assistant` label, or any line carrying an
        // assistant `message.content` (forward-compatible).
        let is_assistant = value.get("type").and_then(Value::as_str) == Some("assistant")
            || value
                .get("message")
                .and_then(|m| m.get("role"))
                .and_then(Value::as_str)
                == Some("assistant");
        if !is_assistant {
            continue;
        }
        if let Some(text) = assistant_text(&value) {
            latest = Some(text);
        }
    }
    latest
}

/// Concatenate the `text` parts of an assistant message's `content` array.
///
/// `content` may be a string (older shape) or an array of typed blocks; only
/// `text` blocks contribute. Returns `None` when no textual content is present.
fn assistant_text(value: &Value) -> Option<String> {
    let content = value.get("message").and_then(|m| m.get("content"))?;
    if let Some(s) = content.as_str() {
        return (!s.trim().is_empty()).then(|| s.to_string());
    }
    let arr = content.as_array()?;
    let mut joined = String::new();
    for block in arr {
        if block.get("type").and_then(Value::as_str) == Some("text") {
            if let Some(t) = block.get("text").and_then(Value::as_str) {
                joined.push_str(t);
                joined.push('\n');
            }
        }
    }
    (!joined.trim().is_empty()).then(|| joined.trim_end().to_string())
}

/// Persist the orchestrator's `<MEMORY>` blocks from its final output as
/// [`Knowledge`] rows. The store's `is_substantive` gate is the only filter:
/// an absent block, an empty body, or a context-echo body persists nothing.
fn capture_orchestrator_knowledge(input: &HookInput, cwd: &str) {
    let Some(path) = transcript_path(input) else {
        return;
    };
    let Some(output) = last_assistant_text(&path) else {
        return;
    };
    let blocks = extract_memory_blocks(&output);
    if blocks.is_empty() {
        return;
    }

    let Ok(paths) = ClaudePaths::for_project(Path::new(cwd)) else {
        return;
    };
    let store = KnowledgeStore::new(paths.claude_dir());

    // Spec scope when a pipeline is active, else Global project knowledge.
    let spec = crate::shared::context::current_spec(cwd).filter(|s| !s.is_empty());
    let session = input
        .session_id
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let captured_at = now_iso8601();

    for body in blocks {
        // First non-empty line is the label; the whole block is the content.
        let label: String = body
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("")
            .trim()
            .chars()
            .take(200)
            .collect();
        let scope = match &spec {
            Some(s) => Scope::Spec { spec: s.clone() },
            None => Scope::Global,
        };
        let k = Knowledge {
            // Summary — the existing promotion path reclassifies to
            // Decision/Lesson by imperative downstream.
            kind: Kind::Summary,
            scope,
            label,
            content: body,
            origin: Origin {
                spec: spec.clone(),
                wave: None,
                role: Some(ORCHESTRATOR_ROLE.to_string()),
                session: session.clone(),
                captured_at: captured_at.clone(),
            },
            confidence: 0.0,
            status: Status::Active,
        };
        // The store's quality gate skips non-substantive records (`Ok(None)`).
        let _ = store.write(&k);
    }
}

impl Observer for SessionStopObserver {
    fn observe(&self, input: &HookInput, ctx: &Ctx) {
        // Belt-and-braces: this observer is registered for `Trigger::Stop`
        // (main session) only, but never capture a subagent turn here — its
        // `<MEMORY>` is already taken by `agent_summary_observer`.
        if input.is_subagent() {
            return;
        }
        let cwd = ctx.project_dir_or_cwd(input);
        let now = SystemTime::now();

        // Anti-spam — bail if the previous Stop fired inside the window.
        let marker = marker_path(&cwd);
        if recently_stopped(&marker, now) {
            return;
        }
        touch_marker(&marker);

        // Capture the orchestrator's own intentional knowledge. The store gate
        // rejects anything non-substantive, so a Stop with no `<MEMORY>` (or a
        // placeholder/echo one) persists nothing.
        capture_orchestrator_knowledge(input, &cwd);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustard_core::domain::model::contract::Trigger;
    use serde_json::json;
    use tempfile::tempdir;

    fn ctx(dir: &str) -> Ctx {
        Ctx {
            project_dir: dir.to_string(),
            trigger: Some(Trigger::Stop),
            workspace_root: None,
        }
    }

    /// Write a minimal transcript JSONL whose last assistant turn carries
    /// `text`, and return its path.
    fn write_transcript(dir: &Path, text: &str) -> PathBuf {
        let path = dir.join("session.jsonl");
        let line = json!({
            "type": "assistant",
            "message": { "role": "assistant", "content": [{ "type": "text", "text": text }] }
        })
        .to_string();
        // A user line first, then the assistant turn — exercises the walk.
        let user = json!({ "type": "user", "message": { "role": "user", "content": "go" } })
            .to_string();
        std::fs::write(&path, format!("{user}\n{line}\n")).unwrap();
        path
    }

    fn stop_input(session: &str, transcript: &Path) -> HookInput {
        HookInput {
            hook_event_name: Some("Stop".to_string()),
            session_id: Some(session.to_string()),
            raw: json!({ "transcript_path": transcript.to_string_lossy() }),
            ..HookInput::default()
        }
    }

    /// Read every persisted knowledge record across the four store dirs.
    fn read_store(project: &Path) -> Vec<Knowledge> {
        KnowledgeStore::new(project.join(".claude")).read_all(None)
    }

    #[test]
    fn stop_still_touches_anti_spam_marker() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_str().unwrap();
        std::fs::create_dir_all(dir.path().join(".claude/.harness")).unwrap();
        let transcript = write_transcript(dir.path(), "no memory block here");
        SessionStopObserver.observe(&stop_input("s-stop", &transcript), &ctx(project));
        assert!(marker_path(project).exists(), "marker should be touched");
    }

    #[test]
    fn antispam_skips_second_stop_inside_window() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_str().unwrap();
        let transcript = write_transcript(dir.path(), "no memory");
        SessionStopObserver.observe(&stop_input("s-1", &transcript), &ctx(project));
        let first = fs::modified(&marker_path(project)).unwrap();
        SessionStopObserver.observe(&stop_input("s-1", &transcript), &ctx(project));
        let second = fs::modified(&marker_path(project)).unwrap();
        assert_eq!(first, second, "second Stop inside the window is a no-op");
    }

    #[test]
    fn main_stop_with_memory_block_persists_global_knowledge() {
        // No MUSTARD_ACTIVE_SPEC and no pipeline-state → Global scope.
        let dir = tempdir().unwrap();
        let project = dir.path();
        let transcript = write_transcript(
            project,
            "Done.\n<MEMORY>\nPrefer write_atomic over std::fs::write for store rows.\n\
             A torn write corrupts the markdown frontmatter on crash.\n</MEMORY>",
        );
        SessionStopObserver.observe(
            &stop_input("s-orch", &transcript),
            &ctx(project.to_str().unwrap()),
        );
        let all = read_store(project);
        assert_eq!(all.len(), 1, "one orchestrator memory persisted: {all:?}");
        let k = &all[0];
        assert!(k.content.contains("write_atomic"), "body persisted: {}", k.content);
        assert_eq!(k.origin.role.as_deref(), Some("orchestrator"));
        assert_eq!(k.origin.session.as_deref(), Some("s-orch"));
        assert_eq!(k.kind, Kind::Summary);
        // Global scope when no spec is active (env unset in CI). The pipeline
        // FS-fallback wrote no state file, so this must be Global.
        assert_eq!(k.scope, Scope::Global, "no active spec → Global");
    }

    #[test]
    fn main_stop_scopes_to_active_pipeline_spec() {
        // The pipeline-state FS fallback of `current_spec` resolves a spec
        // without mutating process env (the crate forbids unsafe env writes).
        let dir = tempdir().unwrap();
        let project = dir.path();
        let states = project.join(".claude").join(".pipeline-states");
        std::fs::create_dir_all(&states).unwrap();
        std::fs::write(states.join("my-feature-zzz.json"), "{}").unwrap();

        let transcript = write_transcript(
            project,
            "<MEMORY>The conflict resolver must retry on 409, never blind-overwrite.</MEMORY>",
        );
        SessionStopObserver.observe(
            &stop_input("s-spec", &transcript),
            &ctx(project.to_str().unwrap()),
        );
        let all = read_store(project);
        assert_eq!(all.len(), 1, "one spec-scoped memory persisted: {all:?}");
        // When MUSTARD_ACTIVE_SPEC is unset (CI), the FS fallback yields the
        // state-file spec; when it IS set, the env spec wins — either way Spec.
        assert!(
            matches!(all[0].scope, Scope::Spec { .. }),
            "active pipeline → Spec scope, got {:?}",
            all[0].scope
        );
        assert!(all[0].origin.spec.is_some(), "origin records the spec");
    }

    #[test]
    fn main_stop_without_memory_block_persists_nothing() {
        let dir = tempdir().unwrap();
        let project = dir.path();
        let transcript = write_transcript(project, "I refactored module X and ran the tests.");
        SessionStopObserver.observe(
            &stop_input("s-none", &transcript),
            &ctx(project.to_str().unwrap()),
        );
        assert!(read_store(project).is_empty(), "a bare recap persists nothing");
    }

    #[test]
    fn empty_memory_block_is_gated_out() {
        let dir = tempdir().unwrap();
        let project = dir.path();
        // The block parses but its body is whitespace → extractor drops it.
        let transcript = write_transcript(project, "<MEMORY>   \n\t  </MEMORY>");
        SessionStopObserver.observe(
            &stop_input("s-empty", &transcript),
            &ctx(project.to_str().unwrap()),
        );
        assert!(read_store(project).is_empty(), "empty block → nothing");
    }

    #[test]
    fn context_echo_memory_block_is_gated_out() {
        let dir = tempdir().unwrap();
        let project = dir.path();
        // A body that is pure echoed Guards/context → is_substantive rejects it.
        let transcript = write_transcript(
            project,
            "<MEMORY>\nCONTEXT: the orchestrator routes intent\n</MEMORY>",
        );
        SessionStopObserver.observe(
            &stop_input("s-echo", &transcript),
            &ctx(project.to_str().unwrap()),
        );
        assert!(read_store(project).is_empty(), "context echo → gated out");
    }

    #[test]
    fn subagent_stop_input_is_never_captured_here() {
        // Belt-and-braces: an input the harness marks as a subagent (agent_id
        // present) is skipped — its MEMORY belongs to agent_summary_observer.
        let dir = tempdir().unwrap();
        let project = dir.path();
        let transcript = write_transcript(project, "<MEMORY>should not be captured here</MEMORY>");
        let mut input = stop_input("s-sub", &transcript);
        input.agent_id = Some("explore-42".to_string());
        SessionStopObserver.observe(&input, &ctx(project.to_str().unwrap()));
        assert!(read_store(project).is_empty(), "subagent input is skipped");
    }
}
