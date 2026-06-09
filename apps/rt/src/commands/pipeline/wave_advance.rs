//! `mustard-rt run wave-advance` — composite dispatch face: the next pending
//! wave level, prompts already rendered inline.
//!
//! Composes, **in-process** (module-qualified, no subprocess):
//!
//! 1. `dispatch-plan` — [`crate::commands::pipeline::dispatch_plan::build_plan`]
//!    (the wave DAG + ordering, including the single-spec one-item fallback).
//! 2. `agent-prompt-render` — for each item of the next pending level, the
//!    prompt is rendered inline via
//!    [`crate::commands::agent::agent_prompt_render::render_prompt_at`] and
//!    returned as text, so the orchestrator no longer shells the `prompt_cmd`
//!    each dispatch item used to carry.
//!
//! ## "Next pending level" semantics
//!
//! A wave counts as **completed** when a `pipeline.wave.complete` event with
//! its wave number exists in the spec's per-spec NDJSON `.events/` log (the
//! same signal `emit-pipeline` writes and the resume projections fold). There
//! is **no reliable persisted "dispatched" signal** — `pipeline.task.dispatch`
//! is emitted by the orchestrator relay, not enforced — so this command
//! returns the items of the FIRST dependency level (ascending) that still has
//! a non-completed wave, filtered to its non-completed waves. Re-invoking
//! after dispatch but before the waves complete returns the same level again;
//! the caller owns not double-dispatching within a session. All waves
//! completed (or no plan at all) → empty array.
//!
//! ## Output
//!
//! A deterministic JSON array, one item per agent of the pending level:
//! `[{wave, role, subproject, subagent_type, prompt}]` — `prompt` is the full
//! rendered dispatch text, ready for the `Task` tool. Fail-open: an unknown
//! spec degrades to `[]`; exit 0 always.

use crate::commands::agent::agent_prompt_render::{self, RenderMode};
use crate::commands::pipeline::dispatch_plan;
use mustard_core::domain::model::event::EVENT_PIPELINE_WAVE_COMPLETE;
use mustard_core::io::claude_paths::ClaudePaths;
use mustard_core::view::projection::read_harness_events_from_ndjson_dir;
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// One ready-to-dispatch agent of the pending level.
#[derive(Debug, Serialize)]
pub struct AdvanceItem {
    /// 1-based wave number (`0` marks the wave-less single-spec fallback).
    pub wave: u32,
    /// Role token (the `{role}` suffix of `wave-N-{role}`).
    pub role: String,
    /// Subproject path relative to the project root, or `"."`.
    pub subproject: String,
    /// The `subagent_type` to pass to `Task` (picked by the tool, never by hand).
    #[serde(rename = "subagent_type")]
    pub subagent_type: String,
    /// The rendered dispatch prompt (the stdout `agent-prompt-render` would
    /// print), inline — the orchestrator relays it straight to `Task`.
    pub prompt: String,
}

/// CLI entry — `mustard-rt run wave-advance --spec <slug>`.
pub fn run(spec: &str) {
    let project = PathBuf::from(crate::shared::context::project_dir());
    let items = advance(&project, spec);
    println!(
        "{}",
        serde_json::to_string_pretty(&items).unwrap_or_else(|_| "[]".to_string())
    );
}

/// The composite miolo against an explicit `project` root (testable without
/// mutating the process cwd). See the module docs for the pending-level
/// semantics.
pub(crate) fn advance(project: &Path, spec: &str) -> Vec<AdvanceItem> {
    let spec_dir = dispatch_plan::resolve_spec_dir(project, spec);
    let plan = dispatch_plan::build_plan(project, &spec_dir, spec, None);
    if plan.is_empty() {
        return Vec::new();
    }

    let completed = completed_waves(project, spec);
    let pending_level = plan
        .iter()
        .filter(|it| !completed.contains(&it.wave))
        .map(|it| it.level)
        .min();
    let Some(level) = pending_level else {
        // Every wave already carries a pipeline.wave.complete — nothing pending.
        return Vec::new();
    };

    plan.into_iter()
        .filter(|it| it.level == level && !completed.contains(&it.wave))
        .map(|it| {
            // Wave 0 is the single-spec fallback: render the root spec.md
            // (no `--wave`), exactly like the prompt_cmd dispatch-plan emits.
            let wave_arg = (it.wave > 0).then_some(it.wave);
            let prompt = agent_prompt_render::render_prompt_at(
                project,
                Some(spec),
                wave_arg,
                &it.role,
                Path::new(&it.subproject),
                RenderMode::First,
                None,
                None,
                None,
                None,
            );
            AdvanceItem {
                wave: it.wave,
                role: it.role,
                subproject: it.subproject,
                subagent_type: it.subagent_type,
                prompt,
            }
        })
        .collect()
}

