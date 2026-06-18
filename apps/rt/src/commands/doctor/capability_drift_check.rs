//! `mustard-rt run doctor --check capability-drift` — capability/grain drift
//! advisory.
//!
//! Roadmap #6 companion to [`super::superseded_check`]. A capability's
//! `## Covers` carries `entity.{name}` wikilinks pointing at the code that
//! realises the behaviour. When that code is renamed or deleted, the grain
//! model stops mining `{name}` as a declaration — the capability now **covers
//! code that no longer exists** (it has *drifted*). This check surfaces that
//! gap by comparing each capability's covered entity names against the grain
//! model's known declaration-name set (read in-process via
//! [`mustard_core::read_entity_names`] — there is NO `registry-query` verb).
//!
//! It is **ADVISORY ONLY**: every drifted cover is emitted as an
//! [`EVENT_CAPABILITY_DRIFT`] event AND reported as a WARN-level line, but the
//! check never blocks and never errors the doctor run.
//!
//! Fail-open: when the grain model is absent (no scan yet) we cannot judge
//! drift — without a model every cover would look "missing", which is a false
//! positive — so the check is a **silent no-op** ([`run`] returns `None`). A
//! single unreadable / malformed capability doc is skipped and recorded in
//! `scannedErrors`; the scan continues and never panics.
//!
//! Agnostic: the entity-name derivation strips only the `entity.` id prefix and
//! keeps the rest verbatim — no `rt.` / language / role assumption. The known
//! set is whatever grain mined as a declaration.

use mustard_core::domain::capability::{CapabilityDrift, EVENT_CAPABILITY_DRIFT};
use mustard_core::domain::model::event::{Actor, ActorKind, HarnessEvent, SCHEMA_VERSION};
use mustard_core::io::fs;
use mustard_core::ClaudePaths;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::Path;

/// The `entity.` id prefix. A capability covers an entity via the wikilink id
/// `entity.{name}`; `{name}` after this prefix is the bare declaration name the
/// grain registry mined. Agnostic — no `rt.` / language assumption.
const ENTITY_NODE_PREFIX: &str = "entity.";

/// One drifted cover — a capability that covers an `entity.{name}` absent from
/// the grain model. Serialized in (capability-id, entity-id) order for
/// byte-stability.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CapabilityDriftItem {
    /// The drifted capability id (`cap.{slug}`).
    pub id: String,
    /// The orphaned cover wikilink id (`entity.{name}`) whose `{name}` is no
    /// longer in the grain declaration-name set.
    pub entity: String,
}

/// The capability-drift advisory report. All vectors are sorted for stability.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CapabilityDriftReport {
    /// `true` when nothing drifted (no covered entity is missing from grain).
    pub ok: bool,
    /// How many capability docs were parsed.
    pub total_capabilities: usize,
    /// The drifted covers (sorted by `(id, entity)`). Advisory — never blocks.
    pub drifted: Vec<CapabilityDriftItem>,
    /// Per-doc read errors (unreadable capability markdown). Sorted; fail-open
    /// evidence.
    pub scanned_errors: Vec<String>,
}

/// Strip the `entity.` id prefix to the bare declaration name the grain model
/// knows. Returns `None` for an id that is not an `entity.*` cover (a `spec.*` /
/// `cap.*` cover is left alone) or an empty name. Agnostic.
fn entity_name_of(id: &str) -> Option<&str> {
    id.strip_prefix(ENTITY_NODE_PREFIX)
        .map(str::trim)
        .filter(|n| !n.is_empty())
}

/// Pure drift detection: for each `(capability_id, covers)` pair, every
/// `entity.{name}` cover whose `{name}` is absent from `known` is a drifted
/// cover. Deterministic — the input order is preserved and the caller sorts.
/// No IO, never panics. (`known` is the grain declaration-name set, compared
/// case-insensitively to mirror the other grain-membership checks.)
#[must_use]
fn detect_drift(
    capabilities: &[(String, Vec<String>)],
    known: &BTreeSet<String>,
) -> Vec<CapabilityDriftItem> {
    // Lower-cased view for case-insensitive membership (grain names are mined
    // verbatim; a capability author may differ in casing).
    let known_lc: BTreeSet<String> = known.iter().map(|n| n.to_ascii_lowercase()).collect();
    let mut out = Vec::new();
    for (id, covers) in capabilities {
        for cover in covers {
            let Some(name) = entity_name_of(cover) else {
                continue; // not an `entity.*` cover — not our concern.
            };
            if !known_lc.contains(&name.to_ascii_lowercase()) {
                out.push(CapabilityDriftItem {
                    id: id.clone(),
                    entity: cover.clone(),
                });
            }
        }
    }
    out
}

