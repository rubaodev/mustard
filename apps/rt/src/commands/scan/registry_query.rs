//! `mustard-rt run registry-query` — emit a *slice* of `entity-registry.json`.
//!
//! The entity registry is large (hundreds of entities, ~half a megabyte of
//! JSON). Several SKILL/template instructions used to tell the LLM to **read the
//! raw file** and walk it by hand (`Read entity-registry.json → entity found?`,
//! `fall back to reading .claude/entity-registry.json and iterating
//! _patterns[...]`). That loads the whole document into the model context.
//!
//! This subcommand replaces those raw reads: it parses the registry once in Rust
//! (via the canonical [`EntityRegistry`] v4 reader) and prints only the relevant
//! slice — typically a few KB — as compact, byte-stable JSON. No LLM.
//!
//! ## Modes (mutually complementary)
//!
//! - `--entity <name>`: exact, case-insensitive lookup. Emits the single entity
//!   object `e[name]` (`{file,properties,decorators,refs,base_class,
//!   table_name,enums,...}`). With `--with-refs`, emits a name→object map that
//!   also includes the first-degree entities named in `refs`.
//! - `--for-spec <path>`: extracts PascalCase tokens from the spec's *prose*
//!   (reusing [`pascal_tokens`] + [`spec_prose`], the same filters
//!   `scope-decompose` uses), intersects them with the registry's entity names,
//!   and emits a name→object map of just those entities.
//! - `--subproject <x>`: emits the `_patterns.{stack}.discovered[]` clusters
//!   tagged for that subproject (the raw cluster objects) — the slice that backs
//!   the scan agent's `{{clustersBlock}}` fallback.
//!
//! ## Output
//!
//! Compact single-line JSON. Maps are emitted via [`serde_json::Map`] in sorted
//! key order (`BTreeMap` collection) and cluster arrays are sorted by label, so
//! the bytes are deterministic across runs. The entity modes default to `{}`
//! (empty object) when there is no hit; the subproject mode defaults to `[]`.
//!
//! ## Fail-open
//!
//! A missing / unreadable / unparseable registry degrades to the empty answer
//! (`{}` / `[]`) and exit 0 — [`EntityRegistry::load`] already absorbs the IO
//! and parse errors. There is never an LLM round-trip.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde_json::{json, Map, Value};

use mustard_core::domain::entity_registry::EntityRegistry;

use crate::commands::spec::prd_build::pascal_tokens;
use crate::commands::spec::scope_decompose::spec_prose;

/// Which slice the caller asked for. Exactly one is set by the dispatcher.
pub struct RegistryQueryOpts {
    /// `--entity <name>`: exact (case-insensitive) entity lookup.
    pub entity: Option<String>,
    /// `--with-refs`: with `--entity`, also pull the first-degree `refs` entities.
    pub with_refs: bool,
    /// `--for-spec <path>`: entities whose names appear in the spec's prose.
    pub for_spec: Option<PathBuf>,
    /// `--subproject <x>`: the clusters tagged for that subproject.
    pub subproject: Option<String>,
}

/// Resolve the project root from the current working directory, falling back to
/// `.` so the registry load is always attempted (fail-open the rest of the way).
fn project_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    mustard_core::io::workspace::workspace_root(&cwd).unwrap_or(cwd)
}

/// Look up a single entity object by case-insensitive name. Returns the entity
/// `Value` together with the registry's canonical-case key, so callers that
/// build a name→object map use the real name (not the user's casing).
fn lookup_entity<'a>(
    entities: &'a Map<String, Value>,
    name: &str,
) -> Option<(&'a str, &'a Value)> {
    let want = name.to_ascii_lowercase();
    entities
        .iter()
        .filter(|(k, _)| !k.starts_with('_'))
        .find(|(k, _)| k.to_ascii_lowercase() == want)
        .map(|(k, v)| (k.as_str(), v))
}

