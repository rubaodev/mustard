//! `mustard-rt run metrics` — a port of `scripts/metrics.js`.
//!
//! A unified CLI for pipeline + hook metrics:
//!
//! - `collect [--hooks-only]` — render the full pipeline + hook-event report.
//! - `report [--since <ISO>] [--event <type>]` — render the hook-event table.
//!
//! Port note: the JS `report --compare` mode resolved git tags via `git show`
//! and is omitted here (a later wave can add it); `--since` / `--event` filters
//! are ported. The `_rtk-gain.js` shell-out is also omitted — RTK analytics are
//! advisory and the JS gain helper is itself a fail-open `rtk gain` shell-out.
//!
//! `--format json` (default) prints a structured JSON document; `--format
//! html` additionally writes a standalone HTML report and prints its path on
//! stderr. The JS script printed markdown; the JSON form is the new default
//! contract for the Rust port (markdown is a human concern, JSON is consumable).

use crate::report::{table, Report};
use serde_json::{json, Map, Value};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Event → category, mirroring `EVENT_CATEGORY` in `metrics.js`.
fn event_category(event: &str) -> &'static str {
    match event {
        "auto-format" | "checklist-auto-mark" | "skill-size-gate" | "spec-size-gate" => "workflow",
        "bash-safety" | "budget-check" | "close-gate" | "enforce-registry" | "review-gate"
        | "skill-validate-gate" | "tool-use-counter" | "duplication-check" | "convention-check"
        | "file-guard" | "guard-verify" | "followup-cancel-gate" => "prevention",
        "bash-native-redirect" => "redirection",
        "memory-auto-extract" | "pre-compact" | "session-memory" | "context-lazy-load"
        | "skill-filter" | "refs-filter" | "spec-hygiene-move" => "extraction",
        "model-routing-gate" => "routing",
        "delegation" => "isolation",
        "rtk-rewrite" => "rtk",
        "output-budget" | "recommended-skills-audit" => "routing-advisory",
        "qa" | "review" => "verification",
        _ => "other",
    }
}

/// Whether `tokens_saved` should be trusted for an event (JS `ALWAYS_TRUSTED_EVENTS`).
fn token_trusted(event: &str) -> bool {
    matches!(
        event,
        "memory-auto-extract"
            | "pre-compact"
            | "spec-hygiene-move"
            | "budget-check"
            | "session-memory"
            | "context-lazy-load"
            | "skill-filter"
            | "refs-filter"
    )
}

/// One aggregated event bucket.
#[derive(Default)]
struct EventAgg {
    count: i64,
    tokens_affected: i64,
    tokens_saved: i64,
    notes: BTreeMap<String, i64>,
}

/// Aggregate every `.jsonl` line under `.claude/.metrics/` into per-event buckets.
fn aggregate_metrics(
    metrics_dir: &Path,
    since: Option<&str>,
    event_filter: Option<&str>,
) -> BTreeMap<String, EventAgg> {
    let mut agg: BTreeMap<String, EventAgg> = BTreeMap::new();
    let Ok(entries) = std::fs::read_dir(metrics_dir) else {
        return agg;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".jsonl") {
            continue;
        }
        let Ok(content) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(v) = serde_json::from_str::<Value>(line) else {
                continue;
            };
            let Some(event) = v.get("event").and_then(Value::as_str) else {
                continue;
            };
            if let Some(f) = event_filter {
                if event != f {
                    continue;
                }
            }
            if let Some(s) = since {
                if let Some(ts) = v.get("ts").and_then(Value::as_str) {
                    if ts < s {
                        continue;
                    }
                }
            }
            let bucket = agg.entry(event.to_string()).or_default();
            bucket.count += 1;
            if let Some(n) = v.get("tokens_affected").and_then(Value::as_i64) {
                bucket.tokens_affected += n;
            }
            if event != "rtk-rewrite" {
                if let Some(n) = v.get("tokens_saved").and_then(Value::as_i64) {
                    bucket.tokens_saved += n;
                }
            }
            if let Some(note) = v.get("note").and_then(Value::as_str) {
                if !note.is_empty() {
                    *bucket.notes.entry(note.to_string()).or_insert(0) += 1;
                }
            }
        }
    }
    agg
}

