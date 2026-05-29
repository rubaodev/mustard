//! `graph_wirelink_gate` — pre-write validation of concept-graph wirelinks.
//!
//! FASE 2 (decision 8 — wirelinks/grafo). A `PreToolUse(Write|Edit)` `Check`
//! scoped to `.claude/graph/**`: before a concept-node file is written, it
//! parses the `[[id]]` wirelinks out of the incoming content and validates
//! each one against the live [`GraphIndex`] ([`graph::build_index`], pure). Any
//! `[[id]]` whose target is not a known node is flagged so the typo is caught
//! *before* it becomes a silent orphan edge in the next `graph-index` build.
//!
//! ## Role — advisory, never blocking (fail-open)
//!
//! The gate **never** denies. Its verdict is [`Verdict::Warn`] (advisory,
//! default) or [`Verdict::Inject`] (the same unknown-id report folded into the
//! agent's context) depending on `MUSTARD_GRAPH_WIRELINK_MODE`. The default is
//! `warn`. `off` disables it entirely. There is no `strict`/`deny` mode by
//! design — a wirelink to a not-yet-written node is a normal cold-start state
//! (the orphan surfaces as a build warning later), so blocking the write would
//! be wrong.
//!
//! Determinism: the same content + same vault yields the same verdict. Every
//! IO failure (unreadable vault, missing project dir) degrades to
//! [`Verdict::Allow`] — the validation simply does not fire.

use crate::commands::scan::graph::{self, GraphIndex};
use mustard_core::domain::model::contract::{Check, Ctx, HookInput, Trigger, Verdict};
use mustard_core::io::atomic_md::scan_links;
use mustard_core::platform::error::Error;
use std::path::Path;

/// The concept-graph wirelink pre-write gate. Stateless — every invocation
/// rebuilds the index from the vault on disk.
pub struct GraphWirelinkGate;

/// Output mode of the gate. Advisory only — there is no blocking mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WirelinkMode {
    /// Disabled — the gate is a pure no-op (`Allow`).
    Off,
    /// Surface the unknown-id report as a non-blocking advisory (default).
    Warn,
    /// Fold the same report into the agent's context (`additionalContext`).
    Inject,
}

/// Read `MUSTARD_GRAPH_WIRELINK_MODE` (default `warn`). `off` disables the
/// gate; `inject` emits the report as context; anything else is `warn`.
fn wirelink_mode() -> WirelinkMode {
    match std::env::var("MUSTARD_GRAPH_WIRELINK_MODE")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "off" => WirelinkMode::Off,
        "inject" => WirelinkMode::Inject,
        _ => WirelinkMode::Warn,
    }
}

