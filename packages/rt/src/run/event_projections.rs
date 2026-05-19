//! `mustard-rt run event-projections` — a port of `scripts/event-projections.js`.
//!
//! Read-only projections over the harness event log
//! (`.claude/.harness/events.jsonl`). Each view derives a JSON document from
//! the parsed events; the CLI prints it to stdout. Exit `0` always (fail-open).
//!
//! Views ported: `agent-visibility`, `pipeline-state`, `session-summary`,
//! `epic-summary`. The JS `buildSlopeReport` projection is **deliberately not
//! ported** — B3 deleted the `duplication.warn` / `convention.warn` hooks that
//! fed it, so nothing emits those events anymore (b4 spec, dead-code removal).
//! `cross-session-timeline`, `spec-tree` and `pr-metrics` remain JS-only views
//! a later wave can port; an unknown `--view` returns `{ "error": ... }`.
//!
//! `--format json` (default) prints the projection. `--format html` wraps the
//! same JSON in a standalone HTML page and prints its path on stderr.

use crate::report::Report;
use mustard_core::io::event_store::JsonlEventStore;
use mustard_core::model::event::HarnessEvent;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// `agent.stop` summary truncation, matching `DEFAULT_AGENT_SUMMARY_CHARS`.
const AGENT_SUMMARY_CHARS: usize = 800;
/// Finding-confidence floor, matching `DEFAULT_FINDING_CONFIDENCE`.
const FINDING_CONFIDENCE: f64 = 0.7;
/// Per-wave event cap, matching `DEFAULT_AGENT_EVENT_LIMIT`.
const AGENT_EVENT_LIMIT: usize = 40;

/// Replay the harness event log under `cwd`.
fn read_events(cwd: &Path) -> Vec<HarnessEvent> {
    JsonlEventStore::for_project(cwd).replay().unwrap_or_default()
}

/// `buildAgentVisibility` — recent events of a wave plus high-confidence
/// findings. If `wave` is `None`, the max wave seen is used.
fn build_agent_visibility(events: &[HarnessEvent], wave: Option<u32>) -> Value {
    let wave = wave.unwrap_or_else(|| events.iter().map(|e| e.wave).max().unwrap_or(0));

    let mut wave_events: Vec<Value> = Vec::new();
    let mut findings: Vec<&HarnessEvent> = Vec::new();
    for ev in events {
        if ev.wave == wave {
            wave_events.push(truncate_summary(ev));
        }
        if ev.event == "finding" {
            let conf = ev.payload.get("confidence").and_then(Value::as_f64).unwrap_or(0.0);
            if conf >= FINDING_CONFIDENCE {
                findings.push(ev);
            }
        }
    }
    // Sort findings: confidence desc, then ts desc.
    findings.sort_by(|a, b| {
        let ca = a.payload.get("confidence").and_then(Value::as_f64).unwrap_or(0.0);
        let cb = b.payload.get("confidence").and_then(Value::as_f64).unwrap_or(0.0);
        cb.partial_cmp(&ca)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| b.ts.cmp(&a.ts))
    });
    // Dedup findings by the first 60 chars of normalised content.
    let mut seen = std::collections::HashSet::new();
    let mut deduped: Vec<Value> = Vec::new();
    for f in findings {
        let content = f.payload.get("content").and_then(Value::as_str).unwrap_or("");
        let key: String = content
            .to_lowercase()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(60)
            .collect();
        if seen.insert(key) {
            deduped.push(serde_json::to_value(f).unwrap_or(Value::Null));
        }
    }
    // Keep the most recent events within the limit.
    if wave_events.len() > AGENT_EVENT_LIMIT {
        wave_events.drain(..wave_events.len() - AGENT_EVENT_LIMIT);
    }
    json!({ "wave": wave, "events": wave_events, "findings": deduped })
}

