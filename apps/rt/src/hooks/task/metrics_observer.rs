//! `metrics_observer` — emits a `tool.use` heartbeat after a tool completes.
//!
//! Ports `metrics-tracker.js`'s verdict-free heartbeat. Shared plumbing lives
//! in [`super::common`]. The emitted event actor id stays `"metrics-tracker"`
//! (the telemetry namespace — unrelated to this module's wire id).

use super::common;
use mustard_core::domain::model::contract::{Ctx, HookInput, Observer, Trigger};
use serde_json::{json, Value};

/// `metrics-tracker`: emits a `tool.use` heartbeat after a tool completes.
///
/// CONCERN: the JS hook resolves the active pipeline-state to tag the event
/// with `phase` / `spec` / `wave`. That depends on pipeline-state access that
/// the `Ctx` does not yet expose (Wave-4/5 concern). This port emits the
/// verdict-free heartbeat with the salient `target` fields; the `phase` /
/// `spec` tags are left `null`, exactly as the JS does when no state is found.
pub struct MetricsObserver;

impl Observer for MetricsObserver {
    fn observe(&self, input: &HookInput, ctx: &Ctx) {
        if ctx.trigger != Some(Trigger::PostToolUse) {
            return;
        }
        let project = if ctx.project_dir.is_empty() {
            common::project_dir(input)
        } else {
            ctx.project_dir.clone()
        };
        let tool_name = input.tool_name.as_deref().unwrap_or_default();
        let tool_input = &input.tool_input;

        // Salient `target` fields, capped — mirrors the JS `target` object.
        let mut target = serde_json::Map::new();
        if let Some(file) = tool_input
            .get("file_path")
            .or_else(|| tool_input.get("notebook_path"))
            .and_then(|v| v.as_str())
        {
            target.insert("file".into(), json!(file));
        }
        if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
            target.insert("command".into(), json!(common::cap(cmd, 120)));
        }
        if let Some(pat) = tool_input.get("pattern").and_then(|v| v.as_str()) {
            target.insert("pattern".into(), json!(common::cap(pat, 80)));
        }
        if let Some(desc) = tool_input.get("description").and_then(|v| v.as_str()) {
            target.insert("description".into(), json!(common::cap(desc, 100)));
        }
        if let Some(sub) = tool_input.get("subagent_type").and_then(|v| v.as_str()) {
            target.insert("subagent".into(), json!(sub));
        }
        if let Some(url) = tool_input.get("url").and_then(|v| v.as_str()) {
            target.insert("url".into(), json!(common::cap(url, 120)));
        }

        let payload = json!({
            "tool": tool_name,
            "phase": Value::Null,
            "target": if target.is_empty() { Value::Null } else { Value::Object(target) },
        });
        common::emit_event(&project, "metrics-tracker", "tool.use", payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    fn ctx(trigger: Trigger, dir: &str) -> Ctx {
        Ctx {
            project_dir: dir.to_string(),
            trigger: Some(trigger),
            workspace_root: None,
        }
    }

    #[test]
    fn metrics_observer_observe_is_infallible() {
        let dir = tempdir().unwrap();
        let project = dir.path().to_str().unwrap();
        let input = HookInput {
            tool_name: Some("Bash".to_string()),
            tool_input: json!({ "command": "git status" }),
            hook_event_name: Some("PostToolUse".to_string()),
            ..HookInput::default()
        };
        MetricsObserver.observe(&input, &ctx(Trigger::PostToolUse, project));
    }
}