/// Read every `.claude/capabilities/*.md` under `root`, parse it, and build the
/// drift report against `known` (the grain declaration-name set). Fail-open: an
/// unreadable doc is skipped + recorded; a missing capabilities dir yields an
/// empty (ok) report. `known` is injected so the detection core is testable
/// without a grain binary.
fn build_report(root: &Path, known: &BTreeSet<String>) -> CapabilityDriftReport {
    let Ok(paths) = ClaudePaths::for_project(root) else {
        return ok_empty();
    };
    let caps_dir = paths.capabilities_dir();
    let Ok(entries) = fs::read_dir(&caps_dir) else {
        // No capabilities dir — nothing to judge.
        return ok_empty();
    };

    // Collect + sort the `.md` names so iteration (and error) order is
    // deterministic. A `.md` entry that is not a readable file (e.g. a
    // directory named `*.md`) is left in: the `read_to_string` below fails on
    // it and records the error — fail-open, never a panic.
    let mut files: Vec<String> = entries
        .into_iter()
        .filter(|e| e.file_name.ends_with(".md"))
        .map(|e| e.file_name)
        .collect();
    files.sort();

    let mut capabilities: Vec<(String, Vec<String>)> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    for file in &files {
        let path = caps_dir.join(file);
        let Ok(md) = fs::read_to_string(&path) else {
            errors.push(format!("{file}: unreadable (drift skipped)"));
            continue;
        };
        let cap = crate::commands::capability::parse(&md);
        capabilities.push((cap.id, cap.covers));
    }
    let total_capabilities = capabilities.len();

    let mut drifted = detect_drift(&capabilities, known);
    drifted.sort_by(|a, b| a.id.cmp(&b.id).then_with(|| a.entity.cmp(&b.entity)));
    errors.sort();

    CapabilityDriftReport {
        ok: drifted.is_empty(),
        total_capabilities,
        drifted,
        scanned_errors: errors,
    }
}

/// An empty, healthy report (no capabilities / no drift).
fn ok_empty() -> CapabilityDriftReport {
    CapabilityDriftReport {
        ok: true,
        total_capabilities: 0,
        drifted: Vec::new(),
        scanned_errors: Vec::new(),
    }
}

/// Run the capability-drift advisory under `root` (the workspace root holding
/// `.claude/`).
///
/// Reads the grain model's known declaration-name set in-process via
/// [`mustard_core::read_entity_names`] (NO `registry-query` verb). When the
/// model is ABSENT, returns `None` — a silent no-op, because without a model
/// every cover would look "missing" (a false positive). Otherwise builds the
/// report, emits one [`EVENT_CAPABILITY_DRIFT`] per drifted cover via
/// [`crate::shared::events::route::emit`], and returns `Some(report)` for the
/// doctor renderer. Advisory only — emits + reports, never blocks.
#[must_use]
pub fn run(root: &Path) -> Option<CapabilityDriftReport> {
    let model = root.join(".claude").join("grain.model.json");
    // No grain model ⇒ cannot judge drift ⇒ silent no-op.
    if !model.is_file() {
        return None;
    }
    let known: BTreeSet<String> = mustard_core::read_entity_names(&model).into_iter().collect();

    let report = build_report(root, &known);

    // Advisory event per drifted cover. Fail-open: emit errors are swallowed
    // (telemetry is never load-bearing).
    let project = root.to_string_lossy();
    for item in &report.drifted {
        emit_drift(&project, &item.id, &item.entity);
    }

    Some(report)
}

