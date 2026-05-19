//! The module registry — which enforcement modules run for which event/tool.
//!
//! Open/Closed in practice (b3 spec § Arquitetura "SOLID"): adding a check is
//! *only* registering a [`Module`] here. The dispatcher reads the registry and
//! never changes. A module is keyed by the `(Trigger, tool)` pairs it applies
//! to, so an unrelated invocation skips it entirely instead of running it just
//! to have it self-`Allow`.

use crate::hooks::bash_guard::BashGuard;
use crate::hooks::budget::BudgetGuard;
use crate::hooks::model_routing::ModelRoutingGate;
use crate::hooks::skills_audit::SkillsAudit;
use crate::hooks::tracker::{
    MainContextCounter, MetricsTracker, SkillUsageTracker, SubagentTracker, ToolUseCounter,
};
use mustard_core::config::Mode;
use mustard_core::model::contract::{Check, Observer, Trigger};

/// Which tool an `(event, tool)` registration entry applies to.
///
/// The JS `settings.json` matchers are one of: a literal tool name (`"Bash"`,
/// `"Task"`), an alternation (`"Task|Agent"` — expressed as two entries here),
/// the wildcard `".*"` (every tool), or absent (a non-tool lifecycle event
/// like `SubagentStart`). [`ToolMatch`] models exactly those three cases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolMatch {
    /// A non-tool lifecycle event — the harness invocation has no `tool_name`.
    ///
    /// No Wave-3 module needs this exact case (lifecycle modules use
    /// [`ToolMatch::Any`], which already matches a `None` tool); it is kept as
    /// API surface for a later wave that registers a `None`-tool-only module.
    #[allow(dead_code)]
    None,
    /// Every tool (the `".*"` matcher), and also non-tool events: the JS `.*`
    /// PreToolUse matcher fires for any invocation.
    Any,
    /// One specific tool name.
    Named(&'static str),
}

impl ToolMatch {
    /// `true` if this matcher applies to an invocation carrying `tool`.
    #[must_use]
    fn matches(self, tool: Option<&str>) -> bool {
        match self {
            Self::None => tool.is_none(),
            Self::Any => true,
            Self::Named(name) => tool == Some(name),
        }
    }
}

/// One enforcement concern. A module is a `Check`, an `Observer`, or both.
/// `bash_guard`, for example, is both — the four ported PreToolUse(Bash) gates
/// (`Check`) and the `pr-detect` PostToolUse(Bash) telemetry (`Observer`).
pub struct Module {
    /// Stable id used by `mustard-rt check <id>` and by the enforcement
    /// config (`MUSTARD_<ID>_MODE`). Lowercase, snake or kebab.
    pub id: &'static str,
    /// The `(Trigger, ToolMatch)` pairs this module applies to.
    pub applies_to: &'static [(Trigger, ToolMatch)],
    /// The gate behaviour, if this module decides anything. `None` for a
    /// pure-`Observer` module.
    pub check: Option<Box<dyn Check>>,
    /// The telemetry behaviour, if this module observes. `None` for a
    /// pure-`Check` module.
    pub observer: Option<Box<dyn Observer>>,
}

impl Module {
    /// `true` if this module is applicable to the given event/tool.
    #[must_use]
    pub fn matches(&self, trigger: Trigger, tool: Option<&str>) -> bool {
        self.applies_to
            .iter()
            .any(|(t, want_tool)| *t == trigger && want_tool.matches(tool))
    }
}

/// The set of registered enforcement modules.
pub struct Registry {
    modules: Vec<Module>,
}

