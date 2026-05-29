//! Deterministic concept-node generator (Wave 2-b — graph/wirelinks).
//!
//! Materialises the `.claude/graph/*.md` concept-graph **from the
//! entity-registry**, with **no LLM round-trip**. The cold-path interpreter
//! ([`super::interpret::interpreted_to_nodes`]) is no longer the source of the
//! `[[id]]` graph — this pass is. Every entity (and every enum) the registry
//! records becomes one concept-node addressed by a stable, namespaced id; the
//! entity's `refs` become `[[id]]` edges to the sibling entity nodes that the
//! registry also knows about (orphan edges are never emitted).
//!
//! ## Node schema
//!
//! - **id**: `{NS}.entity.{slug}` for entities, `{NS}.enum.{slug}` for enums,
//!   where `NS` is the fixed root namespace [`GRAPH_NS`] (the registry `e` map
//!   is global — it carries no per-entity subproject — so a single stable
//!   prefix keeps the ids byte-stable, and the Wave-4 resolver matches any
//!   `*.entity.{slug}` suffix regardless of prefix).
//! - **frontmatter**: `id`, `kind` (`entity` / `enum`), `name`, `file` (when
//!   the registry recorded one), and the generated marker [`GEN_FM_KEY`].
//! - **body**: the entity description (when present) followed by a `## Edges`
//!   list of `[[id]]` wikilinks; enums list their values instead.
//!
//! ## Idempotence + manual-node preservation
//!
//! Generated nodes carry **two** markers — a frontmatter `generated: mustard`
//! field ([`GEN_FM_KEY`]) and an HTML comment [`GEN_BODY_MARKER`] in the body.
//! Regeneration:
//!
//! 1. **rewrites** every node the registry currently implies (byte-identical
//!    on a no-change run — the render is a pure function of the registry),
//! 2. **deletes** generated nodes that the registry no longer implies (stale
//!    generated nodes are reaped so the graph never accretes dead nodes), and
//! 3. **never reads, rewrites, or removes** a node that lacks the generated
//!    marker — hand-written concept-nodes are owned by the human.
//!
//! A second run over an unchanged registry is therefore byte-identical, and a
//! manual node dropped into `.claude/graph/` survives every regeneration
//! intact.
//!
//! ## Fail-open
//!
//! Every filesystem error degrades to a skip — the registry remains the source
//! of truth and a vault that could not be materialised is never fatal to the
//! scan. The MOC (`index.md`) is **not** written here: that side effect stays
//! in the explicit `graph-index` command ([`super::graph::materialise_index`]).

use mustard_core::domain::entity_registry::EntityRegistry;
use mustard_core::io::fs as mfs;
use mustard_core::ClaudePaths;
use serde_json::Value;
use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use super::interpret::slugify;

/// Fixed root namespace for registry-derived concept-node ids. The registry
/// `e` map is global (no per-entity subproject), so one stable prefix keeps
/// the ids byte-stable; the Wave-4 resolver matches any `*.entity.{slug}`
/// suffix, so the prefix value is not load-bearing for resolution.
pub const GRAPH_NS: &str = "registry";

/// Frontmatter key marking a node as machine-generated. A node carrying
/// `generated: mustard` is owned by this pass (rewritten + reaped); a node
/// without it is hand-written and never touched.
pub const GEN_FM_KEY: &str = "generated: mustard";

/// HTML comment marker embedded in a generated node body — a second,
/// body-level witness of provenance so the marker survives even a frontmatter
/// edit. The reaper treats *either* marker as "generated".
pub const GEN_BODY_MARKER: &str = "<!-- mustard:generated -->";

/// The outcome of one generation pass — counts for the caller's report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GenerationReport {
    /// Nodes written (created or rewritten) this pass.
    pub written: usize,
    /// Stale generated nodes removed this pass.
    pub removed: usize,
    /// Manual (non-generated) nodes left untouched.
    pub preserved: usize,
}

/// Generate the concept-graph from `<project_root>/.claude/entity-registry.json`.
///
/// Reads the registry through the canonical [`EntityRegistry`] reader, renders
/// one node per entity + enum, writes them under `.claude/graph/`, and reaps
/// any previously-generated node the registry no longer implies. Manual nodes
/// are preserved. Returns a [`GenerationReport`]; fail-open at every step.
pub fn generate_graph_nodes(project_root: &Path) -> GenerationReport {
    let registry = EntityRegistry::load(project_root);
    let nodes = build_nodes(&registry);
    write_nodes(project_root, &nodes)
}

