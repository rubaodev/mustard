//! `agent_summary_observer` — PostToolUse(Task) memory writer (W8.T8.4).
//!
//! Every time a `Task` (subagent) returns, we scan its output for an explicit
//! `<MEMORY>…</MEMORY>` block — the *intentional* knowledge the agent chose to
//! pass forward. That is the ONLY thing we capture. A `Resumo:` / `Summary:`
//! line is "what I did" (a task summary), not transferable knowledge; capturing
//! it polluted `agent_memory` with run-recaps, so the fallback was removed. No
//! `<MEMORY>` block → nothing is persisted (the hook returns early).
//!
//! What we find is persisted as an `agent_memory` row via the W7 helper
//! `crate::commands::knowledge::memory::persist_agent_memory_md`. The hook never
//! blocks — it is a pure [`Observer`].
//!
//! ## W3C → W4B migration
//!
//! `emit_economy_operation` routes economy events via
//! `crate::shared::events::route::emit` (NDJSON path). W4B then moved the
//! `agent_memory` write-path off SQLite onto markdown rows under
//! `.claude/memory/agent/` via `crate::commands::knowledge::memory::persist_agent_memory_md`,
//! so no `rusqlite` connection is opened from this module.
//!
//! ## Fail-open
//!
//! Every IO step degrades to a no-op. Telemetry is not load-bearing.

use serde_json::json;
use mustard_core::domain::model::event::ActorKind;
use crate::shared::events::economy;
use super::memory_block::extract_memory_block;
use mustard_core::domain::model::contract::{Ctx, HookInput, Observer};

/// The W8 auto-capture hook.
pub struct AgentSummaryObserver;


/// Pull the role for an `agent_memory` row from the Task input.
fn role_from_input(input: &HookInput) -> Option<String> {
    input
        .tool_input
        .get("subagent_type")
        .and_then(|v| v.as_str())
        .map(str::to_string)
}

/// Pull the Task output text. The harness layers vary; try common locations.
fn task_output(input: &HookInput) -> String {
    // The PostToolUse payload typically lands under `tool_response` /
    // `tool_result` / `output`. Probe all three.
    for key in ["tool_response", "tool_result", "output", "result"] {
        if let Some(v) = input.raw.get(key) {
            if let Some(s) = v.as_str() {
                if !s.is_empty() {
                    return s.to_string();
                }
            }
            // Sometimes the harness nests the text under `.text`.
            if let Some(s) = v.get("text").and_then(|x| x.as_str()) {
                if !s.is_empty() {
                    return s.to_string();
                }
            }
        }
    }
    String::new()
}

/// Persist a single captured summary as an `agent_memory` markdown row.
/// Fail-open: a write error degrades silently.
///
/// W4B migration: persistence moved off SQLite entirely. The summary lands as
/// a markdown file under `.claude/memory/agent/` via
/// [`crate::commands::knowledge::memory::persist_agent_memory_md`]. No `rusqlite::Connection`
/// is opened from this path.
fn persist(
    cwd: &str,
    session_id: Option<&str>,
    spec: Option<&str>,
    role: Option<&str>,
    summary: &str,
    details: Option<&str>,
) {
    // W4B migration: persistence moved off SQLite — write a markdown row in
    // `.claude/memory/agent/` via the shared helper.
    let _ = crate::commands::knowledge::memory::persist_agent_memory_md(
        cwd,
        session_id,
        spec,
        None,
        role,
        summary,
        details,
        0.7,
        Some("active"),
    );
}