/// Emit one `capability.drift` advisory event. Fail-open (return ignored).
fn emit_drift(project_dir: &str, cap_id: &str, entity_id: &str) {
    let payload = serde_json::to_value(CapabilityDrift {
        id: cap_id.to_string(),
        entity: entity_id.to_string(),
    })
    .unwrap_or(serde_json::Value::Null);
    let event = HarnessEvent {
        v: SCHEMA_VERSION,
        ts: mustard_core::time::now_iso8601(),
        session_id: crate::shared::context::session_id(),
        wave: 0,
        actor: Actor {
            kind: ActorKind::Cli,
            id: Some("doctor".to_string()),
            actor_type: None,
        },
        event: EVENT_CAPABILITY_DRIFT.to_string(),
        payload,
        spec: None,
    };
    let _ = crate::shared::events::route::emit(project_dir, &event);
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustard_core::domain::capability::Capability;
    use tempfile::tempdir;

    /// Seed `<root>/.claude/capabilities/{slug}.md` from a `Capability`.
    fn seed_capability(root: &Path, slug: &str, cap: &Capability) {
        let dir = root.join(".claude").join("capabilities");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(format!("{slug}.md")),
            crate::commands::capability::render(cap),
        )
        .unwrap();
    }

    fn known_set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    // --- pure detection ---------------------------------------------------

    /// A capability covering a PRESENT entity drifts not; covering a MISSING
    /// one drifts (one item). Non-`entity.*` covers are ignored.
    #[test]
    fn detect_drift_present_vs_missing_entity() {
        let known = known_set(&["InvoiceService", "Order"]);
        let caps = vec![
            (
                "cap.present".to_string(),
                vec!["entity.InvoiceService".to_string()],
            ),
            (
                "cap.missing".to_string(),
                vec!["entity.GoneService".to_string()],
            ),
            // A `spec.*` cover that slipped in is not our concern.
            (
                "cap.mixed".to_string(),
                vec!["entity.Order".to_string(), "spec.something".to_string()],
            ),
        ];
        let drifted = detect_drift(&caps, &known);
        assert_eq!(
            drifted,
            vec![CapabilityDriftItem {
                id: "cap.missing".to_string(),
                entity: "entity.GoneService".to_string(),
            }],
            "only the missing-entity cover drifts"
        );
    }

    /// Case-insensitive membership: a casing-only difference is NOT drift.
    #[test]
    fn detect_drift_is_case_insensitive() {
        let known = known_set(&["InvoiceService"]);
        let caps = vec![("cap.x".to_string(), vec!["entity.invoiceservice".to_string()])];
        assert!(detect_drift(&caps, &known).is_empty(), "casing diff is not drift");
    }

    // --- end-to-end over docs --------------------------------------------

    /// A capability covering a present entity → no drift; covering a missing
    /// entity → exactly one drift item + a warn-able report.
    #[test]
    fn build_report_present_no_drift_missing_one_drift() {
        let dir = tempdir().unwrap();
        let root = dir.path();

        seed_capability(
            root,
            "present",
            &Capability {
                id: "cap.present".into(),
                covers: vec!["entity.InvoiceService".into()],
                ..Capability::default()
            },
        );
        let known = known_set(&["InvoiceService"]);
        let report = build_report(root, &known);
        assert!(report.ok, "present entity ⇒ no drift: {report:?}");
        assert_eq!(report.total_capabilities, 1);
        assert!(report.drifted.is_empty());

        // Now add a capability covering a missing entity.
        seed_capability(
            root,
            "stale",
            &Capability {
                id: "cap.stale".into(),
                covers: vec!["entity.RemovedThing".into()],
                ..Capability::default()
            },
        );
        let report = build_report(root, &known);
        assert!(!report.ok, "missing entity ⇒ drift");
        assert_eq!(report.total_capabilities, 2);
        assert_eq!(report.drifted.len(), 1);
        assert_eq!(report.drifted[0].id, "cap.stale");
        assert_eq!(report.drifted[0].entity, "entity.RemovedThing");
    }

    /// `run` with a missing grain model is a SILENT no-op (returns `None`) and
    /// emits nothing — without a model we cannot judge drift.
    #[test]
    fn run_absent_grain_is_silent_noop() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // A capability doc exists but there is NO grain.model.json.
        seed_capability(
            root,
            "x",
            &Capability {
                id: "cap.x".into(),
                covers: vec!["entity.Whatever".into()],
                ..Capability::default()
            },
        );
        assert!(run(root).is_none(), "absent grain ⇒ silent no-op");
    }

    /// An unreadable capability doc is skipped + recorded; the scan continues.
    #[test]
    fn unreadable_doc_is_skipped_and_recorded() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let caps = root.join(".claude").join("capabilities");
        std::fs::create_dir_all(&caps).unwrap();
        // A directory named like a doc is unreadable as a file → recorded.
        std::fs::create_dir_all(caps.join("weird.md")).unwrap();
        // A healthy capability alongside it.
        seed_capability(
            root,
            "good",
            &Capability {
                id: "cap.good".into(),
                covers: vec!["entity.Known".into()],
                ..Capability::default()
            },
        );
        let known = known_set(&["Known"]);
        let report = build_report(root, &known);
        // The good cap parsed; the weird "doc" was recorded as an error.
        assert_eq!(report.total_capabilities, 1);
        assert_eq!(report.scanned_errors.len(), 1);
        assert!(report.scanned_errors[0].contains("weird.md"));
        assert!(report.ok, "the one good cap covers a present entity");
    }

    /// No capabilities dir ⇒ an ok/empty report (build_report) — and the byte
    /// shape is stable across runs.
    #[test]
    fn no_capabilities_dir_is_ok_empty_and_stable() {
        let dir = tempdir().unwrap();
        let known = known_set(&["Anything"]);
        let a = build_report(dir.path(), &known);
        let b = build_report(dir.path(), &known);
        assert!(a.ok);
        assert_eq!(a.total_capabilities, 0);
        assert_eq!(
            serde_json::to_string(&a).unwrap(),
            serde_json::to_string(&b).unwrap(),
            "byte-stable"
        );
    }
}
