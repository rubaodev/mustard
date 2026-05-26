//! W11.T11.4 — Tauri command `economy_summary` for the `/economia` page.
//!
//! Wires three sources into one frontend payload:
//!
//! 1. The W5 baseline JSON file (read indirectly via
//!    `mustard-rt run economy report --format json`) — gives the operational
//!    baselines captured during pipeline runs.
//! 2. The W11 `economy_savings` table in `telemetry.db` — gives per-wave token
//!    savings the dashboard renders as a sparkline + per-wave table on the
//!    "Deep Refactor Savings" tab.
//! 3. The W11 `economy_baselines` table — optional context if a baseline was
//!    materialised into SQLite (the reconcile path may upgrade JSON entries
//!    here in a future wave; today the JSON file is the source of truth).
//!
//! Fail-open at every step: a missing `telemetry.db`, a missing binary on
//! `PATH`, malformed JSON — each degrade to a default field rather than an
//! error, so the dashboard never displays a hard failure for a feature the
//! user can ignore.

use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

/// Per-wave token savings row sourced from `telemetry.db.economy_savings`.
#[derive(Serialize, Default, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct WaveSavings {
    /// Wave label as recorded by the writer (e.g. `W0`, `W1`, ..., `W12`).
    pub wave_id: String,
    /// Total `savings_tokens` summed across every operation in this wave.
    pub savings_tokens: i64,
    /// Distinct operations contributing to this wave's savings.
    pub operations: i64,
}

/// One baseline entry as returned by `economy_report::EconomyReport`. We keep
/// the parse lenient — only the fields the dashboard consumes are typed; the
/// rest pass through opaquely so a schema bump on the rt side doesn't break
/// this command.
#[derive(Serialize, Default, Clone, Debug)]
#[serde(rename_all = "snake_case")]
pub struct BaselineEntry {
    pub operation: String,
    pub wave: u32,
    pub captured_at: String,
    pub duration_ms: i64,
    pub from_history: bool,
}

/// Aggregated payload returned by `economy_summary`.
#[derive(Serialize, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub struct EconomySummary {
    /// Sum of `savings_tokens` across every wave (W11.T11.5 headline card).
    pub total_savings_tokens: i64,
    /// Per-wave breakdown, sorted ascending by `wave_id` for the table + the
    /// sparkline. An empty vec is the empty-state signal for the UI.
    pub per_wave: Vec<WaveSavings>,
    /// Operational baselines captured via `mustard-rt run economy report`.
    pub baselines: Vec<BaselineEntry>,
    /// Total number of baseline entries (mirrors the report.total field).
    pub baseline_total: usize,
    /// Best-effort diagnostic — non-empty when the rt CLI couldn't be reached
    /// or the telemetry DB couldn't be opened. The frontend can surface this
    /// in a subtle subtitle without failing the page.
    pub notes: Vec<String>,
}