/// Truncate an `agent.stop` event's `payload.summary`, leaving others as-is.
fn truncate_summary(ev: &HarnessEvent) -> Value {
    let mut value = serde_json::to_value(ev).unwrap_or(Value::Null);
    if ev.event == "agent.stop" {
        if let Some(summary) = ev.payload.get("summary").and_then(Value::as_str) {
            if summary.chars().count() > AGENT_SUMMARY_CHARS {
                let cut: String = summary.chars().take(AGENT_SUMMARY_CHARS).collect();
                if let Some(p) = value.get_mut("payload").and_then(Value::as_object_mut) {
                    p.insert("summary".to_string(), json!(format!("{cut}…")));
                }
            }
        }
    }
    value
}

/// `buildPipelineState` — current phase + dispatch failures + metrics.
fn build_pipeline_state(events: &[HarnessEvent], spec: Option<&str>) -> Value {
    let mut phase: Option<String> = None;
    let mut last_event_at: Option<String> = None;
    let mut started_at: Option<String> = None;
    let mut dispatch_failures: Vec<Value> = Vec::new();
    let mut decisions: Vec<Value> = Vec::new();
    let mut lessons: Vec<Value> = Vec::new();
    let mut api_calls = 0i64;
    let mut tool_breakdown: serde_json::Map<String, Value> = serde_json::Map::new();
    let mut agent_count = 0i64;
    let mut failures_by_phase: serde_json::Map<String, Value> = serde_json::Map::new();

    for ev in events {
        if let Some(s) = spec {
            if ev.spec.as_deref() != Some(s) {
                continue;
            }
        }
        if !ev.ts.is_empty() {
            if started_at.is_none() {
                started_at = Some(ev.ts.clone());
            }
            last_event_at = Some(ev.ts.clone());
        }
        match ev.event.as_str() {
            "pipeline.phase" => {
                if let Some(to) = ev.payload.get("to").and_then(Value::as_str) {
                    phase = Some(to.to_string());
                } else if let Some(from) = ev.payload.get("from").and_then(Value::as_str) {
                    phase = Some(from.to_string());
                }
            }
            "dispatch.failure" => {
                dispatch_failures.push(serde_json::to_value(ev).unwrap_or(Value::Null));
                let ph = ev
                    .payload
                    .get("phase")
                    .and_then(Value::as_str)
                    .unwrap_or("UNKNOWN")
                    .to_string();
                let n = failures_by_phase.get(&ph).and_then(Value::as_i64).unwrap_or(0);
                failures_by_phase.insert(ph, json!(n + 1));
            }
            "decision" => decisions.push(serde_json::to_value(ev).unwrap_or(Value::Null)),
            "lesson" => lessons.push(serde_json::to_value(ev).unwrap_or(Value::Null)),
            "tool.use" => {
                let tool = ev.payload.get("tool").and_then(Value::as_str).unwrap_or("unknown");
                if tool != "Read" {
                    api_calls += 1;
                    let n = tool_breakdown.get(tool).and_then(Value::as_i64).unwrap_or(0);
                    tool_breakdown.insert(tool.to_string(), json!(n + 1));
                }
            }
            "agent.start" => agent_count += 1,
            _ => {}
        }
    }

    json!({
        "spec": spec,
        "phase": phase,
        "lastEventAt": last_event_at,
        "dispatchFailures": dispatch_failures,
        "decisions": decisions,
        "lessons": lessons,
        "metrics": {
            "apiCalls": api_calls,
            "toolBreakdown": tool_breakdown,
            "retries": failures_by_phase.values().filter_map(Value::as_i64).sum::<i64>(),
            "agentCount": agent_count,
            "startedAt": started_at,
            "dispatchFailuresByPhase": failures_by_phase,
        },
    })
}