/// Collect the first-degree entity names referenced in an entity's `refs`.
///
/// `refs` entries are either bare strings or objects carrying a `path` /
/// `entity` / `target` field; we keep any token that resolves to a known entity
/// name (case-insensitive). Only names that exist in the registry are kept —
/// a `refs` path like `src/user.rs` that is not itself an entity is ignored.
fn ref_entity_names(entity: &Value) -> Vec<String> {
    let Some(refs) = entity.get("refs").and_then(Value::as_array) else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for r in refs {
        let candidate = r
            .as_str()
            .or_else(|| r.get("entity").and_then(Value::as_str))
            .or_else(|| r.get("target").and_then(Value::as_str))
            .or_else(|| r.get("name").and_then(Value::as_str));
        if let Some(name) = candidate {
            out.push(name.to_string());
        }
    }
    out
}

/// Build the `--entity` slice. Without `--with-refs`, returns the bare entity
/// object `e[name]`. With `--with-refs`, returns a sorted name→object map of the
/// entity plus its first-degree `refs` entities that exist in the registry.
/// A miss returns `{}`.
fn entity_slice(registry: &EntityRegistry, name: &str, with_refs: bool) -> Value {
    let Some(entities) = registry.entities() else {
        return json!({});
    };
    let Some((canonical, value)) = lookup_entity(entities, name) else {
        return json!({});
    };

    if !with_refs {
        return value.clone();
    }

    // Sorted map: the entity itself + every first-degree ref that is a known
    // entity. Sorted keys ⇒ byte-stable output.
    let mut slice: BTreeMap<String, Value> = BTreeMap::new();
    slice.insert(canonical.to_string(), value.clone());
    for ref_name in ref_entity_names(value) {
        if let Some((ref_canonical, ref_value)) = lookup_entity(entities, &ref_name) {
            slice
                .entry(ref_canonical.to_string())
                .or_insert_with(|| ref_value.clone());
        }
    }
    Value::Object(slice.into_iter().collect())
}

/// Build the `--for-spec` slice: entities whose names appear (case-insensitive)
/// in the spec's *prose*. Reuses [`pascal_tokens`] over [`spec_prose`] — the
/// exact filters `scope-decompose` uses — so headings / file paths in bullets /
/// fenced code never count. Returns a sorted name→object map (`{}` when none).
fn for_spec_slice(registry: &EntityRegistry, spec_text: &str) -> Value {
    let Some(entities) = registry.entities() else {
        return json!({});
    };

    // PascalCase tokens from the prose, lower-cased for matching.
    let referenced: BTreeSet<String> = pascal_tokens(&spec_prose(spec_text))
        .into_iter()
        .map(|t| t.to_ascii_lowercase())
        .collect();

    let mut slice: BTreeMap<String, Value> = BTreeMap::new();
    for (key, value) in entities {
        if key.starts_with('_') {
            continue;
        }
        if referenced.contains(&key.to_ascii_lowercase()) {
            slice.insert(key.clone(), value.clone());
        }
    }
    Value::Object(slice.into_iter().collect())
}

/// Build the `--subproject` slice: the `_patterns.{stack}.discovered[]` clusters
/// tagged for `subproject`. Mirrors [`EntityRegistry::cluster_labels`] scoping —
/// a cluster matches when it has no `subprojectName` (un-scoped) or its
/// `subprojectName` equals / is a path-suffix of `subproject`. Returns the raw
/// cluster objects sorted by `label` (byte-stable); `[]` when none.
fn subproject_slice(registry: &EntityRegistry, subproject: &str) -> Value {
    let Some(patterns) = registry.patterns() else {
        return json!([]);
    };
    let mut clusters: Vec<Value> = Vec::new();
    for body in patterns.values() {
        let Some(arr) = body.get("discovered").and_then(Value::as_array) else {
            continue;
        };
        for cluster in arr {
            let scoped_out = matches!(
                cluster.get("subprojectName").and_then(Value::as_str),
                Some(name) if !subproject.ends_with(name) && name != subproject
            );
            if !scoped_out {
                clusters.push(cluster.clone());
            }
        }
    }
    clusters.sort_by(|a, b| {
        let la = a.get("label").and_then(Value::as_str).unwrap_or("");
        let lb = b.get("label").and_then(Value::as_str).unwrap_or("");
        la.cmp(lb)
    });
    Value::Array(clusters)
}

