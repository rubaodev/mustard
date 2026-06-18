//! [`project_capabilities`] — durable-capability roll-up over the
//! `capability.*` event channel.
//!
//! Folds [`EVENT_CAPABILITY_DECLARED`] / [`EVENT_CAPABILITY_UPDATE`] /
//! [`EVENT_CAPABILITY_DRIFT`] into a typed [`CapabilityRollup`]:
//! - **state** comes from [`EVENT_CAPABILITY_DECLARED`] — the authoritative,
//!   doc-faithful snapshot (newest declaration wins). The capability DOC is the
//!   single living source, so its snapshot — not a replayed delta sequence — is
//!   what defines the current state. This is robust: a lost `capability.update`
//!   can never desync the state from the doc.
//! - **history** comes from [`EVENT_CAPABILITY_UPDATE`] — each update is the
//!   *computed* per-requirement change-log entry (Added / Modified / Removed),
//!   appended in event order as the reviewable record of how the capability
//!   evolved. Updates do NOT mutate state (the snapshot already reflects them);
//!   they are the audit trail beside it.
//! - **drift** is recorded as `stale` + the set of `orphaned` entities.
//!
//! Like every projection here it is total (always returns *something*),
//! deterministic (same input → same output, byte-stable ordering), reads only
//! event payloads, and never touches the filesystem or panics.

use crate::domain::capability::{
    Capability, CapabilityDeclared, CapabilityDrift, CapabilityUpdate, Requirement, UpdateOp,
    EVENT_CAPABILITY_DECLARED, EVENT_CAPABILITY_DRIFT, EVENT_CAPABILITY_UPDATE,
};
use crate::domain::model::event::HarnessEvent;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One entry in a capability's computed change-log — a single requirement-level
/// change folded from a `capability.update` event. The reviewable trail of how
/// the capability evolved; it does NOT define current state (the `declared`
/// snapshot does).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityChange {
    /// Whether this requirement was added, modified, or removed.
    pub op: UpdateOp,
    /// The requirement the change concerns (the `curr` requirement for
    /// Added / Modified, the `prev` requirement for Removed).
    pub requirement: Requirement,
    /// The spec that produced this change (from the event envelope), if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub spec: Option<String>,
}

/// One capability's folded state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityState {
    /// The capability's CURRENT full state — the latest `capability.declared`
    /// snapshot (doc-faithful, newest wins). Updates do not mutate this.
    pub capability: Capability,
    /// The computed per-requirement change-log, in event order — one entry per
    /// `capability.update`. The reviewable record beside the snapshot.
    pub history: Vec<CapabilityChange>,
    /// `true` once any `capability.drift` event named this capability — it
    /// covers an entity that no longer exists.
    pub stale: bool,
    /// The orphaned entity ids that triggered drift (deduplicated, sorted).
    pub orphaned: Vec<String>,
}

/// The typed roll-up returned by [`project_capabilities`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRollup {
    /// Latest state per capability id, sorted by id for byte-stable output.
    pub capabilities: Vec<CapabilityState>,
}

