//! Outbound events emitted by the bootstrap.
//!
//! - [`emit_scope_for_session`] — fresh `pipeline.scope` so the session's
//!   `current_spec` lookup tracks the resumed spec (not a stale closed one).
//! - [`emit_resume_mode`] — the `pipeline.resume_mode` record carrying the
//!   decided mode (debounced by [`super::run`]).
//!
//! Both fail-open: any router error is silently discarded.

use crate::shared::context::session_id;
use crate::shared::events::route;
use mustard_core::domain::model::event::{
    Actor, ActorKind, HarnessEvent, EVENT_PIPELINE_RESUME_MODE, EVENT_PIPELINE_SCOPE,
    SCHEMA_VERSION,
};
use mustard_core::time::now_iso8601;
use serde_json::json;
use std::path::Path;

/// Emit a `pipeline.resume_mode` event via the NDJSON router (fail-open).
pub(super) fn emit_resume_mode(project: &Path, spec: &str, mode: &str) {
    let event = HarnessEvent {
        v: SCHEMA_VERSION,
        ts: now_iso8601(),
        session_id: session_id(),
        wave: 0,
        actor: Actor {
            kind: ActorKind::Orchestrator,
            id: Some("resume-bootstrap".to_string()),
            actor_type: None,
        },
        event: EVENT_PIPELINE_RESUME_MODE.to_string(),
        payload: json!({ "mode": mode }),
        spec: Some(spec.to_string()),
    };
    let _ = route::emit(project.to_string_lossy().as_ref(), &event);
}

/// Emit a fresh `pipeline.scope` event for the resumed spec via the router so
/// `last_pipeline_scope_for_session` returns this spec in subsequent calls
/// within the same Claude session (prevents stale closed-spec attribution).
///
/// `pipeline.*` events are routed to SQLite by [`route::emit`] — this
/// preserves the existing session-lookup contract without a direct store call.
///
/// Fail-open: any emit error is silently discarded.
pub(super) fn emit_scope_for_session(project: &Path, spec: &str) {
    let event = HarnessEvent {
        v: SCHEMA_VERSION,
        ts: now_iso8601(),
        session_id: session_id(),
        wave: 0,
        actor: Actor {
            kind: ActorKind::Orchestrator,
            id: Some("resume-bootstrap".to_string()),
            actor_type: None,
        },
        event: EVENT_PIPELINE_SCOPE.to_string(),
        payload: json!({ "scope": "resumed" }),
        spec: Some(spec.to_string()),
    };
    let _ = route::emit(project.to_string_lossy().as_ref(), &event);
}
