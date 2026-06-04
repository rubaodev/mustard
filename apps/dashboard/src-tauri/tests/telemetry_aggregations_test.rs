//! Onda 1 (spec `dashboard-sqlite-out-telemetria-ndjson`): the dead SQLite
//! `db.rs` facade was deleted. The Wave-6B variant of this suite drove every
//! telemetry-agg function through `db::with_db` (which always short-circuited
//! with `None`); both the facade and those `&Connection`-taking aggregators
//! are gone. The telemetry-aggregation Tauri commands now return empty/default
//! payloads directly. Only the JSON shapes survive, so we assert their default
//! contract here. Faithful NDJSON-backed aggregation + its tests land in Onda 2.

use mustard_dashboard_lib::telemetry_agg::EffortBreakdown;

#[test]
fn effort_breakdown_default_is_empty() {
    let e = EffortBreakdown::default();
    assert!(e.top_files.is_empty());
    assert!(e.top_tools.is_empty());
    assert!(e.top_phases.is_empty());
    assert!(e.top_agents.is_empty());
}