/// The `file_path` of a Write/Edit invocation (accepts the legacy `path` key).
fn file_path_of(input: &HookInput) -> Option<String> {
    let ti = &input.tool_input;
    ti.get("file_path")
        .or_else(|| ti.get("path"))
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// `true` when `file_path` lives under `.claude/graph/` — the only tree this
/// gate scopes to. Forward-slash normalised; matches the path anywhere in the
/// string so both absolute (`/p/.claude/graph/x.md`) and relative
/// (`.claude/graph/x.md`) forms hit.
fn is_graph_path(file_path: &str) -> bool {
    let norm = file_path.replace('\\', "/");
    norm.contains(".claude/graph/")
}

/// The content a Write/Edit is about to put on disk: `content` (Write) or the
/// `new_string` replacement (Edit). Empty when neither is present.
fn incoming_content(input: &HookInput) -> String {
    let ti = &input.tool_input;
    if let Some(s) = ti.get("content").and_then(|x| x.as_str()) {
        return s.to_string();
    }
    if let Some(s) = ti.get("new_string").and_then(|x| x.as_str()) {
        return s.to_string();
    }
    String::new()
}

/// The `[[id]]`-shaped wirelinks in `content`: every `[[…]]` token whose inner
/// text is a namespaced concept-id (`[a-zA-Z0-9_.-]`). Free-text wikilinks
/// (`[[my note]]`) are dropped — they are not graph edges. Deduplicated,
/// source order preserved.
fn wirelink_ids(content: &str) -> Vec<String> {
    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    let mut out: Vec<String> = Vec::new();
    for token in scan_links(content) {
        if graph::is_concept_id(&token) && seen.insert(token.clone()) {
            out.push(token);
        }
    }
    out
}

/// Validate `content`'s wirelinks against `index`, returning the unknown ids
/// (those not present as a node). The id of the file being written is excluded
/// when known — a node may legitimately reference its own id, and during a
/// fresh write the node is not yet in the on-disk index.
fn unknown_ids(content: &str, index: &GraphIndex, self_id: Option<&str>) -> Vec<String> {
    wirelink_ids(content)
        .into_iter()
        .filter(|id| {
            self_id != Some(id.as_str()) && !index.nodes.contains_key(id)
        })
        .collect()
}

/// Parse the frontmatter `id:` of the node being written, so a self-reference
/// is not mis-flagged as unknown. Mirrors `graph::build_index`'s parse: only
/// the `---\n…\n---` leading block, only the first `id:` line.
fn self_id_of(content: &str) -> Option<String> {
    let stripped = content.strip_prefix("---\n")?;
    let end = stripped.find("\n---")?;
    for line in stripped[..end].lines() {
        if let Some(rest) = line.strip_prefix("id:") {
            let v = rest.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Format the advisory report for a non-empty `unknown` set.
fn report(unknown: &[String]) -> String {
    let list = unknown.join(", ");
    format!(
        "[graph-wirelink] {} unknown concept-id wirelink(s) in this \
         .claude/graph/ write: {list}. These would become orphan edges in the \
         next graph-index build — fix the id or write the target node first.",
        unknown.len()
    )
}

impl Check for GraphWirelinkGate {
    fn evaluate(&self, input: &HookInput, ctx: &Ctx) -> Result<Verdict, Error> {
        // Only PreToolUse(Write|Edit) reaches us; be defensive regardless.
        if ctx.trigger != Some(Trigger::PreToolUse) {
            return Ok(Verdict::Allow);
        }
        let mode = wirelink_mode();
        if mode == WirelinkMode::Off {
            return Ok(Verdict::Allow);
        }
        // Scope: only concept-graph files.
        let Some(file_path) = file_path_of(input) else {
            return Ok(Verdict::Allow);
        };
        if !is_graph_path(&file_path) {
            return Ok(Verdict::Allow);
        }
        let content = incoming_content(input);
        if content.is_empty() {
            return Ok(Verdict::Allow);
        }
        // Build the live index (pure read; fail-open to empty on any error).
        let cwd = ctx.project_dir_or_cwd(input);
        let index = graph::build_index(Path::new(&cwd));
        let self_id = self_id_of(&content);
        let unknown = unknown_ids(&content, &index, self_id.as_deref());
        if unknown.is_empty() {
            return Ok(Verdict::Allow);
        }
        let msg = report(&unknown);
        match mode {
            WirelinkMode::Warn => Ok(Verdict::Warn { message: msg }),
            WirelinkMode::Inject => Ok(Verdict::Inject { context: msg }),
            WirelinkMode::Off => Ok(Verdict::Allow),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::Path as StdPath;

    fn write_node(root: &StdPath, file: &str, body: &str) {
        let path = root.join(".claude").join("graph").join(file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    fn pre_write(cwd: &str, file_path: &str, content: &str) -> (HookInput, Ctx) {
        let input = HookInput {
            tool_name: Some("Write".to_string()),
            tool_input: json!({ "file_path": file_path, "content": content }),
            hook_event_name: Some("PreToolUse".to_string()),
            cwd: Some(cwd.to_string()),
            ..HookInput::default()
        };
        let ctx = Ctx {
            project_dir: cwd.to_string(),
            trigger: Some(Trigger::PreToolUse),
            workspace_root: None,
        };
        (input, ctx)
    }

    #[test]
    fn warns_on_unknown_wirelink_id() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // One real node lives in the vault.
        write_node(
            root,
            "rt.entity.user.md",
            "---\nid: rt.entity.user\nkind: entity\n---\n# User\n",
        );
        let cwd = root.to_string_lossy().into_owned();
        // The incoming write references a node that does NOT exist (typo).
        let content = "---\nid: rt.conv.scan\nkind: conv\n---\nsee [[rt.entity.usr]]\n";
        let graph_file = root
            .join(".claude")
            .join("graph")
            .join("rt.conv.scan.md")
            .to_string_lossy()
            .into_owned();
        let (input, ctx) = pre_write(&cwd, &graph_file, content);
        let verdict = GraphWirelinkGate.evaluate(&input, &ctx).expect("no error");
        match verdict {
            Verdict::Warn { message } => {
                assert!(message.contains("rt.entity.usr"), "got {message}");
                // Never blocks.
                assert!(!Verdict::Warn { message: message.clone() }.is_blocking());
            }
            other => panic!("expected Warn, got {other:?}"),
        }
    }

    #[test]
    fn allows_when_all_wirelinks_resolve() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write_node(
            root,
            "rt.entity.user.md",
            "---\nid: rt.entity.user\nkind: entity\n---\n# User\n",
        );
        let cwd = root.to_string_lossy().into_owned();
        // Valid wirelink to the existing node + a self-reference.
        let content = "---\nid: rt.conv.scan\nkind: conv\n---\nsee [[rt.entity.user]] and self [[rt.conv.scan]]\n";
        let graph_file = root
            .join(".claude")
            .join("graph")
            .join("rt.conv.scan.md")
            .to_string_lossy()
            .into_owned();
        let (input, ctx) = pre_write(&cwd, &graph_file, content);
        assert_eq!(
            GraphWirelinkGate.evaluate(&input, &ctx).expect("no error"),
            Verdict::Allow
        );
    }

    #[test]
    fn ignores_writes_outside_graph_tree() {
        let dir = tempfile::tempdir().unwrap();
        let cwd = dir.path().to_string_lossy().into_owned();
        // A spec.md write with an unknown wirelink is NOT this gate's concern.
        let content = "see [[totally.unknown.id]]\n";
        let (input, ctx) = pre_write(&cwd, "src/main.rs", content);
        assert_eq!(
            GraphWirelinkGate.evaluate(&input, &ctx).expect("no error"),
            Verdict::Allow
        );
    }

    #[test]
    fn free_text_wikilinks_are_not_validated() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let cwd = root.to_string_lossy().into_owned();
        // `[[my note]]` is free text, not a concept-id → not an edge → no warn.
        let content = "---\nid: rt.conv.x\nkind: conv\n---\nfree [[my note]] here\n";
        let graph_file = root
            .join(".claude")
            .join("graph")
            .join("rt.conv.x.md")
            .to_string_lossy()
            .into_owned();
        let (input, ctx) = pre_write(&cwd, &graph_file, content);
        assert_eq!(
            GraphWirelinkGate.evaluate(&input, &ctx).expect("no error"),
            Verdict::Allow
        );
    }

    #[test]
    fn non_pre_tool_use_allows() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let cwd = root.to_string_lossy().into_owned();
        let content = "---\nid: rt.conv.x\nkind: conv\n---\n[[unknown.id.here]]\n";
        let graph_file = root
            .join(".claude")
            .join("graph")
            .join("rt.conv.x.md")
            .to_string_lossy()
            .into_owned();
        let (mut input, mut ctx) = pre_write(&cwd, &graph_file, content);
        input.hook_event_name = Some("PostToolUse".to_string());
        ctx.trigger = Some(Trigger::PostToolUse);
        assert_eq!(
            GraphWirelinkGate.evaluate(&input, &ctx).expect("no error"),
            Verdict::Allow
        );
    }
}