/// Serialize the aggregation into the JSON `byEvent` document.
fn agg_to_json(agg: &BTreeMap<String, EventAgg>) -> Value {
    let mut by_event = Map::new();
    let (mut total_count, mut total_saved, mut total_affected) = (0i64, 0i64, 0i64);
    for (event, b) in agg {
        let trusted = token_trusted(event);
        let saved = if trusted && event != "rtk-rewrite" { b.tokens_saved } else { 0 };
        by_event.insert(
            event.clone(),
            json!({
                "count": b.count,
                "category": event_category(event),
                "tokensAffected": b.tokens_affected,
                "tokensSaved": saved,
                "notes": b.notes.iter().map(|(k, v)| (k.clone(), json!(v))).collect::<Map<_, _>>(),
            }),
        );
        total_count += b.count;
        total_saved += saved;
        total_affected += b.tokens_affected;
    }
    json!({
        "byEvent": by_event,
        "total": { "count": total_count, "tokensSaved": total_saved, "tokensAffected": total_affected },
    })
}

/// Read pipeline-state files into a list of `{ name, metrics }`.
fn collect_specs(claude_dir: &Path) -> Vec<Value> {
    let states_dir = claude_dir.join(".pipeline-states");
    let mut out = Vec::new();
    let Ok(entries) = std::fs::read_dir(&states_dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if !name.ends_with(".json") || name.ends_with(".metrics.json") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(entry.path()) else {
            continue;
        };
        let Ok(state) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        let metrics = state.get("metrics").cloned();
        let Some(metrics) = metrics else { continue };
        let spec_name = name.trim_end_matches(".json").to_string();
        let spec_dir = claude_dir.join("spec").join("active").join(&spec_name);
        out.push(json!({
            "name": spec_name,
            "metrics": metrics,
            "isOrphaned": !spec_dir.exists(),
        }));
    }
    out
}

/// Build the `collect` JSON document.
fn build_collect(cwd: &Path, hooks_only: bool) -> Value {
    let claude_dir = cwd.join(".claude");
    let hook_events = aggregate_metrics(&claude_dir.join(".metrics"), None, None);
    let specs = if hooks_only { Vec::new() } else { collect_specs(&claude_dir) };

    let active: Vec<&Value> = specs.iter().filter(|s| s["isOrphaned"] == json!(false)).collect();
    let orphaned: Vec<&Value> = specs.iter().filter(|s| s["isOrphaned"] == json!(true)).collect();
    let total_specs = specs.len();
    let pass1 = specs
        .iter()
        .filter(|s| s["metrics"].get("retries").and_then(Value::as_i64).unwrap_or(0) == 0)
        .count();

    json!({
        "hookEvents": agg_to_json(&hook_events),
        "pipelines": {
            "tracked": total_specs,
            "active": active.len(),
            "orphaned": orphaned.len(),
            "pass1": pass1,
            "pass1Pct": if total_specs > 0 { (pass1 * 100 / total_specs) as i64 } else { 0 },
            "specs": specs,
        },
    })
}

/// Build the `report` JSON document.
fn build_report(cwd: &Path, since: Option<&str>, event_filter: Option<&str>) -> Value {
    let metrics_dir = cwd.join(".claude").join(".metrics");
    let agg = aggregate_metrics(&metrics_dir, since, event_filter);
    agg_to_json(&agg)
}

/// Write a standalone HTML report wrapping the metrics document.
fn write_html_report(cwd: &Path, subcommand: &str, doc: &Value) -> Option<PathBuf> {
    let dir = cwd.join(".claude").join(".qa-reports");
    std::fs::create_dir_all(&dir).ok()?;
    let mut report = Report::new(format!("Metrics — {subcommand}"), "pipeline + hook telemetry");

    // Render the hook-event table when present.
    let by_event = doc
        .get("hookEvents")
        .and_then(|h| h.get("byEvent"))
        .or_else(|| doc.get("byEvent"))
        .and_then(Value::as_object);
    if let Some(by_event) = by_event {
        let mut rows: Vec<Vec<String>> = by_event
            .iter()
            .map(|(event, e)| {
                vec![
                    event.clone(),
                    e.get("count").and_then(Value::as_i64).unwrap_or(0).to_string(),
                    e.get("category").and_then(Value::as_str).unwrap_or("").to_string(),
                    e.get("tokensSaved").and_then(Value::as_i64).unwrap_or(0).to_string(),
                ]
            })
            .collect();
        rows.sort_by(|a, b| a[0].cmp(&b[0]));
        report.section(
            "Hook Events",
            &table(&["Event", "Count", "Category", "Tokens Saved"], &rows),
        );
    }
    report.pre_section("Raw", &serde_json::to_string_pretty(doc).unwrap_or_default());
    let path = dir.join(format!("metrics-{subcommand}.html"));
    std::fs::write(&path, report.render()).ok()?;
    Some(path)
}

