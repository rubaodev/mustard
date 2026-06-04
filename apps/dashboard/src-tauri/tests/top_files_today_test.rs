//! Onda 1 (spec `dashboard-sqlite-out-telemetria-ndjson`): the dead SQLite
//! `db.rs` facade was deleted. The Wave-6B variant of this suite exercised
//! `db::with_db` + `aggregate_activity_from_db`; both are gone. The live
//! `dashboard_workspace_summary` path keeps `top_files_today` from the
//! mustard-core NDJSON projection. Here we just assert the public `FileCount`
//! shape survives the migration. A session-agnostic NDJSON file ranking is
//! Onda 2.

use mustard_dashboard_lib::telemetry_agg::FileCount;

#[test]
fn file_count_shape() {
    let f = FileCount {
        path: String::from("src/lib.rs"),
        count: 3,
    };
    assert_eq!(f.path, "src/lib.rs");
    assert_eq!(f.count, 3);
}
