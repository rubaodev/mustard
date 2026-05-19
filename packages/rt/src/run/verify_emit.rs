//! `mustard-rt run verify-emit` — a port of `scripts/verify-emit.js`.
//!
//! Confirms that a named event was emitted to the harness bus
//! (`.claude/.harness/events.jsonl`) within a recent time window. Used by the
//! orchestrator after an "emit-and-continue" step to catch a silently-failed
//! emit instead of trusting the emitter's fail-open semantics blindly.
//!
//! Scans the log backward — the most-recent match wins an early exit. Exit `0`
//! on a match, `1` on no match within the window, `2` on bad arguments
//! (the JS contract).

use crate::run::env;
use serde_json::Value;
use std::path::PathBuf;

/// Parse a duration string (`30s`, `1m`, `500ms`, `2h`, or a bare ms integer)
/// into milliseconds. Defaults to `30_000` on an empty/invalid value, exactly
/// like the JS `parseDuration`.
fn parse_duration(s: &str) -> i64 {
    let s = s.trim();
    if s.is_empty() {
        return 30_000;
    }
    let parse_prefix = |suffix: &str| -> Option<i64> {
        s.strip_suffix(suffix)
            .and_then(|n| n.parse::<i64>().ok())
    };
    if let Some(n) = parse_prefix("ms") {
        return n;
    }
    if let Some(n) = parse_prefix("s") {
        return n * 1000;
    }
    if let Some(n) = parse_prefix("m") {
        return n * 60_000;
    }
    if let Some(n) = parse_prefix("h") {
        return n * 3_600_000;
    }
    s.parse::<i64>().unwrap_or(30_000)
}

/// `verify-emit` argument bundle.
struct Args {
    event: String,
    since_ms: i64,
    payload_key: Option<String>,
    payload_value: Option<String>,
    spec: Option<String>,
    quiet: bool,
}

/// The outcome of a verification scan — maps directly to a process exit code.
#[derive(Debug, PartialEq, Eq)]
enum VerifyOutcome {
    /// A matching event was found `age_secs` seconds ago.
    Found { age_secs: i64 },
    /// No matching event within the window.
    Miss,
}

/// Scan the parsed event lines (newest-last) for a match within the window.
///
/// `now_ms` is injected so the scan is deterministic under test. Returns
/// [`VerifyOutcome::Found`] for the first (newest) match, else `Miss`.
fn scan(lines: &[String], args: &Args, now_ms: i64) -> VerifyOutcome {
    let cutoff = now_ms - args.since_ms;
    for line in lines.iter().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(ev) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if ev.get("event").and_then(Value::as_str) != Some(args.event.as_str()) {
            continue;
        }
        if let Some(spec) = &args.spec {
            if ev.get("spec").and_then(Value::as_str) != Some(spec.as_str()) {
                continue;
            }
        }
        let Some(ts) = ev.get("ts").and_then(Value::as_str) else {
            continue;
        };
        let Some(ts_ms) = crate::run::complete_spec::parse_iso_millis(ts) else {
            continue;
        };
        if ts_ms < cutoff {
            // Scanning backward — anything earlier is also out of window.
            break;
        }
        if let Some(key) = &args.payload_key {
            let payload_val = ev.get("payload").and_then(|p| p.get(key));
            let Some(payload_val) = payload_val else {
                continue;
            };
            if let Some(want) = &args.payload_value {
                let got = match payload_val {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                if &got != want {
                    continue;
                }
            }
        }
        let age_secs = (now_ms - ts_ms) / 1000;
        return VerifyOutcome::Found { age_secs };
    }
    VerifyOutcome::Miss
}

