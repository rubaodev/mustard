//! `shared::events::economy` — the single home for emitting `pipeline.economy.*`
//! telemetry events.
//!
//! Before this module ~31 modules each carried a verbatim `emit_economy` /
//! `emit_economy_operation` / `emit_economy_event` copy that built the same
//! [`HarnessEvent`] envelope (actor, ts, session, spec, route) and only varied
//! the operation name + payload. This is the permanent single owner: a
//! call-site is now one line and there are no per-module copies.

use crate::shared::context::{current_spec, session_id};
use crate::shared::events::route;
use mustard_core::domain::model::event::{Actor, ActorKind, HarnessEvent, SCHEMA_VERSION};
use serde_json::{json, Value};

/// Emit an arbitrary `pipeline.economy.*` event: build the canonical envelope
/// (`v`/`ts`/`session_id`/`actor`/`spec`) and route it through
/// [`route::emit`]. Fail-open — telemetry never blocks the caller.
///
/// `cwd` is the directory route resolution uses to locate `.claude/`. `spec` is
/// resolved via [`current_spec`] from that `cwd`.
pub fn emit(
    cwd: &str,
    actor: ActorKind,
    actor_id: &str,
    event_name: &str,
    spec: Option<&str>,
    payload: Value,
) {
    // `spec`: an explicit override (the caller already knows the spec); `None`
    // falls back to resolving it from `cwd`.
    let spec = spec
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .or_else(|| current_spec(cwd));
    let ev = HarnessEvent {
        v: SCHEMA_VERSION,
        ts: mustard_core::time::now_iso8601(),
        session_id: session_id(),
        wave: 0,
        actor: Actor {
            kind: actor,
            id: Some(actor_id.to_string()),
            actor_type: None,
        },
        event: event_name.to_string(),
        payload,
        spec,
    };
    let _ = route::emit(cwd, &ev);
}

/// Emit the dominant `pipeline.economy.operation.invoked` event for a
/// Rust-native operation. Builds the common payload
/// (`operation` / `duration_ms` / `tokens_used: 0` / `was_rust_only: true`) and
/// merges any operation-specific `extra` object on top, then delegates to
/// [`emit`] with the operation name as the actor id.
pub fn emit_operation(
    cwd: &str,
    actor: ActorKind,
    operation: &str,
    duration_ms: u64,
    spec: Option<&str>,
    extra: Value,
) {
    let mut payload = json!({
        "operation": operation,
        "duration_ms": i64::try_from(duration_ms).unwrap_or(i64::MAX),
        "tokens_used": 0,
        "was_rust_only": true,
    });
    if let (Some(obj), Some(ex)) = (payload.as_object_mut(), extra.as_object()) {
        for (k, v) in ex {
            obj.insert(k.clone(), v.clone());
        }
    }
    emit(cwd, actor, operation, "pipeline.economy.operation.invoked", spec, payload);
}
