// Integration tests are separate binary targets and not exempt from
// `clippy::unwrap_used` etc. via `#[cfg(test)]`. Mirror the carve-out from
// `src/main.rs` so test panics on `.unwrap()` remain valid assertions.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::map_unwrap_or,
    clippy::uninlined_format_args
)]

//! Integration tests for `pipeline-state-ingest` + the
//! `pipeline_state_from_events` projection.
//!
//! ## History
//!
//! - Wave 1 (no-sqlite migration, W2A): the `pipeline-state-ingest`
//!   subcommand was reduced to a no-op stub once pipeline state moved to
//!   NDJSON events. The original integration tests verified the
//!   `.pipeline-states/*.json` → SQLite ingest path, which no longer exists.
//! - W8A-3 (no-sqlite Wave 8): the production-shape projection assertions
//!   were preserved by feeding NDJSON events directly into
//!   `pipeline_state_from_events` via
//!   [`mustard_core::projection::read_workspace_events`]. Two contract
//!   assertions stay in this file:
//!     - the ingest subcommand emits the canonical empty-run JSON shape;
//!     - `pipeline_state_from_events` over an NDJSON-seeded workspace
//!       returns `Some(view)` for a known spec, exercising the same fold
//!       the resume/active-spec readers consume.

use mustard_core::model::event::HarnessEvent;
use mustard_rt::run::event_projections::pipeline_state_from_events;
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Test helpers
// ---------------------------------------------------------------------------

fn project_dir() -> TempDir {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".claude").join(".harness")).expect("harness dir");
    dir
}

/// Run `mustard-rt run pipeline-state-ingest [--delete]` against `dir` and
/// return the parsed JSON output.
fn run_ingest(dir: &Path, delete: bool) -> Value {
    let bin = env!("CARGO_BIN_EXE_mustard-rt");
    let mut cmd = Command::new(bin);
    cmd.args(["run", "pipeline-state-ingest"]);
    if delete {
        cmd.arg("--delete");
    }
    cmd.env("CLAUDE_PROJECT_DIR", dir.to_string_lossy().as_ref());
    let out = cmd.output().expect("run mustard-rt");
    let stdout = String::from_utf8_lossy(&out.stdout);
    serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| json!({ "parse_error": e.to_string(), "raw": stdout.as_ref() }))
}

/// Append one NDJSON event under `<dir>/.claude/spec/<spec>/.events/seed.ndjson`.
fn append_event(dir: &Path, spec: &str, event_name: &str, ts: &str, payload: Value) {
    let events_dir = dir.join(".claude").join("spec").join(spec).join(".events");
    std::fs::create_dir_all(&events_dir).unwrap();
    let line = json!({
        "event": event_name,
        "kind": "pipeline",
        "ts": ts,
        "v": 1,
        "spec": spec,
        "session_id": "seed",
        "wave": 0,
        "actor": "test",
        "payload": payload,
    });
    let path = events_dir.join("seed.ndjson");
    let mut body = std::fs::read_to_string(&path).unwrap_or_default();
    body.push_str(&line.to_string());
    body.push('\n');
    std::fs::write(&path, body).unwrap();
}

// ---------------------------------------------------------------------------
// Test 1 — ingest stub returns the canonical empty-run JSON shape
// ---------------------------------------------------------------------------

#[test]
fn ingest_subcommand_emits_canonical_noop_shape() {
    let tmp = project_dir();
    let dir = tmp.path();
    let result = run_ingest(dir, false);
    assert!(
        result.get("parse_error").is_none(),
        "ingest stub must emit parseable JSON: {result}"
    );
    assert_eq!(result["ingested"], json!(0), "no-op ingest reports zero: {result}");
    assert_eq!(result["deleted"], json!(0), "no-op ingest deletes nothing: {result}");
    let errors = result["errors"].as_array().expect("errors array");
    assert!(errors.is_empty(), "no errors expected for no-op ingest: {errors:?}");
}

#[test]
fn ingest_subcommand_with_delete_still_noop() {
    let tmp = project_dir();
    let dir = tmp.path();
    let result = run_ingest(dir, true);
    assert_eq!(result["ingested"], json!(0));
    assert_eq!(result["deleted"], json!(0));
}

// ---------------------------------------------------------------------------
// Test 2 — pipeline_state_from_events folds NDJSON events into a view
// ---------------------------------------------------------------------------

#[test]
fn pipeline_state_projection_reads_ndjson_seeded_workspace() {
    let tmp = project_dir();
    let dir = tmp.path();
    let spec = "ndjson-spec";

    // Seed the per-spec NDJSON log with a scope + status pair.
    append_event(
        dir,
        spec,
        "pipeline.scope",
        "2026-05-20T00:00:00.000Z",
        json!({
            "scope": "full",
            "lang": "en",
            "model": "opus",
            "isWavePlan": true,
            "totalWaves": 3,
        }),
    );
    append_event(
        dir,
        spec,
        "pipeline.status",
        "2026-05-20T00:00:01.000Z",
        json!({ "to": "active" }),
    );

    // Read events back via the same canonical walker the production
    // resume/active-spec readers use and fold via the projection.
    let events: Vec<HarnessEvent> = mustard_core::projection::read_workspace_events(dir);
    assert!(
        events.iter().any(|e| e.event == "pipeline.scope"),
        "scope event must survive the round-trip: {events:?}"
    );

    let view = pipeline_state_from_events(&events, spec, None)
        .expect("pipeline_state_from_events must return a view when scope+status exist");
    // The fold runs without panic and produces a non-empty view; the precise
    // field shape is exercised by the core projection unit tests.
    assert_eq!(view.spec, spec);
}