/// Resolve the telemetry DB path. Mirrors the rt helper so the two stay in
/// lockstep; environment override matches the runtime contract.
fn telemetry_db_path(repo: &Path) -> PathBuf {
    if let Ok(p) = std::env::var("MUSTARD_TELEMETRY_DB_PATH") {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    repo.join(".claude").join(".harness").join("telemetry.db")
}

/// Query `economy_savings` grouped by `wave_id`. Returns an empty vec when
/// the DB is missing, the table is missing, or the query fails — every error
/// path is fail-open.
fn per_wave_from_db(repo: &Path) -> (Vec<WaveSavings>, Option<String>) {
    let path = telemetry_db_path(repo);
    if !path.exists() {
        return (Vec::new(), Some(format!("telemetry.db not found at {}", path.display())));
    }
    let Ok(conn) = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    else {
        return (Vec::new(), Some(format!("cannot open telemetry.db at {}", path.display())));
    };
    let sql = "SELECT wave_id, \
                      COALESCE(SUM(savings_tokens), 0), \
                      COUNT(DISTINCT operation) \
               FROM economy_savings GROUP BY wave_id ORDER BY wave_id ASC";
    let Ok(mut stmt) = conn.prepare(sql) else {
        return (Vec::new(), Some("economy_savings query failed".to_string()));
    };
    let rows = stmt.query_map([], |r| {
        Ok(WaveSavings {
            wave_id: r.get::<_, String>(0)?,
            savings_tokens: r.get::<_, i64>(1)?,
            operations: r.get::<_, i64>(2)?,
        })
    });
    match rows {
        Ok(it) => (it.filter_map(std::result::Result::ok).collect(), None),
        Err(_) => (Vec::new(), Some("economy_savings rows failed".to_string())),
    }
}

/// Shell to `mustard-rt run economy report --format json` and parse stdout.
/// Returns `(entries, total)`. Fail-open per the spawn / parse layer.
fn baselines_from_rt(repo: &Path) -> (Vec<BaselineEntry>, usize, Option<String>) {
    let args: &[&str] = &["run", "economy", "report", "--format", "json"];
    let mut cmd = mustard_rt_command(args);
    cmd.current_dir(repo);
    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => return (Vec::new(), 0, Some(format!("spawn mustard-rt: {e}"))),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json = slice_json(&stdout);
    let parsed: Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(_) => return (Vec::new(), 0, Some("malformed economy report json".to_string())),
    };
    let total = parsed.get("total").and_then(Value::as_u64).unwrap_or(0) as usize;
    let entries: Vec<BaselineEntry> = parsed
        .get("entries")
        .and_then(Value::as_array)
        .map(|arr| {
            arr.iter()
                .filter_map(|e| {
                    Some(BaselineEntry {
                        operation: e.get("operation").and_then(Value::as_str)?.to_string(),
                        wave: e.get("wave").and_then(Value::as_u64).unwrap_or(0) as u32,
                        captured_at: e
                            .get("captured_at")
                            .and_then(Value::as_str)
                            .unwrap_or("")
                            .to_string(),
                        duration_ms: e.get("duration_ms").and_then(Value::as_i64).unwrap_or(0),
                        from_history: e
                            .get("from_history")
                            .and_then(Value::as_bool)
                            .unwrap_or(false),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    (entries, total, None)
}

/// `economy_summary` — Tauri command. Returns the merged W11 payload.
///
/// Fail-open: the function never returns `Err` for missing data; it surfaces
/// degradation through `EconomySummary::notes` so the dashboard can render a
/// subtle hint without breaking the page.
#[tauri::command]
pub fn economy_summary(repo_path: String) -> Result<EconomySummary, String> {
    let repo = PathBuf::from(&repo_path);
    let mut notes: Vec<String> = Vec::new();

    let (per_wave, note_db) = per_wave_from_db(&repo);
    if let Some(n) = note_db {
        notes.push(n);
    }
    let total_savings_tokens: i64 = per_wave.iter().map(|w| w.savings_tokens).sum();

    let (baselines, baseline_total, note_rt) = baselines_from_rt(&repo);
    if let Some(n) = note_rt {
        notes.push(n);
    }

    Ok(EconomySummary {
        total_savings_tokens,
        per_wave,
        baselines,
        baseline_total,
        notes,
    })
}

// ── helpers shared with the rest of the dashboard ────────────────────────────

/// Build a `Command` that invokes `mustard-rt`. Windows uses `cmd /C` so PATH
/// resolution matches every other dashboard caller (see `spec_views.rs`).
fn mustard_rt_command(args: &[&str]) -> std::process::Command {
    #[cfg(target_os = "windows")]
    {
        let mut c = crate::process_util::no_window_command("cmd");
        let mut full: Vec<&str> = vec!["/C", "mustard-rt"];
        full.extend_from_slice(args);
        c.args(&full);
        c
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut c = crate::process_util::no_window_command("mustard-rt");
        c.args(args);
        c
    }
}

/// Trim leading RTK / log noise so `serde_json::from_str` sees a clean JSON
/// document starting at the first `{`.
fn slice_json(stdout: &str) -> &str {
    match stdout.find('{') {
        Some(i) => &stdout[i..],
        None => stdout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn missing_db_returns_empty_per_wave_with_note() {
        let dir = tempdir().unwrap();
        let (rows, note) = per_wave_from_db(dir.path());
        assert!(rows.is_empty());
        assert!(note.is_some(), "expected a note when telemetry.db is absent");
    }

    #[test]
    fn per_wave_aggregates_savings_and_operations() {
        let dir = tempdir().unwrap();
        // Materialise a minimal telemetry.db at the expected path.
        let harness = dir.path().join(".claude").join(".harness");
        std::fs::create_dir_all(&harness).unwrap();
        let db_path = harness.join("telemetry.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE economy_savings (\
               wave_id TEXT NOT NULL, \
               operation TEXT NOT NULL, \
               savings_tokens INTEGER NOT NULL DEFAULT 0, \
               measured_at INTEGER NOT NULL, \
               PRIMARY KEY (wave_id, operation, measured_at)\
             );",
        )
        .unwrap();
        let now: i64 = 1_700_000_000_000;
        for (w, op, sav) in [
            ("W0", "scan-rust-first", 1000_i64),
            ("W0", "templates-md-moat", 500),
            ("W1", "sub-spec-link", 2000),
        ] {
            conn.execute(
                "INSERT INTO economy_savings (wave_id, operation, savings_tokens, measured_at) \
                 VALUES (?1, ?2, ?3, ?4)",
                rusqlite::params![w, op, sav, now],
            )
            .unwrap();
        }
        drop(conn);

        let (rows, note) = per_wave_from_db(dir.path());
        assert!(note.is_none(), "no note expected on a healthy DB");
        assert_eq!(rows.len(), 2);
        let w0 = rows.iter().find(|r| r.wave_id == "W0").unwrap();
        assert_eq!(w0.savings_tokens, 1500);
        assert_eq!(w0.operations, 2);
        let w1 = rows.iter().find(|r| r.wave_id == "W1").unwrap();
        assert_eq!(w1.savings_tokens, 2000);
        assert_eq!(w1.operations, 1);
    }

    #[test]
    fn summary_total_is_sum_of_per_wave() {
        let rows = vec![
            WaveSavings {
                wave_id: "W0".to_string(),
                savings_tokens: 100,
                operations: 1,
            },
            WaveSavings {
                wave_id: "W1".to_string(),
                savings_tokens: 250,
                operations: 2,
            },
        ];
        let total: i64 = rows.iter().map(|r| r.savings_tokens).sum();
        assert_eq!(total, 350);
    }

    #[test]
    fn baseline_entry_serializes_required_fields() {
        let e = BaselineEntry {
            operation: "verify".to_string(),
            wave: 1,
            captured_at: "T".to_string(),
            duration_ms: 42,
            from_history: true,
        };
        let v = serde_json::to_value(e).unwrap();
        for f in ["operation", "wave", "captured_at", "duration_ms", "from_history"] {
            assert!(v.get(f).is_some(), "missing field {f}");
        }
    }
}
