//! `mustard-rt run close-pipeline` — composite CLOSE face: review verdicts +
//! QA + (only on QA pass) the terminal complete + summary, in one report.
//!
//! Composes, **in-process** (module-qualified, no subprocess):
//!
//! 1. **Reviews (advisory)** — every `review.result` event found in the spec's
//!    per-spec NDJSON log, listed chronologically. Purely informational: a
//!    rejected review does not block this composite (the REVIEW loop owns the
//!    retry policy).
//! 2. **QA** — [`crate::commands::review::qa_run::run_qa_with_options`]
//!    (`self_invoked: true`, since this process IS `mustard-rt` and an AC may
//!    try to rebuild it). Emits the `qa.result` event exactly like the
//!    standalone `qa-run`.
//! 3. **Complete + summary — only with `overall == "pass"`** —
//!    [`crate::commands::spec::complete_spec::finalize`] (the QA-less tail of
//!    `run_complete`; QA already ran above) then
//!    [`crate::commands::pipeline::pipeline_summary::build_for_dir`].
//!
//! QA `fail` **or** `skip` → `completed: false`, `summary: null`, and the
//! failed/skipped ACs stay visible in `qa.criteria` — the spec is NOT closed.
//! No bypass is passed anywhere: the `pipeline.complete` QA gate inside
//! `emit-pipeline` remains the standing authority for any later close attempt.
//!
//! ## Output
//!
//! ```json
//! {
//!   "completed": true,
//!   "qa": { "overall": "pass", "criteria": [...] },
//!   "reviews": [ { "critical": 0, "subproject": null, "verdict": "approved" } ],
//!   "summary": { "done": [...], "left": [...], "nextSteps": [...], "followUps": [...] }
//! }
//! ```
//!
//! Keys serialize sorted (serde_json default map). The report carries no
//! timestamps; `qa.criteria` keeps the standalone `qa-run` per-criterion shape
//! (which includes each AC's measured `duration_ms`, the existing contract).

use crate::commands::pipeline::{dispatch_plan, pipeline_summary};
use crate::commands::review::qa_run::{self, QaRunOptions};
use crate::commands::spec::complete_spec;
use mustard_core::io::claude_paths::ClaudePaths;
use mustard_core::view::projection::read_harness_events_from_ndjson_dir;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

/// CLI entry — `mustard-rt run close-pipeline --spec <slug>`.
pub fn run(spec: &str) {
    let cwd = PathBuf::from(crate::shared::context::project_dir());
    let report = close(&cwd, spec);
    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_else(|_| "{}".to_string())
    );
}

/// The composite miolo against an explicit `cwd` root (testable without
/// mutating the process cwd). Returns the report Value [`run`] prints.
pub(crate) fn close(cwd: &Path, spec: &str) -> Value {
    // 1. Reviews — advisory listing of every review.result verdict.
    let reviews = collect_review_verdicts(cwd, spec);

    // 2. QA — the in-process run (emits qa.result, writes the sidecar/report).
    let qa = qa_run::run_qa_with_options(cwd, spec, QaRunOptions { self_invoked: true });
    let qa_json = json!({
        "overall": qa.overall,
        "criteria": qa_run::criteria_json(&qa.criteria),
    });

    // 3. Only a hard pass closes. `skip` (no AC / nothing ran) is NOT a pass
    //    here — an unverified spec must not be finalized by the composite.
    let (completed, summary) = if qa.overall == "pass" {
        let complete_value = complete_spec::finalize(cwd, spec);
        let ok = complete_value
            .get("ok")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        let spec_dir = dispatch_plan::resolve_spec_dir(cwd, spec);
        // Advisory: an unreadable spec.md degrades the summary to null without
        // un-completing the close.
        let summary = pipeline_summary::build_for_dir(&spec_dir)
            .map(|(model, _header)| pipeline_summary::model_json(&model))
            .unwrap_or(Value::Null);
        (ok, summary)
    } else {
        (false, Value::Null)
    };

    json!({
        "completed": completed,
        "qa": qa_json,
        "reviews": reviews,
        "summary": summary,
    })
}