/// Fold every `capability.*` event into a [`CapabilityRollup`].
///
/// Per-id semantics, applied in the slice's order:
/// - `capability.declared` (re)sets the capability's full **state** (a later
///   declaration overwrites an earlier one — newest wins) while **preserving**
///   any `history` and drift already recorded for that id. The doc snapshot is
///   authoritative for state.
/// - `capability.update` does NOT mutate state; it appends one computed
///   [`CapabilityChange`] (op + requirement + originating spec) to the
///   capability's `history` — the reviewable change-log.
/// - `capability.drift` flags the capability `stale` and records the orphaned
///   entity id.
///
/// An update or drift for an id never seen via a declaration still
/// materialises a state (seeded with just that `id`), so a stream that lost its
/// declaration event still surfaces the capability rather than dropping it.
///
/// Returns an empty rollup when no `capability.*` event exists. Deterministic:
/// the output vector is sorted by id; each `history` is in event order.
#[must_use]
pub fn project_capabilities(events: &[HarnessEvent]) -> CapabilityRollup {
    // Keyed by capability id; BTreeMap so the final vector is id-sorted without
    // an explicit sort pass (byte-stable output).
    let mut states: BTreeMap<String, CapabilityState> = BTreeMap::new();

    for ev in events {
        match ev.event.as_str() {
            EVENT_CAPABILITY_DECLARED => {
                let payload: CapabilityDeclared = parse(ev);
                let id = payload.capability.id.clone();
                if id.is_empty() {
                    continue; // a capability with no id cannot be keyed.
                }
                // Newest declaration wins for STATE; keep history + drift accrued.
                let entry = states.entry(id).or_default();
                entry.capability = payload.capability;
            }
            EVENT_CAPABILITY_UPDATE => {
                let payload: CapabilityUpdate = parse(ev);
                if payload.id.is_empty() {
                    continue;
                }
                let entry = states.entry(payload.id.clone()).or_default();
                // Seed the id on a state materialised from an update alone.
                if entry.capability.id.is_empty() {
                    entry.capability.id.clone_from(&payload.id);
                }
                // Updates feed HISTORY, not state — the doc snapshot already
                // reflects the requirement; this is the reviewable trail.
                entry.history.push(CapabilityChange {
                    op: payload.op,
                    requirement: payload.requirement,
                    spec: ev.spec.clone(),
                });
            }
            EVENT_CAPABILITY_DRIFT => {
                let payload: CapabilityDrift = parse(ev);
                if payload.id.is_empty() {
                    continue;
                }
                let entry = states.entry(payload.id.clone()).or_default();
                if entry.capability.id.is_empty() {
                    entry.capability.id.clone_from(&payload.id);
                }
                entry.stale = true;
                if !payload.entity.is_empty() && !entry.orphaned.contains(&payload.entity) {
                    entry.orphaned.push(payload.entity);
                }
            }
            _ => {}
        }
    }

    // Sort each orphaned set for byte-stable output (insertion order is
    // event order; an explicit sort makes the result order-independent).
    for state in states.values_mut() {
        state.orphaned.sort();
    }

    CapabilityRollup { capabilities: states.into_values().collect() }
}

