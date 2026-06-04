//! Onda 1 (spec `dashboard-sqlite-out-telemetria-ndjson`): the dead SQLite
//! `db.rs` facade was deleted. The Wave-6B variant of this suite asserted that
//! `db::with_db` always returned `None`; with the facade gone, only the public
//! `SpecRow` shape contract remains to verify. Specs are derived from the
//! `.claude/spec/*/` filesystem walk (`dashboard_specs`); the NDJSON-backed
//! phase merge is Onda 2.

use mustard_dashboard_lib::SpecRow;

#[test]
fn spec_row_default_shape() {
    let row = SpecRow {
        name: String::from("spec-x"),
        status: Some(String::from("active")),
        phase: Some(String::from("plan")),
        started_at: None,
        completed_at: None,
        affected_files: Vec::new(),
        bucket: None,
        parent: None,
    };
    assert_eq!(row.name, "spec-x");
    assert_eq!(row.phase.as_deref(), Some("plan"));
}