impl Registry {
    /// Build the registry with every module Mustard ships.
    ///
    /// Early b3 waves register only `bash_guard`; later waves push their
    /// families (`budget`, `size_gate`, …) here, leaving the dispatcher
    /// untouched.
    #[must_use]
    pub fn new() -> Self {
        let modules = vec![
            Module {
                id: "bash_guard",
                // `bash_guard` is both a `Check` and an `Observer` — it ports
                // the full Bash family (5/5): `bash-safety`,
                // `bash-native-redirect`, `rtk-rewrite` and `review-gate` as
                // PreToolUse(Bash) gates, plus `pr-detect` as PostToolUse(Bash)
                // telemetry.
                applies_to: &[
                    (Trigger::PreToolUse, ToolMatch::Named("Bash")),
                    (Trigger::PostToolUse, ToolMatch::Named("Bash")),
                ],
                check: Some(Box::new(BashGuard)),
                observer: Some(Box::new(BashGuard)),
            },
            // ── Wave 3: Task / Subagent family ───────────────────────────────
            Module {
                id: "budget",
                // `context-budget` (PreToolUse(Task) prompt-size `Check`) +
                // `output-budget` (PostToolUse(Task) return-size `Observer`).
                applies_to: &[
                    (Trigger::PreToolUse, ToolMatch::Named("Task")),
                    (Trigger::PreToolUse, ToolMatch::Named("Agent")),
                    (Trigger::PostToolUse, ToolMatch::Named("Task")),
                    (Trigger::PostToolUse, ToolMatch::Named("Agent")),
                ],
                check: Some(Box::new(BudgetGuard)),
                observer: Some(Box::new(BudgetGuard)),
            },
            Module {
                id: "model_routing",
                // `model-routing-gate` — PreToolUse(Task) model-selection gate.
                applies_to: &[
                    (Trigger::PreToolUse, ToolMatch::Named("Task")),
                    (Trigger::PreToolUse, ToolMatch::Named("Agent")),
                ],
                check: Some(Box::new(ModelRoutingGate)),
                observer: None,
            },
            Module {
                id: "tool_use_counter",
                // `tool-use-counter` — caps tool uses per Explore subagent.
                // The JS matcher is `.*` on PreToolUse (every tool counts),
                // plus the Subagent lifecycle and SessionStart.
                applies_to: &[
                    (Trigger::PreToolUse, ToolMatch::Any),
                    (Trigger::SubagentStart, ToolMatch::Any),
                    (Trigger::SubagentStop, ToolMatch::Any),
                    (Trigger::SessionStart, ToolMatch::Any),
                ],
                check: Some(Box::new(ToolUseCounter)),
                observer: None,
            },
            Module {
                id: "main_context_counter",
                // `main-context-counter` — enforces L0 on the orchestrator.
                applies_to: &[
                    (Trigger::PreToolUse, ToolMatch::Any),
                    (Trigger::SubagentStart, ToolMatch::Any),
                    (Trigger::SubagentStop, ToolMatch::Any),
                    (Trigger::SessionStart, ToolMatch::Any),
                ],
                check: Some(Box::new(MainContextCounter)),
                observer: None,
            },
            Module {
                id: "subagent_tracker",
                // `subagent-tracker` — `agent.start` / `agent.stop` telemetry.
                applies_to: &[
                    (Trigger::PreToolUse, ToolMatch::Named("Task")),
                    (Trigger::PreToolUse, ToolMatch::Named("Agent")),
                    (Trigger::PostToolUse, ToolMatch::Named("Task")),
                    (Trigger::PostToolUse, ToolMatch::Named("Agent")),
                ],
                check: None,
                observer: Some(Box::new(SubagentTracker)),
            },
            Module {
                id: "metrics_tracker",
                // `metrics-tracker` — `tool.use` heartbeat after a tool runs.
                applies_to: &[
                    (Trigger::PostToolUse, ToolMatch::Named("Bash")),
                    (Trigger::PostToolUse, ToolMatch::Named("Write")),
                    (Trigger::PostToolUse, ToolMatch::Named("Edit")),
                    (Trigger::PostToolUse, ToolMatch::Named("Task")),
                    (Trigger::PostToolUse, ToolMatch::Named("Agent")),
                    (Trigger::PostToolUse, ToolMatch::Named("Read")),
                ],
                check: None,
                observer: Some(Box::new(MetricsTracker)),
            },
            Module {
                id: "skill_usage_tracker",
                // `skill-usage-tracker` — `skill.invoked` event per Skill call.
                applies_to: &[(Trigger::PostToolUse, ToolMatch::Named("Skill"))],
                check: None,
                observer: Some(Box::new(SkillUsageTracker)),
            },
            Module {
                id: "skills_audit",
                // `recommended-skills-audit` — advisory count on PreToolUse(Task).
                applies_to: &[
                    (Trigger::PreToolUse, ToolMatch::Named("Task")),
                    (Trigger::PreToolUse, ToolMatch::Named("Agent")),
                ],
                check: Some(Box::new(SkillsAudit)),
                observer: None,
            },
        ];
        Self { modules }
    }

