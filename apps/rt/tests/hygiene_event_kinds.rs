//! Integration test: `emit-pipeline` accepts the three W5 `hygiene.*` event
//! kinds (`spec-lifecycle-unification` Wave 5).
//!
//! These are first-class new kinds (no legacy alias), so each `emit-pipeline
//! --kind hygiene.*` writes exactly one row and exits 0. An unknown kind still
//! exits 1 (the validation contract is unchanged).

use mustard_core::store::event_store::EventSink;
use mustard_core::store::sqlite_store::SqliteEventStore;
use std::path::Path;
use tempfile::TempDir;

fn project_dir() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".claude").join(".harness")).expect("harness dir");
    dir
}

fn emit(project: &Path, kind: &str, spec: &str, payload: &str) -> std::process::Output {
    let bin = env!("CARGO_BIN_EXE_mustard-rt");
    std::process::Command::new(bin)
        .args(["run", "emit-pipeline", "--kind", kind, "--spec", spec, "--payload", payload])
        .current_dir(project)
        .env("CLAUDE_PROJECT_DIR", project.to_string_lossy().as_ref())
        .output()
        .expect("run mustard-rt")
}

#[test]
fn hygiene_kinds_are_accepted_and_write_single_rows() {
    let tmp = project_dir();
    let project = tmp.path();
    let spec = "hygiene-kinds";

    for kind in ["hygiene.detected", "hygiene.autoclose", "hygiene.skipped"] {
        let out = emit(project, kind, spec, r#"{"spec":"hygiene-kinds"}"#);
        assert!(
            out.status.success(),
            "emit-pipeline --kind {kind} must exit 0. stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    let store = SqliteEventStore::for_project(project).expect("open store");
    let events = store.query(Some(spec)).expect("query");
    for kind in ["hygiene.detected", "hygiene.autoclose", "hygiene.skipped"] {
        let n = events.iter().filter(|e| e.event == kind).count();
        assert_eq!(n, 1, "exactly one {kind} row (no alias fan-out)");
    }
}

#[test]
fn unknown_kind_still_rejected() {
    let tmp = project_dir();
    let out = emit(tmp.path(), "hygiene.bogus", "x", "null");
    assert!(
        !out.status.success(),
        "an unknown kind must still exit non-zero (validation contract)"
    );
}