impl Observer for AgentSummaryObserver {
    fn observe(&self, input: &HookInput, ctx: &Ctx) {
        // Only Task PostToolUse — the registry already constrains us, but
        // belt-and-braces.
        let output = task_output(input);
        if output.is_empty() {
            return;
        }
        let cwd = ctx.project_dir_or_cwd(input);

        // Capture ONLY an explicit `<MEMORY>…</MEMORY>` block — the intentional
        // knowledge the agent marked for the next agent/wave/spec. No block →
        // nothing to persist (a task-summary `Resumo:` is not transferable
        // knowledge and is no longer captured).
        let Some(body) = extract_memory_block(&output) else {
            return;
        };
        // First non-empty line is the summary; remainder = details.
        let mut lines = body.lines();
        let summary = lines.find(|l| !l.trim().is_empty()).unwrap_or("").trim();
        let rest: String = lines.collect::<Vec<_>>().join("\n");
        let rest = rest.trim();
        let details = if rest.is_empty() {
            None
        } else {
            Some(rest.to_string())
        };
        if summary.is_empty() {
            return;
        }
        let summary = summary.to_string();

        let role = role_from_input(input);
        let spec = crate::shared::context::current_spec(&cwd);
        let session_id = input
            .session_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(str::to_string);

        persist(
            &cwd,
            session_id.as_deref(),
            spec.as_deref(),
            role.as_deref(),
            &summary,
            details.as_deref(),
        );
        // Automatic cross-wave memory: when this Task ran AS a wave agent (a spec
        // AND an active wave both resolve), emit the same `agent.memory` event the
        // explicit `memory agent` CLI emits — so the next wave inherits this
        // agent's summary WITHOUT the orchestrator having to remember to call
        // `memory agent`. Non-wave Tasks (Explore, `/task`) carry no
        // MUSTARD_ACTIVE_WAVE → resolve to `None` → skipped.
        if let (Some(spec_slug), Some(wave)) =
            (spec.as_deref(), crate::shared::context::current_wave())
        {
            crate::commands::knowledge::memory::emit_agent_memory_event(
                &cwd,
                spec_slug,
                Some(wave),
                &summary,
                role.as_deref().unwrap_or("agent"),
                session_id.as_deref().unwrap_or(""),
            );
        }
        economy::emit(&cwd, ActorKind::Hook, "auto_capture_summary", "pipeline.economy.operation.invoked", None, json!({"operation": "auto_capture_summary.persist", "duration_ms": 0, "tokens_used": 0}));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustard_core::domain::model::contract::Trigger;
    use serde_json::json;
    use tempfile::tempdir;

    /// A `Resumo:` / `Summary:` line WITHOUT a `<MEMORY>` block is a task recap,
    /// not transferable knowledge — the fallback was removed, so nothing is
    /// captured and no `.claude/memory/agent/` row is written.
    #[test]
    fn resumo_without_memory_block_captures_nothing() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_str().unwrap().to_string();
        let input = HookInput {
            hook_event_name: Some("PostToolUse".to_string()),
            session_id: Some("s-1".to_string()),
            tool_input: json!({"subagent_type": "general-purpose"}),
            raw: json!({"tool_response": "Resumo: refatorei o módulo X e rodei os testes."}),
            ..HookInput::default()
        };
        AgentSummaryObserver.observe(&input, &ctx(&project));
        assert!(
            !dir.path().join(".claude/memory/agent").exists(),
            "a bare Resumo: must not produce a memory row"
        );
    }

    /// An explicit `<MEMORY>…</MEMORY>` block IS captured — its body lands as the
    /// `agent_memory` row.
    #[test]
    fn memory_block_is_captured() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_str().unwrap().to_string();
        let input = HookInput {
            hook_event_name: Some("PostToolUse".to_string()),
            session_id: Some("s-2".to_string()),
            tool_input: json!({"subagent_type": "general-purpose"}),
            raw: json!({
                "tool_response":
                    "did stuff\n<MEMORY>\nChose retry-on-conflict for the writer.\n\
                     The API 409s under concurrent load, so a blind retry corrupts state.\n\
                     </MEMORY>"
            }),
            ..HookInput::default()
        };
        AgentSummaryObserver.observe(&input, &ctx(&project));
        let dir_path = dir.path().join(".claude/memory/agent");
        assert!(dir_path.exists(), "an explicit MEMORY block must produce a row");
        let captured = std::fs::read_dir(&dir_path)
            .unwrap()
            .filter_map(Result::ok)
            .map(|e| std::fs::read_to_string(e.path()).unwrap_or_default())
            .collect::<String>();
        assert!(
            captured.contains("retry-on-conflict"),
            "the MEMORY body should be persisted: {captured}"
        );
    }

    fn ctx(dir: &str) -> Ctx {
        Ctx {
            project_dir: dir.to_string(),
            trigger: Some(Trigger::PostToolUse),
            workspace_root: None,
        }
    }
}