/// One concept-node ready to render — the pure product of the registry.
#[derive(Debug, Clone, PartialEq, Eq)]
struct GenNode {
    /// `{NS}.{kind}.{slug}` id.
    id: String,
    /// Node kind (`entity` / `enum`).
    kind: String,
    /// Display name (the registry key).
    name: String,
    /// Source file relative path, when the registry recorded one.
    file: Option<String>,
    /// Short description, when the registry enriched one.
    description: Option<String>,
    /// Outbound `[[id]]` edge ids (already namespaced + filtered to known nodes).
    edges: Vec<String>,
    /// Enum value list (empty for entities).
    values: Vec<String>,
}

/// Compose a node id from the fixed namespace, a kind, and a raw name.
#[must_use]
fn node_id(kind: &str, raw_name: &str) -> String {
    format!("{GRAPH_NS}.{kind}.{}", slugify(raw_name))
}

/// Build the full deterministic node set from the registry. Pure — no IO.
///
/// Edges are derived from each entity's `refs`: a ref is rendered as an edge
/// only when it slugifies to another entity id that is itself in the node set,
/// so the generated graph carries **zero orphan edges**.
fn build_nodes(registry: &EntityRegistry) -> Vec<GenNode> {
    // 1. The entity-id set, so refs can be filtered to known targets.
    let mut entity_ids: BTreeSet<String> = BTreeSet::new();
    if let Some(entities) = registry.entities() {
        for name in entities.keys() {
            if name.starts_with('_') {
                continue;
            }
            entity_ids.insert(node_id("entity", name));
        }
    }

    let mut out: Vec<GenNode> = Vec::new();

    // 2. Entity nodes.
    if let Some(entities) = registry.entities() {
        for (name, body) in entities {
            if name.starts_with('_') {
                continue;
            }
            let id = node_id("entity", name);
            let map = body.as_object();
            let file = map
                .and_then(|m| m.get("file"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            let description = map
                .and_then(|m| m.get("description"))
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .map(str::to_string);
            // Edges: refs → sibling entity ids that exist in the set.
            let mut edges: BTreeSet<String> = BTreeSet::new();
            if let Some(refs) = map.and_then(|m| m.get("refs")).and_then(Value::as_array) {
                for r in refs {
                    let Some(ref_name) = r.as_str() else { continue };
                    let target = node_id("entity", ref_name);
                    if target != id && entity_ids.contains(&target) {
                        edges.insert(target);
                    }
                }
            }
            out.push(GenNode {
                id,
                kind: "entity".to_string(),
                name: name.clone(),
                file,
                description,
                edges: edges.into_iter().collect(),
                values: Vec::new(),
            });
        }
    }

    // 3. Enum nodes.
    if let Some(enums) = registry.enums() {
        for (name, body) in enums {
            if name.starts_with('_') {
                continue;
            }
            let id = node_id("enum", name);
            // The registry stores enums either as a bare value array or as a
            // rich `{ values, file, ... }` object — accept both shapes.
            let (values, file) = match body {
                Value::Array(arr) => (value_strings(arr), None),
                Value::Object(map) => {
                    let values = map
                        .get("values")
                        .and_then(Value::as_array)
                        .map(|a| value_strings(a))
                        .unwrap_or_default();
                    let file = map
                        .get("file")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                        .map(str::to_string);
                    (values, file)
                }
                _ => (Vec::new(), None),
            };
            out.push(GenNode {
                id,
                kind: "enum".to_string(),
                name: name.clone(),
                file,
                description: None,
                edges: Vec::new(),
                values,
            });
        }
    }

    // Byte-stable order: by id ascending.
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// Collect the string members of a JSON array, dropping non-strings and the
/// `...(N total)` compression sentinel `compress_values` inserts.
fn value_strings(arr: &[Value]) -> Vec<String> {
    arr.iter()
        .filter_map(Value::as_str)
        .filter(|s| !s.starts_with("...("))
        .map(str::to_string)
        .collect()
}

/// Render a single generated node to its byte-stable markdown form.
///
/// Frontmatter key order is fixed (`id`, `kind`, `name`, `file?`, the generated
/// marker) so a no-change regeneration is byte-identical.
#[must_use]
fn render_node(node: &GenNode) -> String {
    let mut out = String::new();
    out.push_str("---\n");
    let _ = writeln!(out, "id: {}", node.id);
    let _ = writeln!(out, "kind: {}", node.kind);
    let _ = writeln!(out, "name: {}", node.name);
    if let Some(file) = &node.file {
        let _ = writeln!(out, "file: {file}");
    }
    let _ = writeln!(out, "{GEN_FM_KEY}");
    out.push_str("---\n\n");
    let _ = writeln!(out, "{GEN_BODY_MARKER}");
    let _ = writeln!(out, "# {}\n", node.name);
    if let Some(desc) = &node.description {
        let _ = writeln!(out, "{desc}\n");
    }
    if node.kind == "enum" && !node.values.is_empty() {
        out.push_str("## Values\n\n");
        for v in &node.values {
            let _ = writeln!(out, "- {v}");
        }
        out.push('\n');
    }
    if node.edges.is_empty() {
        out.push_str("_No outbound edges._\n");
    } else {
        out.push_str("## Edges\n\n");
        for edge in &node.edges {
            let _ = writeln!(out, "- [[{edge}]]");
        }
        out.push('\n');
    }
    out
}

/// `true` when `content` carries either generation marker. A node lacking both
/// is hand-written and must never be rewritten or removed.
#[must_use]
fn is_generated(content: &str) -> bool {
    // The frontmatter marker is the canonical witness; the body comment is a
    // belt-and-braces fallback so a frontmatter reorder still reads as generated.
    let fm_marked = content
        .lines()
        .take_while(|l| *l != "---" || content.starts_with("---"))
        .any(|l| l.trim() == GEN_FM_KEY);
    fm_marked || content.contains(GEN_BODY_MARKER)
}

/// Write every node, reap stale generated nodes, preserve manual nodes.
fn write_nodes(project_root: &Path, nodes: &[GenNode]) -> GenerationReport {
    let mut report = GenerationReport::default();
    let Ok(paths) = ClaudePaths::for_project(project_root) else {
        return report;
    };
    let graph_dir = paths.graph_dir();
    if mfs::create_dir_all(&graph_dir).is_err() {
        return report;
    }

    // Filenames we are about to (re)write — used to spare them from the reaper.
    let mut written_files: BTreeSet<String> = BTreeSet::new();

    // 1. Write the current node set.
    for node in nodes {
        let filename = format!("{}.md", node.id);
        let path = graph_dir.join(&filename);
        let body = render_node(node);
        if mfs::write_atomic(&path, body.as_bytes()).is_ok() {
            report.written += 1;
            written_files.insert(filename);
        }
    }

    // 2. Reap stale generated nodes; count + preserve manual ones.
    if let Ok(entries) = mfs::read_dir(&graph_dir) {
        for entry in entries {
            if entry.is_dir {
                continue;
            }
            let name = entry.file_name.clone();
            if !name.ends_with(".md") || name == "index.md" {
                continue;
            }
            // A file we just wrote is current — skip.
            if written_files.contains(&name) {
                continue;
            }
            let Ok(content) = mfs::read_to_string(&entry.path) else {
                continue;
            };
            if is_generated(&content) {
                // Stale generated node — no longer implied by the registry.
                if mfs::remove_file(&entry.path).is_ok() {
                    report.removed += 1;
                }
            } else {
                // Hand-written node — leave it exactly as-is.
                report.preserved += 1;
            }
        }
    }

    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::collections::BTreeMap;
    use tempfile::tempdir;

    /// Plant a workspace anchor + an `entity-registry.json` with the given body.
    fn seed_registry(root: &Path, doc: Value) {
        std::fs::create_dir_all(root.join(".claude")).unwrap();
        std::fs::write(root.join("mustard.json"), b"{}").unwrap();
        let paths = ClaudePaths::for_project(root).unwrap();
        let pretty = format!("{}\n", serde_json::to_string_pretty(&doc).unwrap());
        std::fs::write(paths.entity_registry_json_path(), pretty).unwrap();
    }

    fn fixture_doc() -> Value {
        json!({
            "_meta": { "version": "4.0", "generated": "2026-05-29" },
            "_patterns": {},
            "_enums": {
                "Role": { "values": ["Admin", "Guest"], "file": "src/role.rs" },
                "Status": ["Open", "Closed"]
            },
            "e": {
                "User": {
                    "file": "src/user.rs",
                    "description": "A registered user.",
                    "refs": ["Order", "Unknown"]
                },
                "Order": { "file": "src/order.rs", "refs": ["User"] },
                "_placeholder": {}
            }
        })
    }

    #[test]
    fn generates_entity_and_enum_nodes() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        seed_registry(root, fixture_doc());

        let report = generate_graph_nodes(root);
        // 2 entities + 2 enums (placeholder excluded).
        assert_eq!(report.written, 4, "entities + enums written");
        assert_eq!(report.preserved, 0);
        assert_eq!(report.removed, 0);

        let graph = ClaudePaths::for_project(root).unwrap().graph_dir();
        let user = std::fs::read_to_string(graph.join("registry.entity.user.md")).unwrap();
        assert!(user.contains("id: registry.entity.user"));
        assert!(user.contains("kind: entity"));
        assert!(user.contains("file: src/user.rs"));
        assert!(user.contains("A registered user."));
        assert!(user.contains(GEN_FM_KEY));
        assert!(user.contains(GEN_BODY_MARKER));
        // Edge to Order (known entity) is emitted; "Unknown" (no node) is dropped.
        assert!(user.contains("[[registry.entity.order]]"));
        assert!(!user.contains("unknown"));

        let role = std::fs::read_to_string(graph.join("registry.enum.role.md")).unwrap();
        assert!(role.contains("kind: enum"));
        assert!(role.contains("- Admin"));
        assert!(role.contains("- Guest"));

        // Bare-array enum shape is also handled.
        let status = std::fs::read_to_string(graph.join("registry.enum.status.md")).unwrap();
        assert!(status.contains("- Open"));
        assert!(status.contains("- Closed"));
    }

    #[test]
    fn second_generation_is_byte_identical() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        seed_registry(root, fixture_doc());

        generate_graph_nodes(root);
        let graph = ClaudePaths::for_project(root).unwrap().graph_dir();
        let snapshot: BTreeMap<String, String> = std::fs::read_dir(&graph)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read_to_string(e.path()).unwrap(),
                )
            })
            .collect();

        // Regenerate — must be byte-for-byte identical.
        generate_graph_nodes(root);
        let snapshot2: BTreeMap<String, String> = std::fs::read_dir(&graph)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| {
                (
                    e.file_name().to_string_lossy().into_owned(),
                    std::fs::read_to_string(e.path()).unwrap(),
                )
            })
            .collect();
        assert_eq!(snapshot, snapshot2, "regeneration must be byte-stable");
    }

    #[test]
    fn manual_nodes_are_preserved_and_stale_generated_reaped() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        seed_registry(root, fixture_doc());

        let graph = ClaudePaths::for_project(root).unwrap().graph_dir();
        std::fs::create_dir_all(&graph).unwrap();
        // A hand-written node (NO generated marker).
        let manual = "---\nid: hand.conv.style\nkind: conv\n---\n# Style guide\nManual node.\n";
        std::fs::write(graph.join("hand.conv.style.md"), manual).unwrap();
        // A stale generated node for an entity NOT in the registry.
        let stale = format!(
            "---\nid: registry.entity.ghost\nkind: entity\nname: Ghost\n{GEN_FM_KEY}\n---\n\n{GEN_BODY_MARKER}\n# Ghost\n_No outbound edges._\n"
        );
        std::fs::write(graph.join("registry.entity.ghost.md"), &stale).unwrap();

        let report = generate_graph_nodes(root);
        // 4 registry nodes written, the stale generated ghost reaped, manual kept.
        assert_eq!(report.written, 4);
        assert_eq!(report.removed, 1, "stale generated node reaped");
        assert_eq!(report.preserved, 1, "manual node counted as preserved");

        // Manual node survives intact, byte-for-byte.
        let after = std::fs::read_to_string(graph.join("hand.conv.style.md")).unwrap();
        assert_eq!(after, manual, "manual node must be untouched");
        // Stale generated node is gone.
        assert!(!graph.join("registry.entity.ghost.md").exists());
    }

    #[test]
    fn empty_registry_is_inert() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        seed_registry(root, json!({ "_meta": { "version": "4.0" }, "e": {}, "_enums": {} }));
        let report = generate_graph_nodes(root);
        assert_eq!(report.written, 0);
        assert_eq!(report.removed, 0);
    }

    /// End-to-end: generate the graph from a registry, then resolve a closure
    /// seeded on an entity. The closure must be NON-empty and reach the
    /// entity's `refs` neighbour over the generated `[[id]]` edge — proving the
    /// deterministic generator feeds the Wave-4 resolver with NO LLM involved.
    #[test]
    fn generated_graph_drives_resolve_closure() {
        use crate::commands::scan::resolve::{resolve_closure, ResolveScope};
        use std::collections::BTreeSet;

        let dir = tempdir().unwrap();
        let root = dir.path();
        seed_registry(root, fixture_doc());

        let report = generate_graph_nodes(root);
        assert!(report.written >= 2, "entities generated");

        // Seed on the `User` entity (the resolver matches any `*.entity.user`).
        let scope = ResolveScope {
            entities: vec!["User".to_string()],
            ..ResolveScope::default()
        };
        let out = resolve_closure(root, &scope);
        assert!(
            !out.closure.is_empty(),
            "closure over generated nodes must be non-empty"
        );
        let ids: BTreeSet<&str> = out.closure.iter().map(|n| n.id.as_str()).collect();
        assert!(ids.contains("registry.entity.user"), "seed in closure");
        assert!(
            ids.contains("registry.entity.order"),
            "ref edge User->Order reached over the generated graph"
        );
    }
}