/// Every `review.result` verdict recorded for `spec`, chronological. Advisory
/// — the composite lists them verbatim (verdict / critical count /
/// subproject); it never blocks on a rejection. Fail-open: a missing events
/// dir yields `[]`.
fn collect_review_verdicts(cwd: &Path, spec: &str) -> Vec<Value> {
    let events_dir = ClaudePaths::for_project(cwd)
        .and_then(|p| p.for_spec(spec))
        .ok()
        .map_or_else(
            || {
                ClaudePaths::compose_unchecked(cwd)
                    .spec_dir()
                    .join(spec)
                    .join(".events")
            },
            |sp| sp.events_dir(),
        );
    let mut events = read_harness_events_from_ndjson_dir(&events_dir);
    events.sort_by(|a, b| a.ts.cmp(&b.ts));
    events
        .into_iter()
        .filter(|e| e.event == "review.result" && e.spec.as_deref() == Some(spec))
        .map(|e| {
            json!({
                "verdict": e.payload.get("verdict").cloned().unwrap_or(Value::Null),
                "critical": e.payload.get("criticalCount").cloned().unwrap_or(Value::Null),
                "subproject": e.payload.get("subproject").cloned().unwrap_or(Value::Null),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustard_core::domain::model::event::{Actor, ActorKind, HarnessEvent, SCHEMA_VERSION};
    use serde_json::json;
    use tempfile::tempdir;

    /// Anchor a project root so `ClaudePaths::for_project` resolves.
    fn anchor(dir: &Path) {
        std::fs::create_dir_all(dir.join(".claude")).unwrap();
        std::fs::write(dir.join("mustard.json"), b"{}").unwrap();
    }

    /// Seed a flat spec whose single AC runs `cmd`.
    fn seed_spec(project: &Path, slug: &str, cmd: &str) -> PathBuf {
        let spec_dir = project.join(".claude").join("spec").join(slug);
        std::fs::create_dir_all(&spec_dir).unwrap();
        std::fs::write(
            spec_dir.join("spec.md"),
            format!(
                "# {slug}\n\n## Acceptance Criteria\n- [ ] AC-1: it runs — Command: `{cmd}`\n"
            ),
        )
        .unwrap();
        std::fs::write(
            spec_dir.join("meta.json"),
            r#"{"stage":"Execute","outcome":"Active","phase":"EXECUTE","scope":"light","lang":"en-US"}"#,
        )
        .unwrap();
        spec_dir
    }

    /// Emit a `review.result` event for `spec` (the same shape `review-result`
    /// records).
    fn emit_review(project: &Path, spec: &str, verdict: &str, critical: i64, ts: &str) {
        let event = HarnessEvent {
            v: SCHEMA_VERSION,
            ts: ts.to_string(),
            session_id: "test-session".to_string(),
            wave: 0,
            actor: Actor {
                kind: ActorKind::Cli,
                id: Some("review-result".to_string()),
                actor_type: None,
            },
            event: "review.result".to_string(),
            payload: json!({
                "spec": spec,
                "verdict": verdict,
                "criticalCount": critical,
                "subproject": null,
            }),
            spec: Some(spec.to_string()),
        };
        crate::shared::events::route::emit(project.to_str().unwrap(), &event);
    }

    /// Happy path: a passing AC closes the spec — reviews listed, QA pass,
    /// `completed: true`, summary present, and the spec's projection + sidecar
    /// land on completed.
    #[test]
    fn composite_close_pipeline_pass_completes_and_summarises() {
        let dir = tempdir().unwrap();
        anchor(dir.path());
        let project = dir.path();
        let spec = "close-pass";
        // `echo ok` exits 0 under both `cmd /c` and `sh -c`.
        let spec_dir = seed_spec(project, spec, "echo ok");
        emit_review(project, spec, "approved", 0, "2026-06-09T00:00:01.000Z");

        let report = close(project, spec);

        assert_eq!(report["qa"]["overall"], json!("pass"), "{report}");
        assert_eq!(report["completed"], json!(true), "{report}");
        // Reviews listed verbatim, advisory.
        assert_eq!(report["reviews"][0]["verdict"], json!("approved"), "{report}");
        assert_eq!(report["reviews"][0]["critical"], json!(0), "{report}");
        // Summary carries the json-format shape.
        assert!(report["summary"]["done"].is_array(), "{report}");
        assert!(report["summary"]["nextSteps"].is_array(), "{report}");

        // The close really landed: meta.json flipped to Close/Completed.
        let meta: Value = serde_json::from_str(
            &std::fs::read_to_string(spec_dir.join("meta.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(meta["stage"], json!("Close"), "{meta}");
        assert_eq!(meta["outcome"], json!("Completed"), "{meta}");
    }

    /// Degraded: a failing AC reports the reproved criterion and does NOT
    /// close — `completed: false`, no summary, sidecar untouched.
    #[test]
    fn composite_close_pipeline_qa_fail_does_not_close() {
        let dir = tempdir().unwrap();
        anchor(dir.path());
        let project = dir.path();
        let spec = "close-fail";
        // `exit 3` exits non-zero under both `cmd /c` and `sh -c`.
        let spec_dir = seed_spec(project, spec, "exit 3");
        emit_review(project, spec, "rejected", 2, "2026-06-09T00:00:01.000Z");

        let report = close(project, spec);

        assert_eq!(report["qa"]["overall"], json!("fail"), "{report}");
        assert_eq!(report["completed"], json!(false), "{report}");
        assert_eq!(report["summary"], Value::Null, "no summary on a failed QA");
        // The reproved AC is named in the criteria.
        assert_eq!(report["qa"]["criteria"][0]["id"], json!("AC-1"), "{report}");
        assert_eq!(report["qa"]["criteria"][0]["status"], json!("fail"), "{report}");
        // Reviews stay advisory (the rejected verdict is listed, not acted on).
        assert_eq!(report["reviews"][0]["verdict"], json!("rejected"), "{report}");

        // NOT closed: sidecar still mid-pipeline.
        let meta: Value = serde_json::from_str(
            &std::fs::read_to_string(spec_dir.join("meta.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(meta["stage"], json!("Execute"), "{meta}");
        assert_eq!(meta["outcome"], json!("Active"), "{meta}");
        // And no pipeline.complete event landed.
        assert!(!crate::commands::event::verify_emit::verify_event_landed(
            project,
            "pipeline.complete",
            Some(spec),
            Some("1h"),
        ));
    }

    /// Degraded: an unknown spec degrades to QA `skip` — which is NOT a pass:
    /// `completed: false`, empty reviews, null summary.
    #[test]
    fn composite_close_pipeline_unknown_spec_skips_without_closing() {
        let dir = tempdir().unwrap();
        anchor(dir.path());
        let report = close(dir.path(), "ghost-spec");
        assert_eq!(report["qa"]["overall"], json!("skip"), "{report}");
        assert_eq!(report["completed"], json!(false), "{report}");
        assert_eq!(report["reviews"], json!([]), "{report}");
        assert_eq!(report["summary"], Value::Null, "{report}");
    }
}