/// Deserialise a typed payload from an event, falling back to the type's
/// `Default` on any shape mismatch (fail-open — a malformed payload never
/// panics and never drops the fold).
fn parse<T: Default + for<'de> Deserialize<'de>>(ev: &HarnessEvent) -> T {
    serde_json::from_value(ev.payload.clone()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::capability::Scenario;
    use crate::domain::model::event::{Actor, ActorKind, SCHEMA_VERSION};
    use serde_json::json;

    fn ev(event: &str, payload: serde_json::Value) -> HarnessEvent {
        HarnessEvent {
            v: SCHEMA_VERSION,
            ts: "2026-06-17T10:00:00Z".into(),
            session_id: "s1".into(),
            wave: 0,
            actor: Actor { kind: ActorKind::Hook, id: None, actor_type: None },
            event: event.into(),
            payload,
            spec: None,
        }
    }

    #[test]
    fn empty_events_yield_empty_rollup() {
        let r = project_capabilities(&[]);
        assert!(r.capabilities.is_empty());
    }

    #[test]
    fn declared_sets_state_updates_feed_history_not_state() {
        // declared (current snapshot = R1+R2) carries STATE; the update events
        // are the computed change-log and land in HISTORY — they do NOT mutate
        // state (no double-application). Drift is recorded as stale.
        let cap = Capability {
            id: "cap.x".into(),
            title: "Cap X".into(),
            status: "active".into(),
            requirements: vec![
                Requirement {
                    statement: "R1".into(),
                    scenarios: vec![Scenario { name: "s".into(), ..Scenario::default() }],
                },
                Requirement { statement: "R2".into(), scenarios: vec![] },
            ],
            covers: vec!["rt.entity.A".into()],
            ..Capability::default()
        };
        let events = vec![
            ev(EVENT_CAPABILITY_DECLARED, json!({ "capability": cap })),
            ev(
                EVENT_CAPABILITY_UPDATE,
                json!({ "id": "cap.x", "op": "added",
                        "requirement": { "statement": "R2", "scenarios": [] } }),
            ),
            ev(
                EVENT_CAPABILITY_UPDATE,
                json!({ "id": "cap.x", "op": "modified",
                        "requirement": { "statement": "R1", "scenarios": [] } }),
            ),
            ev(EVENT_CAPABILITY_DRIFT, json!({ "id": "cap.x", "entity": "rt.entity.A" })),
        ];

        let r = project_capabilities(&events);
        assert_eq!(r.capabilities.len(), 1);
        let st = &r.capabilities[0];
        assert_eq!(st.capability.id, "cap.x");
        assert_eq!(st.capability.title, "Cap X");
        // STATE = the declared snapshot, verbatim (NOT double-mutated by the
        // updates): R1 still carries its scenario, R2 present once.
        let statements: Vec<&str> =
            st.capability.requirements.iter().map(|r| r.statement.as_str()).collect();
        assert_eq!(statements, vec!["R1", "R2"]);
        assert_eq!(
            st.capability.requirements[0].scenarios.len(),
            1,
            "R1 scenario preserved — update did NOT mutate state"
        );
        // HISTORY = the change-log, in event order.
        assert_eq!(st.history.len(), 2);
        assert_eq!(st.history[0].op, UpdateOp::Added);
        assert_eq!(st.history[0].requirement.statement, "R2");
        assert_eq!(st.history[1].op, UpdateOp::Modified);
        assert_eq!(st.history[1].requirement.statement, "R1");
        // Drift recorded.
        assert!(st.stale);
        assert_eq!(st.orphaned, vec!["rt.entity.A".to_string()]);
    }

    #[test]
    fn update_history_carries_spec_from_envelope() {
        // The originating spec rides on the event envelope and is folded into
        // the change-log entry (so the change-log is attributable).
        let mut update = ev(
            EVENT_CAPABILITY_UPDATE,
            json!({ "id": "cap.s", "op": "removed",
                    "requirement": { "statement": "gone", "scenarios": [] } }),
        );
        update.spec = Some("retire-feature".into());
        let r = project_capabilities(&[update]);
        let st = &r.capabilities[0];
        // Materialised from the update alone (id seeded), with one history entry.
        assert_eq!(st.capability.id, "cap.s");
        assert_eq!(st.history.len(), 1);
        assert_eq!(st.history[0].op, UpdateOp::Removed);
        assert_eq!(st.history[0].spec.as_deref(), Some("retire-feature"));
        // State requirements untouched — the update did not append a phantom req.
        assert!(st.capability.requirements.is_empty());
    }

    #[test]
    fn latest_declaration_wins_but_preserves_drift() {
        let v1 = Capability { id: "cap.v".into(), title: "v1".into(), ..Capability::default() };
        let v2 = Capability { id: "cap.v".into(), title: "v2".into(), ..Capability::default() };
        let events = vec![
            ev(EVENT_CAPABILITY_DECLARED, json!({ "capability": v1 })),
            ev(EVENT_CAPABILITY_DRIFT, json!({ "id": "cap.v", "entity": "rt.entity.Old" })),
            ev(EVENT_CAPABILITY_DECLARED, json!({ "capability": v2 })),
        ];
        let r = project_capabilities(&events);
        assert_eq!(r.capabilities[0].capability.title, "v2", "newest declaration wins");
        assert!(r.capabilities[0].stale, "drift survives a re-declaration");
        assert_eq!(r.capabilities[0].orphaned, vec!["rt.entity.Old".to_string()]);
    }

    #[test]
    fn capabilities_are_sorted_by_id_and_orphans_dedup_sorted() {
        let events = vec![
            ev(EVENT_CAPABILITY_DRIFT, json!({ "id": "cap.b", "entity": "rt.entity.Z" })),
            ev(EVENT_CAPABILITY_DRIFT, json!({ "id": "cap.b", "entity": "rt.entity.A" })),
            ev(EVENT_CAPABILITY_DRIFT, json!({ "id": "cap.b", "entity": "rt.entity.A" })),
            ev(EVENT_CAPABILITY_DRIFT, json!({ "id": "cap.a", "entity": "rt.entity.M" })),
        ];
        let r = project_capabilities(&events);
        // Sorted by id.
        let ids: Vec<&str> = r.capabilities.iter().map(|c| c.capability.id.as_str()).collect();
        assert_eq!(ids, vec!["cap.a", "cap.b"]);
        // Orphans deduped + sorted.
        let b = r.capabilities.iter().find(|c| c.capability.id == "cap.b").unwrap();
        assert_eq!(b.orphaned, vec!["rt.entity.A".to_string(), "rt.entity.Z".to_string()]);
    }

    #[test]
    fn malformed_payload_is_skipped_not_panicked() {
        // Wrong-shaped payloads fail open: declared with no id is ignored,
        // a non-object payload defaults and is skipped.
        let events = vec![
            ev(EVENT_CAPABILITY_DECLARED, json!({ "capability": { "title": "no id" } })),
            ev(EVENT_CAPABILITY_UPDATE, json!("not an object")),
            ev("unrelated.event", json!({ "id": "cap.x" })),
        ];
        let r = project_capabilities(&events);
        assert!(r.capabilities.is_empty());
    }
}