/// The set of wave numbers carrying a `pipeline.wave.complete` event in the
/// spec's per-spec NDJSON log. Fail-open: a missing/unreadable events dir
/// yields the empty set (every wave pending — the conservative read).
fn completed_waves(project: &Path, spec: &str) -> BTreeSet<u32> {
    let events_dir = ClaudePaths::for_project(project)
        .and_then(|p| p.for_spec(spec))
        .ok()
        .map_or_else(
            || {
                ClaudePaths::compose_unchecked(project)
                    .spec_dir()
                    .join(spec)
                    .join(".events")
            },
            |sp| sp.events_dir(),
        );
    read_harness_events_from_ndjson_dir(&events_dir)
        .into_iter()
        .filter(|e| e.event == EVENT_PIPELINE_WAVE_COMPLETE && e.spec.as_deref() == Some(spec))
        .filter_map(|e| {
            e.payload
                .get("wave")
                .and_then(Value::as_u64)
                .and_then(|w| u32::try_from(w).ok())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    /// Anchor a project root so `ClaudePaths::for_project` resolves (mirrors
    /// the dispatch_plan test helper).
    fn anchor(dir: &Path) {
        std::fs::create_dir_all(dir.join(".claude")).unwrap();
        std::fs::write(dir.join("mustard.json"), b"{}").unwrap();
    }

    /// Seed a 3-wave spec: waves 1 and 2 are independent (level 0), wave 3
    /// depends on both (level 1). Each wave dir carries a spec.md with Tasks.
    fn seed_three_waves(project: &Path, slug: &str) -> PathBuf {
        let spec_dir = project.join(".claude").join("spec").join(slug);
        std::fs::create_dir_all(&spec_dir).unwrap();
        std::fs::write(
            spec_dir.join("wave-plan.md"),
            "\
| Wave | Spec | Role | Depends on | Summary |
|------|------|------|------------|---------|
| 1 | [[wave-1-rt]] | rt | — | base |
| 2 | [[wave-2-cli]] | cli | — | parallel base |
| 3 | [[wave-3-core]] | core | [[wave-1-rt]], [[wave-2-cli]] | joins both |
",
        )
        .unwrap();
        for (n, role) in [(1, "rt"), (2, "cli"), (3, "core")] {
            let dir = spec_dir.join(format!("wave-{n}-{role}"));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(
                dir.join("spec.md"),
                format!("# wave-{n}-{role}\n\n## Tasks\n\n- [ ] task for {role}\n"),
            )
            .unwrap();
        }
        spec_dir
    }

    /// Emit a `pipeline.wave.complete` for `wave` into the spec's events log.
    fn complete_wave(project: &Path, spec: &str, wave: u32) {
        use mustard_core::domain::model::event::{Actor, ActorKind, HarnessEvent, SCHEMA_VERSION};
        let event = HarnessEvent {
            v: SCHEMA_VERSION,
            ts: format!("2026-06-09T00:00:0{wave}.000Z"),
            session_id: "test-session".to_string(),
            wave,
            actor: Actor {
                kind: ActorKind::Orchestrator,
                id: Some("emit-pipeline".to_string()),
                actor_type: None,
            },
            event: EVENT_PIPELINE_WAVE_COMPLETE.to_string(),
            payload: json!({ "wave": wave }),
            spec: Some(spec.to_string()),
        };
        crate::shared::events::route::emit(project.to_str().unwrap(), &event);
    }

    /// Happy path: with no wave completed, the first level (the two parallel
    /// waves 1 and 2) comes back, each with its prompt rendered inline.
    #[test]
    fn composite_wave_advance_returns_first_level_with_inline_prompts() {
        let dir = tempdir().unwrap();
        anchor(dir.path());
        let project = dir.path();
        seed_three_waves(project, "adv");

        let items = advance(project, "adv");
        assert_eq!(items.len(), 2, "level 0 carries the two parallel waves");
        assert_eq!(items[0].wave, 1);
        assert_eq!(items[0].role, "rt");
        assert_eq!(items[1].wave, 2);
        assert_eq!(items[1].role, "cli");
        for item in &items {
            assert_eq!(item.subagent_type, "general-purpose");
            assert!(
                item.prompt.contains(&format!("task for {}", item.role)),
                "prompt must inline the wave's task body: {}",
                item.prompt
            );
            assert!(
                !item.prompt.trim().is_empty(),
                "prompt is the rendered text, not a command"
            );
            assert!(
                !item.prompt.contains("agent-prompt-render"),
                "prompt must not be a prompt_cmd shell line"
            );
        }
    }

    /// Dependency progression: completing waves 1 and 2 advances the pending
    /// level to wave 3; completing everything yields the empty array.
    #[test]
    fn composite_wave_advance_progresses_levels_and_drains() {
        let dir = tempdir().unwrap();
        anchor(dir.path());
        let project = dir.path();
        seed_three_waves(project, "adv2");

        // Wave 1 done, wave 2 still pending → level 0 again, only wave 2.
        complete_wave(project, "adv2", 1);
        let items = advance(project, "adv2");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].wave, 2, "non-completed level-0 wave still pending");

        // Both level-0 waves done → level 1 (wave 3).
        complete_wave(project, "adv2", 2);
        let items = advance(project, "adv2");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].wave, 3);
        assert_eq!(items[0].role, "core");

        // Everything done → no pending level → empty.
        complete_wave(project, "adv2", 3);
        let items = advance(project, "adv2");
        assert!(items.is_empty(), "no pending level returns the empty list");
    }

    /// Degraded: an unknown spec (no dir, no spec.md) degrades to `[]`.
    #[test]
    fn composite_wave_advance_unknown_spec_degrades_empty() {
        let dir = tempdir().unwrap();
        anchor(dir.path());
        assert!(advance(dir.path(), "ghost").is_empty());
    }

    /// Single-spec fallback (no wave plan): one wave-0 `impl` item whose
    /// prompt renders the root spec.md (no `--wave` semantics).
    #[test]
    fn composite_wave_advance_single_spec_renders_root() {
        let dir = tempdir().unwrap();
        anchor(dir.path());
        let project = dir.path();
        let spec_dir = project.join(".claude").join("spec").join("flat");
        std::fs::create_dir_all(&spec_dir).unwrap();
        std::fs::write(
            spec_dir.join("spec.md"),
            "# Flat\n\n## Tasks\n\n- [ ] the only task\n",
        )
        .unwrap();

        let items = advance(project, "flat");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].wave, 0);
        assert_eq!(items[0].role, "impl");
        assert!(
            items[0].prompt.contains("the only task"),
            "root spec.md tasks must reach the prompt: {}",
            items[0].prompt
        );
    }
}