/// Dispatch `mustard-rt run metrics`.
pub fn run(subcommand: Option<&str>, args: &[String], format: &str) {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let (doc, sub) = match subcommand {
        Some("collect") => {
            let hooks_only = args.iter().any(|a| a == "--hooks-only");
            (build_collect(&cwd, hooks_only), "collect")
        }
        Some("report") => {
            let mut since = None;
            let mut event = None;
            let mut i = 0;
            while i < args.len() {
                match args[i].as_str() {
                    "--since" => {
                        since = args.get(i + 1).cloned();
                        i += 1;
                    }
                    "--event" => {
                        event = args.get(i + 1).cloned();
                        i += 1;
                    }
                    _ => {}
                }
                i += 1;
            }
            (build_report(&cwd, since.as_deref(), event.as_deref()), "report")
        }
        _ => {
            eprintln!("Usage:");
            eprintln!("  metrics collect [--hooks-only] [--format json|html]");
            eprintln!("  metrics report [--since <ISO>] [--event <type>] [--format json|html]");
            return;
        }
    };

    if format == "html" {
        match write_html_report(&cwd, sub, &doc) {
            Some(path) => eprintln!("[metrics] HTML report: {}", path.display()),
            None => eprintln!("[metrics] WARN: could not write HTML report"),
        }
    }
    println!("{}", serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".to_string()));
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write_metric(dir: &Path, event: &str, line: &str) {
        let m = dir.join(".claude").join(".metrics");
        std::fs::create_dir_all(&m).unwrap();
        let path = m.join(format!("{event}.jsonl"));
        let existing = std::fs::read_to_string(&path).unwrap_or_default();
        std::fs::write(&path, format!("{existing}{line}\n")).unwrap();
    }

    #[test]
    fn report_aggregates_events() {
        let dir = tempdir().unwrap();
        write_metric(dir.path(), "qa", r#"{"event":"qa","note":"pass","ts":"2026-05-19T00:00:00Z"}"#);
        write_metric(dir.path(), "qa", r#"{"event":"qa","note":"fail","ts":"2026-05-19T01:00:00Z"}"#);
        let doc = build_report(dir.path(), None, None);
        assert_eq!(doc["byEvent"]["qa"]["count"], json!(2));
        assert_eq!(doc["byEvent"]["qa"]["category"], json!("verification"));
    }

    #[test]
    fn report_since_filter_excludes_old() {
        let dir = tempdir().unwrap();
        write_metric(dir.path(), "qa", r#"{"event":"qa","ts":"2026-05-01T00:00:00Z"}"#);
        write_metric(dir.path(), "qa", r#"{"event":"qa","ts":"2026-05-19T00:00:00Z"}"#);
        let doc = build_report(dir.path(), Some("2026-05-10T00:00:00Z"), None);
        assert_eq!(doc["byEvent"]["qa"]["count"], json!(1));
    }

    #[test]
    fn html_report_is_standalone() {
        let dir = tempdir().unwrap();
        write_metric(dir.path(), "budget-check", r#"{"event":"budget-check","ts":"2026-05-19T00:00:00Z"}"#);
        let doc = build_report(dir.path(), None, None);
        let path = write_html_report(dir.path(), "report", &doc).unwrap();
        let html = std::fs::read_to_string(path).unwrap();
        assert!(html.starts_with("<!doctype html>"));
        assert!(!html.contains("href=") && !html.contains("src="));
        assert!(html.contains("budget-check"));
    }
}