    /// Every module applicable to the given event/tool, in registration order.
    #[must_use]
    pub fn applicable(&self, trigger: Trigger, tool: Option<&str>) -> Vec<&Module> {
        self.modules
            .iter()
            .filter(|m| m.matches(trigger, tool))
            .collect()
    }

    /// The module with the given id, regardless of event/tool — used by
    /// `mustard-rt check <id>`.
    #[must_use]
    pub fn by_id(&self, id: &str) -> Option<&Module> {
        self.modules.iter().find(|m| m.id == id)
    }
}

impl Default for Registry {
    fn default() -> Self {
        Self::new()
    }
}

/// The enforcement [`Mode`] for a module id.
///
/// Wave 1 keeps this minimal: every module defaults to [`Mode::Strict`], the
/// same default the JS hooks use for an unset `MUSTARD_*_MODE` variable. A
/// later wave wires the full `EnforcementConfig` resolution (`mustard.json` +
/// env) through here; the dispatcher already honours whatever `Mode` it gets.
#[must_use]
pub fn mode_for(_id: &str) -> Mode {
    Mode::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The ids of every module applicable to the given event/tool.
    fn applicable_ids(
        registry: &Registry,
        trigger: Trigger,
        tool: Option<&str>,
    ) -> Vec<&'static str> {
        registry
            .applicable(trigger, tool)
            .iter()
            .map(|m| m.id)
            .collect()
    }

    #[test]
    fn bash_guard_applies_to_bash_events() {
        let registry = Registry::new();
        // `bash_guard` is the Bash-tool gate for both Pre- and PostToolUse.
        assert!(applicable_ids(&registry, Trigger::PreToolUse, Some("Bash"))
            .contains(&"bash_guard"));
        assert!(applicable_ids(&registry, Trigger::PostToolUse, Some("Bash"))
            .contains(&"bash_guard"));
        // It does not apply to a Write tool or a bare lifecycle event.
        assert!(!applicable_ids(&registry, Trigger::PreToolUse, Some("Write"))
            .contains(&"bash_guard"));
    }

    #[test]
    fn wildcard_counters_apply_to_every_pre_tool_use() {
        let registry = Registry::new();
        // `tool_use_counter` / `main_context_counter` use `ToolMatch::Any` —
        // they fire on PreToolUse for any tool (the JS `.*` matcher).
        for tool in ["Bash", "Write", "Read", "Task"] {
            let ids = applicable_ids(&registry, Trigger::PreToolUse, Some(tool));
            assert!(ids.contains(&"tool_use_counter"), "missing for {tool}");
            assert!(ids.contains(&"main_context_counter"), "missing for {tool}");
        }
    }

    #[test]
    fn task_family_applies_on_pre_tool_use_task() {
        let registry = Registry::new();
        let ids = applicable_ids(&registry, Trigger::PreToolUse, Some("Task"));
        for want in ["budget", "model_routing", "subagent_tracker", "skills_audit"] {
            assert!(ids.contains(&want), "missing {want}");
        }
    }

    #[test]
    fn subagent_lifecycle_runs_only_the_counters() {
        let registry = Registry::new();
        // `SubagentStart` (a non-tool event) → only the two counters apply.
        let ids = applicable_ids(&registry, Trigger::SubagentStart, None);
        assert!(ids.contains(&"tool_use_counter"));
        assert!(ids.contains(&"main_context_counter"));
        assert!(!ids.contains(&"bash_guard"));
    }

    #[test]
    fn skill_post_tool_use_runs_skill_usage_tracker() {
        let registry = Registry::new();
        let ids = applicable_ids(&registry, Trigger::PostToolUse, Some("Skill"));
        assert!(ids.contains(&"skill_usage_tracker"));
    }

    #[test]
    fn by_id_finds_registered_modules() {
        let registry = Registry::new();
        for id in [
            "bash_guard",
            "budget",
            "model_routing",
            "tool_use_counter",
            "main_context_counter",
            "subagent_tracker",
            "metrics_tracker",
            "skill_usage_tracker",
            "skills_audit",
        ] {
            assert!(registry.by_id(id).is_some(), "by_id missing {id}");
        }
        assert!(registry.by_id("nonexistent").is_none());
    }
}
