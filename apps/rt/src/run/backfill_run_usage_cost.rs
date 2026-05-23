//! `mustard-rt run backfill-run-usage-cost` — one-shot retroactive pricing.
//!
//! Tactical fix companion to `price_frame-model-fallback`: that fix changed
//! the write path so new spans without a `model` attribute still get priced
//! (sonnet fallback). This subcommand applies the same policy to legacy
//! rows that were already on disk with `cost_usd_micros = NULL`.
//!
//! Output is a stable JSON object on stdout so the calling shell or the
//! dashboard can confirm what happened:
//!
//! ```json
//! {"rows_scanned": 297, "rows_updated": 294, "db_path": "..."}
//! ```
//!
//! Fail-open contract: opening the telemetry DB or computing pricing never
//! aborts the process. Only a failed UPDATE inside the transaction returns
//! exit 1, because that signals real disk-level corruption the user should
//! see.

use mustard_core::telemetry::{writer, TelemetryStore};
use serde_json::json;

use crate::run::env::project_dir;

/// Run the backfill on the project's telemetry.db. Idempotent.
pub fn run() {
    let cwd = project_dir();

    // Open the store. Fail-open at this layer because a missing DB is a
    // benign "nothing to do" state, not an error worth a non-zero exit.
    let store = match TelemetryStore::for_project(&cwd) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("backfill_run_usage_cost: open telemetry store failed ({e}); skipping");
            println!(
                "{}",
                json!({
                    "rows_scanned": 0,
                    "rows_updated": 0,
                    "db_path": cwd.clone(),
                    "error": e.to_string(),
                })
            );
            return;
        }
    };

    match writer::backfill_null_costs(store.conn()) {
        Ok(report) => {
            println!(
                "{}",
                json!({
                    "rows_scanned": report.scanned,
                    "rows_updated": report.updated,
                    "db_path": cwd,
                })
            );
        }
        Err(e) => {
            // A real write failure — surface it via exit 1 so a caller in
            // a shell pipeline notices.
            eprintln!("backfill_run_usage_cost: UPDATE failed: {e}");
            std::process::exit(1);
        }
    }
}
