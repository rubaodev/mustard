//! `mustard-rt run backfill-run-usage-cost` — one-shot retroactive pricing.
//!
//! Applies the shared `mustard_core::economy::estimator::compute_cost_micros`
//! helper to historical `run_usage` rows that were written before the current
//! pricing path landed. Two modes:
//!
//! - Default (`--force` absent): only NULL/0 cost rows are touched. Idempotent.
//!   Use after adding a new pricing branch (e.g. a new model id) so previously
//!   unpriced rows get a number.
//!
//! - `--force`: recompute every row carrying any non-zero token bucket,
//!   overwriting prior cost values. Use after the *formula* changes — e.g.
//!   the 2026-05-23 cache-aware split (cache_read now billed at 10%, not 100%
//!   of input rate), where historical rows have a wrong but non-zero number.
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

/// Run the backfill on the project's telemetry.db.
///
/// `force = false` is idempotent (NULL/0 cost rows only). `force = true`
/// recomputes every row with any non-zero token bucket, overwriting prior
/// `cost_usd_micros` values — required after a pricing-formula change.
pub fn run(force: bool) {
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
                    "force": force,
                    "error": e.to_string(),
                })
            );
            return;
        }
    };

    match writer::backfill_null_costs(store.conn(), force) {
        Ok(report) => {
            println!(
                "{}",
                json!({
                    "rows_scanned": report.scanned,
                    "rows_updated": report.updated,
                    "db_path": cwd,
                    "force": force,
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