/// `buildSessionSummary` — roll-up over a whole session's events.
fn build_session_summary(events: &[HarnessEvent]) -> Value {
    let mut session_id: Option<String> = None;
    let mut started_at: Option<String> = None;
    let mut ended_at: Option<String> = None;
    let mut agent_count = 0i64;
    let mut tool_count = 0i64;
    let mut findings: Vec<Value> = Vec::new();
    let mut decisions: Vec<Value> = Vec::new();
    let mut lessons: Vec<Value> = Vec::new();
    let mut specs = std::collections::BTreeSet::new();

    for ev in events {
        if session_id.is_none() && !ev.session_id.is_empty() {
            session_id = Some(ev.session_id.clone());
        }
        if !ev.ts.is_empty() {
            if started_at.is_none() {
                started_at = Some(ev.ts.clone());
            }
            ended_at = Some(ev.ts.clone());
        }
        if let Some(s) = &ev.spec {
            specs.insert(s.clone());
        }
        match ev.event.as_str() {
            "agent.start" => agent_count += 1,
            "tool.use" => tool_count += 1,
            "finding" => findings.push(serde_json::to_value(ev).unwrap_or(Value::Null)),
            "decision" => decisions.push(serde_json::to_value(ev).unwrap_or(Value::Null)),
            "lesson" => lessons.push(serde_json::to_value(ev).unwrap_or(Value::Null)),
            _ => {}
        }
    }
    json!({
        "sessionId": session_id,
        "startedAt": started_at,
        "endedAt": ended_at,
        "agentCount": agent_count,
        "toolCount": tool_count,
        "specs": specs.into_iter().collect::<Vec<_>>(),
        "findings": findings,
        "decisions": decisions,
        "lessons": lessons,
    })
}

