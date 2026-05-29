//! Post-execute REVIEW/QA gate (2026-05-25 deep-refactor follow-up).
//!
//! When all waves are done (`currentWave >= totalWaves`) — or, in non-wave
//! mode, when stage is `Close` — the orchestrator must NOT freelance into
//! `pipeline.complete`. This module inspects the per-spec REVIEW + QA event
//! state and surfaces an explicit `nextAction` (with companion fields) on the
//! DTO. Fail-open: if the events dir is unreadable we take the conservative
//! path → `ReviewPending`.

use super::ResumeBootstrap;
use mustard_core::io::fs as mfs;
use std::path::Path;

/// True when the spec has finished EXECUTE (all declared waves are done, or
/// the non-wave spec reached `Close` stage).
pub(super) fn execute_complete(out: &ResumeBootstrap) -> bool {
    if out.is_wave_plan {
        out.total_waves > 0 && out.current_wave >= out.total_waves
    } else {
        out.stage.as_deref() == Some("Close")
    }
}

/// Read the spec's per-spec NDJSON event log and return `(qa_pass, has_review,
/// review_rejected)`.
///
/// - `qa_pass` — last `qa.result` has `overall == "pass"`.
/// - `has_review` — at least one `review.result` event exists for the spec.
/// - `review_rejected` — the most recent `review.result` has
///   `verdict == "rejected"`.
fn read_review_qa_state(spec_dir: &Path) -> (bool, bool, bool) {
    let events_dir = spec_dir.join(".events");
    let mut events =
        mustard_core::view::projection::read_harness_events_from_ndjson_dir(&events_dir);
    events.sort_by(|a, b| a.ts.cmp(&b.ts));

    let mut last_qa_overall: Option<String> = None;
    let mut has_review = false;
    let mut last_review_verdict: Option<String> = None;
    for ev in &events {
        match ev.event.as_str() {
            "qa.result" => {
                last_qa_overall = ev
                    .payload
                    .get("overall")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
            "review.result" => {
                has_review = true;
                last_review_verdict = ev
                    .payload
                    .get("verdict")
                    .and_then(|v| v.as_str())
                    .map(str::to_string);
            }
            _ => {}
        }
    }
    let qa_pass = last_qa_overall.as_deref() == Some("pass");
    let review_rejected = last_review_verdict.as_deref() == Some("rejected");
    (qa_pass, has_review, review_rejected)
}

/// Roles to dispatch REVIEW agents for. Order of preference:
/// 1. Roles declared in the spec's `review/spec.md` (if a `## Roles` section
///    exists) — out of scope for this wave; reserved for a future enhancement.
/// 2. The union of `wave-N-{role}` dir suffixes (deduplicated, sorted).
/// 3. A fallback `["mixed"]` when no waves declare a role.
fn derive_review_roles(spec_dir: &Path) -> Vec<String> {
    let Ok(entries) = mfs::read_dir(spec_dir) else {
        return vec!["mixed".to_string()];
    };
    let mut roles: Vec<String> = Vec::new();
    for entry in entries {
        if !entry.is_dir {
            continue;
        }
        let name = &entry.file_name;
        let Some(rest) = name.strip_prefix("wave-") else {
            continue;
        };
        let digit_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
        if digit_end == 0 {
            continue;
        }
        let after = &rest[digit_end..];
        let Some(role) = after.strip_prefix('-') else {
            continue;
        };
        if role.is_empty() {
            continue;
        }
        if !roles.iter().any(|r| r == role) {
            roles.push(role.to_string());
        }
    }
    if roles.is_empty() {
        return vec!["mixed".to_string()];
    }
    roles.sort();
    roles
}

/// Surface the post-execute next action on `out`. When `execute_complete` is
/// false this is a no-op — the orchestrator is still mid-execute and no signal
/// is needed.
pub(super) fn apply_post_execute_gate(
    _project: &Path,
    spec: &str,
    spec_dir: &Path,
    out: &mut ResumeBootstrap,
) {
    if !execute_complete(out) {
        return;
    }
    // Read REVIEW + QA state from the per-spec NDJSON log.
    let (qa_pass, has_review, review_rejected) = read_review_qa_state(spec_dir);

    if qa_pass {
        // Everything green — safe to close.
        out.stage = Some("Close".to_string());
        out.next_action = Some("emit-complete".to_string());
        return;
    }
    if has_review && !review_rejected {
        // REVIEW landed (and not rejected), but QA hasn't passed yet → run QA.
        out.stage = Some("QaPending".to_string());
        out.next_action = Some("run-qa".to_string());
        out.qa_command = Some(format!("mustard-rt run qa-run --spec {spec}"));
        return;
    }
    // No REVIEW yet, OR REVIEW was rejected → dispatch REVIEW agents.
    out.stage = Some("ReviewPending".to_string());
    out.next_action = Some("dispatch-review".to_string());
    out.review_roles = derive_review_roles(spec_dir);
}

#[cfg(test)]
mod tests {
    use super::super::ResumeBootstrap;
    use super::*;

    /// Seed a `.events/<sid>.ndjson` line under the spec dir directly — bypasses
    /// the writer so tests stay hermetic.
    fn write_event_line(spec_dir: &Path, kind: &str, payload: &str, ts: &str) {
        let events_dir = spec_dir.join(".events");
        std::fs::create_dir_all(&events_dir).unwrap();
        let line = format!(
            "{{\"ts\":\"{ts}\",\"event\":\"{kind}\",\"kind\":\"qa\",\"spec\":\"demo\",\"payload\":{payload}}}\n"
        );
        let path = events_dir.join("test.ndjson");
        let prev = std::fs::read_to_string(&path).unwrap_or_default();
        std::fs::write(&path, prev + &line).unwrap();
    }

    /// `execute_complete` is `true` once `currentWave >= totalWaves` in a
    /// wave-plan spec.
    #[test]
    fn execute_complete_true_when_all_waves_done() {
        let mut out = ResumeBootstrap {
            is_wave_plan: true,
            current_wave: 13,
            total_waves: 13,
            ..Default::default()
        };
        assert!(execute_complete(&out));
        out.current_wave = 12;
        assert!(!execute_complete(&out));
    }

    /// All waves done + no events → `ReviewPending` + `dispatch-review` +
    /// reviewRoles derived from wave subdirs.
    #[test]
    fn post_execute_gate_signals_review_pending_when_no_events() {
        let dir = tempfile::tempdir().unwrap();
        let spec_dir = dir.path();
        // Two wave subdirs declaring `rt` and `cli` roles.
        std::fs::create_dir_all(spec_dir.join("wave-0-rt")).unwrap();
        std::fs::create_dir_all(spec_dir.join("wave-1-cli")).unwrap();

        let mut out = ResumeBootstrap {
            is_wave_plan: true,
            current_wave: 2,
            total_waves: 2,
            ..Default::default()
        };
        apply_post_execute_gate(dir.path(), "demo", spec_dir, &mut out);

        assert_eq!(out.stage.as_deref(), Some("ReviewPending"));
        assert_eq!(out.next_action.as_deref(), Some("dispatch-review"));
        assert_eq!(out.review_roles, vec!["cli".to_string(), "rt".to_string()]);
        assert!(out.qa_command.is_none());
    }

    /// Approved REVIEW + no QA → `QaPending` + `run-qa` + qaCommand.
    #[test]
    fn post_execute_gate_signals_qa_pending_after_approved_review() {
        let dir = tempfile::tempdir().unwrap();
        let spec_dir = dir.path();
        write_event_line(
            spec_dir,
            "review.result",
            r#"{"verdict":"approved","spec":"demo"}"#,
            "2026-05-25T10:00:00.000Z",
        );

        let mut out = ResumeBootstrap {
            is_wave_plan: true,
            current_wave: 5,
            total_waves: 5,
            ..Default::default()
        };
        apply_post_execute_gate(dir.path(), "demo", spec_dir, &mut out);

        assert_eq!(out.stage.as_deref(), Some("QaPending"));
        assert_eq!(out.next_action.as_deref(), Some("run-qa"));
        assert_eq!(
            out.qa_command.as_deref(),
            Some("mustard-rt run qa-run --spec demo")
        );
        assert!(out.review_roles.is_empty());
    }

    /// Passing QA → `Close` + `emit-complete`.
    #[test]
    fn post_execute_gate_allows_close_when_qa_passed() {
        let dir = tempfile::tempdir().unwrap();
        let spec_dir = dir.path();
        write_event_line(
            spec_dir,
            "review.result",
            r#"{"verdict":"approved","spec":"demo"}"#,
            "2026-05-25T10:00:00.000Z",
        );
        write_event_line(
            spec_dir,
            "qa.result",
            r#"{"overall":"pass","spec":"demo","criteria":[]}"#,
            "2026-05-25T10:05:00.000Z",
        );

        let mut out = ResumeBootstrap {
            is_wave_plan: true,
            current_wave: 5,
            total_waves: 5,
            ..Default::default()
        };
        apply_post_execute_gate(dir.path(), "demo", spec_dir, &mut out);

        assert_eq!(out.stage.as_deref(), Some("Close"));
        assert_eq!(out.next_action.as_deref(), Some("emit-complete"));
    }

    /// Rejected REVIEW (regardless of staleness) → `ReviewPending` again.
    #[test]
    fn post_execute_gate_returns_to_review_when_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let spec_dir = dir.path();
        std::fs::create_dir_all(spec_dir.join("wave-0-mixed")).unwrap();
        write_event_line(
            spec_dir,
            "review.result",
            r#"{"verdict":"rejected","spec":"demo"}"#,
            "2026-05-25T10:00:00.000Z",
        );

        let mut out = ResumeBootstrap {
            is_wave_plan: true,
            current_wave: 1,
            total_waves: 1,
            ..Default::default()
        };
        apply_post_execute_gate(dir.path(), "demo", spec_dir, &mut out);

        assert_eq!(out.stage.as_deref(), Some("ReviewPending"));
        assert_eq!(out.next_action.as_deref(), Some("dispatch-review"));
        assert_eq!(out.review_roles, vec!["mixed".to_string()]);
    }

    /// Mid-execute (currentWave < totalWaves) → gate is a no-op; no nextAction.
    #[test]
    fn post_execute_gate_is_noop_mid_execute() {
        let dir = tempfile::tempdir().unwrap();
        let mut out = ResumeBootstrap {
            is_wave_plan: true,
            current_wave: 3,
            total_waves: 5,
            stage: Some("Execute".to_string()),
            ..Default::default()
        };
        apply_post_execute_gate(dir.path(), "demo", dir.path(), &mut out);
        assert!(out.next_action.is_none());
        assert_eq!(out.stage.as_deref(), Some("Execute"));
    }

    /// `derive_review_roles` falls back to `["mixed"]` when no wave dirs exist.
    #[test]
    fn derive_review_roles_falls_back_to_mixed() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(derive_review_roles(dir.path()), vec!["mixed".to_string()]);
    }
}