/// Dispatch `mustard-rt run verify-emit`.
pub fn run(
    event: Option<&str>,
    since: Option<&str>,
    payload_key: Option<&str>,
    payload_value: Option<&str>,
    spec: Option<&str>,
    quiet: bool,
) {
    let Some(event) = event.filter(|e| !e.is_empty()) else {
        eprintln!("error: --event required");
        std::process::exit(2);
    };
    let args = Args {
        event: event.to_string(),
        since_ms: since.map_or(30_000, parse_duration),
        payload_key: payload_key.map(str::to_string),
        payload_value: payload_value.map(str::to_string),
        spec: spec.map(str::to_string),
        quiet,
    };

    let project_dir = PathBuf::from(env::project_dir());
    let file = project_dir
        .join(".claude")
        .join(".harness")
        .join("events.jsonl");
    let Ok(raw) = std::fs::read_to_string(&file) else {
        if !args.quiet {
            eprintln!("[verify-emit] events.jsonl not found: {}", file.display());
        }
        std::process::exit(1);
    };

    let lines: Vec<String> = raw.lines().map(str::to_string).collect();
    let now_ms = crate::util::now_millis() as i64;
    match scan(&lines, &args, now_ms) {
        VerifyOutcome::Found { age_secs } => {
            if !args.quiet {
                let spec_note = args
                    .spec
                    .as_ref()
                    .map(|s| format!(" (spec={s})"))
                    .unwrap_or_default();
                println!("[verify-emit] OK: {} {age_secs}s ago{spec_note}", args.event);
            }
            std::process::exit(0);
        }
        VerifyOutcome::Miss => {
            if !args.quiet {
                let win_sec = args.since_ms / 1000;
                let spec_note = args
                    .spec
                    .as_ref()
                    .map(|s| format!(" (spec={s})"))
                    .unwrap_or_default();
                eprintln!(
                    "[verify-emit] MISS: {} not found in last {win_sec}s{spec_note}",
                    args.event
                );
            }
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(event: &str) -> Args {
        Args {
            event: event.to_string(),
            since_ms: 30_000,
            payload_key: None,
            payload_value: None,
            spec: None,
            quiet: true,
        }
    }

    #[test]
    fn parse_duration_units() {
        assert_eq!(parse_duration("30s"), 30_000);
        assert_eq!(parse_duration("1m"), 60_000);
        assert_eq!(parse_duration("500ms"), 500);
        assert_eq!(parse_duration("2h"), 7_200_000);
        assert_eq!(parse_duration("750"), 750);
        assert_eq!(parse_duration(""), 30_000);
        assert_eq!(parse_duration("garbage"), 30_000);
    }

    #[test]
    fn scan_finds_recent_event() {
        let lines = vec![
            r#"{"event":"close-gate.check","ts":"2026-05-19T00:00:00.000Z"}"#.to_string(),
        ];
        let now = crate::run::complete_spec::parse_iso_millis("2026-05-19T00:00:05.000Z").unwrap();
        let r = scan(&lines, &args("close-gate.check"), now);
        assert_eq!(r, VerifyOutcome::Found { age_secs: 5 });
    }

    #[test]
    fn scan_misses_old_event() {
        let lines = vec![
            r#"{"event":"close-gate.check","ts":"2026-05-19T00:00:00.000Z"}"#.to_string(),
        ];
        // 10 minutes later, default 30s window.
        let now = crate::run::complete_spec::parse_iso_millis("2026-05-19T00:10:00.000Z").unwrap();
        assert_eq!(scan(&lines, &args("close-gate.check"), now), VerifyOutcome::Miss);
    }

    #[test]
    fn scan_respects_payload_filter() {
        let lines = vec![
            r#"{"event":"qa","ts":"2026-05-19T00:00:00.000Z","payload":{"result":"fail"}}"#.to_string(),
            r#"{"event":"qa","ts":"2026-05-19T00:00:01.000Z","payload":{"result":"pass"}}"#.to_string(),
        ];
        let now = crate::run::complete_spec::parse_iso_millis("2026-05-19T00:00:02.000Z").unwrap();
        let mut a = args("qa");
        a.payload_key = Some("result".to_string());
        a.payload_value = Some("pass".to_string());
        assert_eq!(scan(&lines, &a, now), VerifyOutcome::Found { age_secs: 1 });
        a.payload_value = Some("skip".to_string());
        assert_eq!(scan(&lines, &a, now), VerifyOutcome::Miss);
    }
}