/// Compute the slice for `opts` against the registry under `root`. Pure: takes
/// the spec text already read, so it is fully unit-testable. Precedence when
/// multiple flags are (mis)passed: `--entity` → `--for-spec` → `--subproject`;
/// none → `{}`.
fn compute_slice(registry: &EntityRegistry, opts: &SliceRequest) -> Value {
    if let Some(name) = &opts.entity {
        entity_slice(registry, name, opts.with_refs)
    } else if let Some(text) = &opts.for_spec_text {
        for_spec_slice(registry, text)
    } else if let Some(sub) = &opts.subproject {
        subproject_slice(registry, sub)
    } else {
        json!({})
    }
}

/// The resolved request after the spec file (if any) has been read — keeps
/// [`compute_slice`] IO-free for tests.
struct SliceRequest {
    entity: Option<String>,
    with_refs: bool,
    for_spec_text: Option<String>,
    subproject: Option<String>,
}

/// Dispatch `mustard-rt run registry-query`. Reads the registry under the
/// resolved project root, computes the slice, and prints compact JSON. Fail-open
/// in every branch (exit 0).
pub fn run(opts: RegistryQueryOpts) {
    let root = project_root();
    let registry = EntityRegistry::load(&root);

    // Read the spec body once (fail-open: an unreadable spec yields no prose, so
    // `for_spec_slice` returns `{}`).
    let for_spec_text = opts.for_spec.as_ref().map(|p| {
        let path = if p.is_absolute() {
            p.clone()
        } else {
            root.join(p)
        };
        mustard_core::io::fs::read_to_string(&path).unwrap_or_default()
    });

    let request = SliceRequest {
        entity: opts.entity,
        with_refs: opts.with_refs,
        for_spec_text,
        subproject: opts.subproject,
    };

    let slice = compute_slice(&registry, &request);
    // Compact (single-line) JSON — the slice is the deliverable, not pretty
    // framing. `to_string` is byte-stable here because every map we build is a
    // sorted `BTreeMap` and arrays are sorted before serialisation.
    println!(
        "{}",
        serde_json::to_string(&slice).unwrap_or_else(|_| "{}".to_string())
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry() -> EntityRegistry {
        EntityRegistry::from_value(json!({
            "_meta": { "version": "4.0" },
            "_patterns": {
                "drizzle": {
                    "discovered": [
                        { "label": "User-CRUD", "suffix": "Service", "fileCount": 7, "subprojectName": "api" },
                        { "label": "Auth", "suffix": "Guard", "fileCount": 3, "subprojectName": "web" },
                        { "label": "Shared", "suffix": "Util", "fileCount": 4 }
                    ]
                }
            },
            "e": {
                "User": {
                    "file": "src/user.rs",
                    "properties": ["id", "name"],
                    "refs": ["Order", "src/user.rs"]
                },
                "Order": { "file": "src/order.rs", "properties": ["id"] },
                "Invoice": { "file": "src/invoice.rs" },
                "_placeholder": {}
            }
        }))
    }

    // --- --entity ----------------------------------------------------------

    #[test]
    fn entity_exact_lookup_returns_only_that_entity() {
        let slice = entity_slice(&registry(), "User", false);
        // The bare entity object — not a name→object map.
        assert_eq!(slice["file"], json!("src/user.rs"));
        assert_eq!(slice["properties"], json!(["id", "name"]));
        // Other entities are absent.
        assert!(slice.get("Order").is_none());
    }

    #[test]
    fn entity_lookup_is_case_insensitive() {
        let slice = entity_slice(&registry(), "uSeR", false);
        assert_eq!(slice["file"], json!("src/user.rs"));
    }

    #[test]
    fn entity_missing_returns_empty_object() {
        let slice = entity_slice(&registry(), "Nope", false);
        assert_eq!(slice, json!({}));
    }

    #[test]
    fn entity_with_refs_pulls_first_degree_entities() {
        let slice = entity_slice(&registry(), "User", true);
        // Name→object map keyed by entity name: User + its ref Order.
        assert!(slice.get("User").is_some());
        assert!(slice.get("Order").is_some());
        assert_eq!(slice["Order"]["file"], json!("src/order.rs"));
        // The non-entity `refs` path (`src/user.rs`) is NOT promoted to a key.
        assert!(slice.get("src/user.rs").is_none());
        // Unrelated entities stay out.
        assert!(slice.get("Invoice").is_none());
    }

    #[test]
    fn entity_placeholder_sentinel_is_never_matched() {
        let slice = entity_slice(&registry(), "_placeholder", false);
        assert_eq!(slice, json!({}));
    }

    // --- --for-spec --------------------------------------------------------

    #[test]
    fn for_spec_returns_only_entities_named_in_prose() {
        // Prose mentions User and Invoice; Order is not mentioned.
        let spec = "# Spec\nLink the Invoice to the User entity.\n\n## Files\n- src/Order.rs\n";
        let slice = for_spec_slice(&registry(), spec);
        assert!(slice.get("User").is_some());
        assert!(slice.get("Invoice").is_some());
        // `Order` appears only inside a `## Files` bullet (a path) — prose filter
        // drops it, so it is NOT in the slice.
        assert!(slice.get("Order").is_none());
        // And it is a slice, not the whole registry: exactly two keys.
        assert_eq!(slice.as_object().unwrap().len(), 2);
    }

    #[test]
    fn for_spec_no_matches_returns_empty_object() {
        let spec = "# Spec\nJust some lowercase prose with no entity names.\n";
        let slice = for_spec_slice(&registry(), spec);
        assert_eq!(slice, json!({}));
    }

    // --- --subproject ------------------------------------------------------

    #[test]
    fn subproject_returns_only_that_subproject_clusters() {
        // `apps/api` → its own cluster (path-suffix match) + the un-scoped one.
        let slice = subproject_slice(&registry(), "apps/api");
        let arr = slice.as_array().expect("array");
        let labels: Vec<&str> = arr
            .iter()
            .filter_map(|c| c.get("label").and_then(Value::as_str))
            .collect();
        // Sorted by label: Shared (un-scoped) + User-CRUD (api). No `Auth` (web).
        assert_eq!(labels, vec!["Shared", "User-CRUD"]);
    }

    #[test]
    fn subproject_missing_patterns_returns_empty_array() {
        let empty = EntityRegistry::from_value(Value::Null);
        assert_eq!(subproject_slice(&empty, "apps/api"), json!([]));
    }

    // --- fail-open + determinism ------------------------------------------

    #[test]
    fn empty_registry_is_fail_open() {
        let empty = EntityRegistry::from_value(Value::Null);
        assert_eq!(entity_slice(&empty, "User", false), json!({}));
        assert_eq!(entity_slice(&empty, "User", true), json!({}));
        assert_eq!(for_spec_slice(&empty, "User stuff"), json!({}));
        assert_eq!(subproject_slice(&empty, "api"), json!([]));
    }

    #[test]
    fn output_is_byte_stable_across_runs() {
        let r = registry();
        let a = serde_json::to_string(&for_spec_slice(&r, "Use User and Invoice here.")).unwrap();
        let b = serde_json::to_string(&for_spec_slice(&r, "Use Invoice and User here.")).unwrap();
        // Sorted keys ⇒ token order in the prose does not change the bytes.
        assert_eq!(a, b);
        let s1 = serde_json::to_string(&subproject_slice(&r, "apps/api")).unwrap();
        let s2 = serde_json::to_string(&subproject_slice(&r, "apps/api")).unwrap();
        assert_eq!(s1, s2);
    }

    #[test]
    fn compute_slice_precedence_entity_over_others() {
        let req = SliceRequest {
            entity: Some("User".to_string()),
            with_refs: false,
            for_spec_text: Some("Invoice".to_string()),
            subproject: Some("api".to_string()),
        };
        // Entity wins — returns the bare User object, not a spec/subproject slice.
        let slice = compute_slice(&registry(), &req);
        assert_eq!(slice["file"], json!("src/user.rs"));
    }
}
