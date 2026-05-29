//! Resume-mode decision + refresh signal.
//!
//! Two event-driven verdicts:
//! - [`decide_mode`] — `continued` / `reanalyzed` / `ask` from the pipeline
//!   state view + dispatch-failure presence.
//! - [`compute_needs_refresh`] — whether a `pipeline.wave.complete` landed
//!   after the last `pipeline.resume_mode`, plus that resume event's age (used
//!   by [`super::run`] to debounce re-emission).
//!
//! Both fail-open: missing events degrade to the conservative path.

use super::{AUTO_CONTINUE_TTL_MS, PipelineDispatchFailurePayload, PipelineStateView};
use mustard_core::domain::model::event::{
    EVENT_PIPELINE_RESUME_MODE, EVENT_PIPELINE_WAVE_COMPLETE,
};
use mustard_core::io::claude_paths::ClaudePaths;
use mustard_core::io::fs as mfs;
use mustard_core::EventReader;
use std::path::{Path, PathBuf};

/// Returns `(needs_refresh, last_resume_mode_age_ms)`.
///
/// `needs_refresh` is `true` when at least one `pipeline.wave.complete` event
/// landed since the most recent `pipeline.resume_mode` event for this spec.
///
/// Reads from the per-spec NDJSON events dir (`.claude/spec/{spec}/.events/`).
/// Fail-open: an unreadable dir returns `(false, None)`.
pub(super) fn compute_needs_refresh(project: &Path, spec: &str) -> (bool, Option<i64>) {
    let events_dir = match ClaudePaths::for_project(project)
        .ok()
        .and_then(|p| p.for_spec(spec).ok())
        .map(|sp| sp.events_dir())
    {
        Some(d) => d,
        None => return (false, None),
    };

    let now_ms = i64::try_from(mustard_core::time::now_unix_millis() as u128).unwrap_or(i64::MAX);

    // Collect all NDJSON events from the dir.
    let ndjson_files: Vec<PathBuf> = mfs::read_dir(&events_dir)
        .ok()
        .into_iter()
        .flatten()
        .map(|e| e.path)
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ndjson"))
        .collect();

    let mut last_resume_ts: Option<String> = None;
    let mut last_wave_complete_ts: Option<String> = None;

    for path in &ndjson_files {
        for ev in EventReader::stream(path) {
            let ts = ev
                .raw
                .get("ts")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            match ev.kind.as_str() {
                k if k == EVENT_PIPELINE_RESUME_MODE => {
                    if ts.as_deref() > last_resume_ts.as_deref() {
                        last_resume_ts = ts;
                    }
                }
                k if k == EVENT_PIPELINE_WAVE_COMPLETE => {
                    if ts.as_deref() > last_wave_complete_ts.as_deref() {
                        last_wave_complete_ts = ts;
                    }
                }
                _ => {}
            }
        }
    }

    let last_resume_ms = last_resume_ts
        .as_deref()
        .and_then(mustard_core::time::parse_iso_millis);
    let last_resume_age = last_resume_ms.map(|ms| now_ms - ms);

    // Needs refresh when there is a wave.complete that is newer than the last resume_mode.
    let needs = match (last_resume_ts.as_deref(), last_wave_complete_ts.as_deref()) {
        (Some(resume_ts), Some(wave_ts)) => wave_ts > resume_ts,
        (None, Some(_)) => true,
        _ => false,
    };

    (needs, last_resume_age)
}

/// Decide the resume mode from the view + dispatch failure state.
///
/// - `continued` — recent events, no dispatch failure, status is in-progress.
/// - `reanalyzed` — pipeline was abandoned (no events for a while) AND no
///   dispatch failure.
/// - `ask` — dispatch failure present OR no state at all.
pub(super) fn decide_mode(
    view: Option<&PipelineStateView>,
    dispatch_failure: Option<&PipelineDispatchFailurePayload>,
) -> String {
    if dispatch_failure.is_some() {
        return "ask".to_string();
    }
    let Some(v) = view else {
        return "ask".to_string();
    };
    let last_ts = v
        .tasks
        .iter()
        .filter_map(|t| t.dispatched_at.clone())
        .max();
    let now_ms = i64::try_from(mustard_core::time::now_unix_millis() as u128).unwrap_or(i64::MAX);
    let age_ms = last_ts
        .as_deref()
        .and_then(mustard_core::time::parse_iso_millis)
        .map(|at| now_ms - at);
    match age_ms {
        Some(ms) if ms <= AUTO_CONTINUE_TTL_MS => "continued".to_string(),
        Some(_) => "reanalyzed".to_string(),
        // No task dispatch yet — orchestrator decides.
        None => "ask".to_string(),
    }
}