/// `buildEpicSummary` — derive a summary view for an epic and its children.
fn build_epic_summary(events: &[HarnessEvent], cwd: &Path, epic: &str) -> Value {
    let states_dir = cwd.join(".claude").join(".pipeline-states");
    let read_state = |name: &str| -> Option<Value> {
        std::fs::read_to_string(states_dir.join(format!("{name}.json")))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
    };
    let root_state = read_state(epic);
    let children: Vec<String> = root_state
        .as_ref()
        .and_then(|s| s.get("children_specs"))
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_str).map(str::to_string).collect())
        .unwrap_or_default();

    let children_info: Vec<Value> = children
        .iter()
        .map(|c| {
            let phase = read_state(c)
                .and_then(|s| {
                    s.get("phaseName")
                        .or_else(|| s.get("phase"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
            json!({ "spec": c, "phase": phase })
        })
        .collect();

    let root_phase = root_state
        .as_ref()
        .and_then(|s| s.get("phaseName").or_else(|| s.get("phase")).and_then(Value::as_str))
        .unwrap_or("")
        .to_uppercase();

    let mut spec_set: std::collections::BTreeSet<&str> = children.iter().map(String::as_str).collect();
    spec_set.insert(epic);

    let mut findings: Vec<Value> = Vec::new();
    let mut decisions: Vec<Value> = Vec::new();
    let mut lessons: Vec<Value> = Vec::new();
    let (mut tool_calls, mut agents) = (0i64, 0i64);
    let (mut min_ts, mut max_ts): (Option<String>, Option<String>) = (None, None);
    let mut folded = root_phase == "CLOSE";

    for ev in events {
        if ev.event == "epic.fold"
            && ev.payload.get("epic").and_then(Value::as_str) == Some(epic)
        {
            folded = true;
        }
        let Some(spec) = ev.spec.as_deref() else { continue };
        if !spec_set.contains(spec) {
            continue;
        }
        if !ev.ts.is_empty() {
            if min_ts.as_deref().map(|m| ev.ts.as_str() < m).unwrap_or(true) {
                min_ts = Some(ev.ts.clone());
            }
            if max_ts.as_deref().map(|m| ev.ts.as_str() > m).unwrap_or(true) {
                max_ts = Some(ev.ts.clone());
            }
        }
        match ev.event.as_str() {
            "finding" => findings.push(serde_json::to_value(ev).unwrap_or(Value::Null)),
            "decision" => decisions.push(serde_json::to_value(ev).unwrap_or(Value::Null)),
            "lesson" => lessons.push(serde_json::to_value(ev).unwrap_or(Value::Null)),
            "tool.use" => tool_calls += 1,
            "agent.start" => agents += 1,
            _ => {}
        }
    }
    let duration_ms = match (
        min_ts.as_deref().and_then(crate::run::complete_spec::parse_iso_millis),
        max_ts.as_deref().and_then(crate::run::complete_spec::parse_iso_millis),
    ) {
        (Some(a), Some(b)) => (b - a).max(0),
        _ => 0,
    };
    json!({
        "epic": epic,
        "children": children_info,
        "findings": findings,
        "decisions": decisions,
        "lessons": lessons,
        "metrics": {
            "toolCallsTotal": tool_calls,
            "agentsTotal": agents,
            "durationMs": duration_ms,
            "startedAt": min_ts,
            "endedAt": max_ts,
        },
        "folded": folded,
    })
}

/// Compute the projection for a `--view`.
fn project(cwd: &Path, view: &str, spec: Option<&str>, wave: Option<u32>) -> Value {
    match view {
        "agent-visibility" => build_agent_visibility(&read_events(cwd), wave),
        "pipeline-state" => build_pipeline_state(&read_events(cwd), spec),
        "session-summary" => build_session_summary(&read_events(cwd)),
        "epic-summary" => match spec {
            Some(s) => build_epic_summary(&read_events(cwd), cwd, s),
            None => json!({ "error": "--spec is required for epic-summary view" }),
        },
        other => json!({ "error": format!("Unknown view: {other}") }),
    }
}

/// Write the standalone HTML report wrapping the projection JSON.
fn write_html_report(cwd: &Path, view: &str, json_text: &str) -> Option<PathBuf> {
    let dir = cwd.join(".claude").join(".qa-reports");
    std::fs::create_dir_all(&dir).ok()?;
    let mut report = Report::new(format!("Event Projection — {view}"), "harness event log view");
    report.pre_section("Projection", json_text);
    let path = dir.join(format!("event-projection-{view}.html"));
    std::fs::write(&path, report.render()).ok()?;
    Some(path)
}

/// Dispatch `mustard-rt run event-projections`.
pub fn run(view: Option<&str>, spec: Option<&str>, wave: Option<u32>, format: &str) {
    let Some(view) = view else {
        eprintln!("Usage: event-projections --view <name> [--spec <name>] [--wave <n>] [--format json|html]");
        eprintln!("Views: agent-visibility, pipeline-state, session-summary, epic-summary");
        return;
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let result = project(&cwd, view, spec, wave);
    let json_text = serde_json::to_string_pretty(&result).unwrap_or_else(|_| "{}".to_string());

    if format == "html" {
        match write_html_report(&cwd, view, &json_text) {
            Some(path) => eprintln!("[event-projections] HTML report: {}", path.display()),
            None => eprintln!("[event-projections] WARN: could not write HTML report"),
        }
    }
    println!("{json_text}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustard_core::model::event::{Actor, ActorKind, SCHEMA_VERSION};

    fn ev(event: &str, spec: Option<&str>, payload: Value) -> HarnessEvent {
        HarnessEvent {
            v: SCHEMA_VERSION,
            ts: "2026-05-19T00:00:00.000Z".to_string(),
            session_id: "s1".to_string(),
            wave: 0,
            actor: Actor { kind: ActorKind::Hook, id: None, actor_type: None },
            event: event.to_string(),
            payload,
            spec: spec.map(str::to_string),
        }
    }

    #[test]
    fn pipeline_state_counts_tool_use_and_phase() {
        let events = vec![
            ev("pipeline.phase", Some("demo"), json!({ "to": "EXECUTE" })),
            ev("tool.use", Some("demo"), json!({ "tool": "Edit" })),
            ev("tool.use", Some("demo"), json!({ "tool": "Read" })),
        ];
        let v = build_pipeline_state(&events, Some("demo"));
        assert_eq!(v["phase"], json!("EXECUTE"));
        // Read is excluded from apiCalls.
        assert_eq!(v["metrics"]["apiCalls"], json!(1));
    }

    #[test]
    fn session_summary_collects_specs_and_counts() {
        let events = vec![
            ev("agent.start", Some("a"), json!({})),
            ev("finding", Some("b"), json!({ "content": "x" })),
        ];
        let v = build_session_summary(&events);
        assert_eq!(v["agentCount"], json!(1));
        assert_eq!(v["specs"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn unknown_view_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let v = project(dir.path(), "slope-report", None, None);
        assert!(v.get("error").is_some());
    }
}
