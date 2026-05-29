//! Concept-node graph: parse, validate, render the MOC (Wave 3 — project-profiler).
//!
//! Walks `.claude/graph/` collecting every concept-node markdown file, parses
//! its frontmatter `id` plus inline `[[id]]` edges, and produces three
//! artifacts: an in-memory adjacency map, an `id → path` lookup table, and an
//! `index.md` Map-Of-Content (MOC) listing every node grouped by kind. The
//! same pass also surfaces validation warnings — `[[id]]` edges pointing at
//! ids the index does not know about (orphan edges) and cycles in the
//! adjacency graph (which the validator records as cut, never panics).
//!
//! The Wave 4 resolver consumes the adjacency + id table to walk the graph
//! and assemble per-agent context; this module is the build/validate face,
//! exposed both as a library (for `sync_entity_registry` / future enrichment passes
//! to call) and as the `mustard-rt run graph-index` subcommand.
//!
//! ## Fail-open
//!
//! A missing `.claude/graph/` tree degrades to an empty index — never an
//! error. A malformed frontmatter (`id` missing) skips that file with a
//! warning rather than aborting the build. The validator records warnings;
//! it never returns `Err`.

use mustard_core::io::fs as mfs;
use mustard_core::ClaudePaths;
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Wave-5 (project-profiler) — edge schema with `kind` + backlinks write-back
// ---------------------------------------------------------------------------

/// The `kind` of a spec→node edge written back into a spec's `## Backlinks`
/// section.
///
/// - [`EdgeKind::Injected`] — the resolver returned this node in the closure
///   handed to the agent. This is a *fact*: the pipeline knows exactly which
///   nodes it injected.
/// - [`EdgeKind::Applied`] — the node is *inferred* to have influenced the
///   work, because at least one file the wave touched lives under the path
///   the node describes. Carries a `confidence` score (0.0–1.0) and must
///   never be presented as a fact (see [`spec`] non-goal).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeKind {
    /// Returned in the closure handed to the agent at dispatch time.
    Injected,
    /// Heuristically matched to the wave's file diff; inferred, never asserted.
    Applied,
}

impl EdgeKind {
    /// The lowercase token used in the `<!-- kind: ... -->` comment marker.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Injected => "injected",
            Self::Applied => "applied",
        }
    }

    /// Parse the lowercase token back into an [`EdgeKind`].
    ///
    /// Carried as the inverse of [`Self::as_str`] for any future caller that
    /// needs to round-trip a backlink line without going through
    /// [`parse_backlinks`] (which already does the full line parse). The
    /// idempotency test exercises the round-trip via `parse_backlinks` —
    /// hence the `dead_code` allow.
    #[must_use]
    #[allow(dead_code)]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "injected" => Some(Self::Injected),
            "applied" => Some(Self::Applied),
            _ => None,
        }
    }
}

/// One write-back edge from a spec to a concept-node.
///
/// Mirrors the on-disk wire format inside the `## Backlinks` block:
/// `- [[id]] <!-- kind: injected -->` or
/// `- [[id]] <!-- kind: applied confidence: 0.50 -->`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SpecBacklinkEdge {
    /// Concept-node id (matches a key in [`GraphIndex::nodes`]).
    pub target: String,
    /// Whether this edge was injected (fact) or applied (inferred).
    pub kind: EdgeKind,
    /// 0.0–1.0 score for [`EdgeKind::Applied`]; `None` for [`EdgeKind::Injected`]
    /// (injection is not a probability — it is a record).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confidence: Option<f32>,
}

/// Marker that opens the spec's auto-managed backlinks section.
pub const BACKLINKS_HEADING: &str = "## Backlinks";
/// HTML comment fencing the auto-managed block; everything between the open
/// and close marker is owned by the write-back step and replaced wholesale.
const BACKLINKS_OPEN: &str = "<!-- mustard:backlinks:start -->";
const BACKLINKS_CLOSE: &str = "<!-- mustard:backlinks:end -->";

/// The output of one graph-index build pass.
///
/// [`build_index`] is **pure**: it only reads `.claude/graph/` and returns this
/// in-memory structure. Writing the `index.md` MOC and injecting skill aliases
/// are *side effects* that live solely in the explicit `graph-index` command
/// ([`materialise_index`]); a resolver cache-miss therefore never rewrites the
/// MOC on disk.
#[derive(Debug, Clone, Default, Serialize)]
pub struct GraphIndex {
    /// `id → relative path under .claude/graph/`. Byte-stable ordering.
    pub nodes: BTreeMap<String, String>,
    /// Adjacency map: `id → outbound edge ids` (in source order).
    pub edges: BTreeMap<String, Vec<String>>,
    /// Validation warnings — orphans + cycles. Each entry is a single line.
    pub warnings: Vec<String>,
    /// Skill files whose frontmatter was extended with an `aliases:` entry.
    /// Populated only by the explicit `graph-index` command (the alias
    /// injection is a write side effect, not part of the pure build).
    pub aliased_skills: Vec<String>,
    /// `id → kind` (e.g. `entity`, `conv`, `skill`). Carried so the explicit
    /// `graph-index` command can render the MOC without re-reading every node.
    #[serde(skip)]
    pub kinds: BTreeMap<String, String>,
}

/// Recursively collect every `.md` file under `dir`, sorted by relative path.
/// Hidden directories (those starting with `.`) are *not* skipped — the vault
/// itself lives under `.claude/graph/`, so dot-prefix is normal here.
fn collect_markdown(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out: Vec<(PathBuf, String)> = Vec::new();
    let mut stack: Vec<PathBuf> = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = mfs::read_dir(&d) else {
            continue;
        };
        for entry in entries {
            if entry.is_dir {
                stack.push(entry.path.clone());
                continue;
            }
            if !entry.file_name.ends_with(".md") {
                continue;
            }
            let rel = entry
                .path
                .strip_prefix(dir)
                .unwrap_or(&entry.path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push((entry.path, rel));
        }
    }
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out
}

/// Extract the frontmatter `id:` value (if present) from a markdown body.
/// Returns `None` when the file has no `---` frontmatter or no `id:` key.
fn parse_frontmatter_id(content: &str) -> Option<String> {
    let stripped = content.strip_prefix("---\n")?;
    let end = stripped.find("\n---")?;
    let block = &stripped[..end];
    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("id:") {
            let value = rest.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Extract the frontmatter `kind:` value, defaulting to `"node"` when absent.
fn parse_frontmatter_kind(content: &str) -> String {
    let Some(stripped) = content.strip_prefix("---\n") else {
        return "node".to_string();
    };
    let Some(end) = stripped.find("\n---") else {
        return "node".to_string();
    };
    let block = &stripped[..end];
    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("kind:") {
            let value = rest.trim();
            if !value.is_empty() {
                return value.to_string();
            }
        }
    }
    "node".to_string()
}

/// Extract every `[[id]]` concept-graph edge from `content`.
///
/// There is no second byte-scanner here: this consumes the single canonical
/// `[[…]]` scanner ([`mustard_core::io::atomic_md::scan_links`]) and applies
/// the concept-id charset filter ([`is_concept_id`]) on top. A token is kept
/// as an edge only when every byte is `[a-zA-Z0-9_.-]` — the namespaced
/// concept-id shape (`<sub>.<kind>.<slug>`). Free-text wikilinks such as
/// `[[my note]]` (which the scanner returns verbatim) are dropped, exactly as
/// the old restricted-charset scanner did.
#[must_use]
pub fn extract_edges(content: &str) -> Vec<String> {
    mustard_core::io::atomic_md::scan_links(content)
        .into_iter()
        .filter(|token| is_concept_id(token))
        .collect()
}

/// `true` when every byte of `token` is in the concept-id charset
/// `[a-zA-Z0-9_.-]` (and the token is non-empty). Mirrors the charset the
/// pre-unification graph scanner accepted between `[[` and `]]`.
pub(crate) fn is_concept_id(token: &str) -> bool {
    !token.is_empty()
        && token
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'_' || c == b'-' || c == b'.')
}

/// Detect cycles in the adjacency map. Returns the ids that participate in a
/// cycle (one entry per back-edge target — duplicates collapsed). Cycles are
/// reported as warnings; the graph is never mutated.
fn detect_cycles(edges: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut cycles: BTreeSet<String> = BTreeSet::new();
    for start in edges.keys() {
        // Iterative DFS with a per-walk `on_stack` set. Each `start` runs its
        // own DFS so we do not need a global `visited` set (the cost is
        // bounded — graphs are small, sub-linear in entity count).
        let mut stack: Vec<(String, usize)> = vec![(start.clone(), 0)];
        let mut on_stack: BTreeSet<String> = BTreeSet::new();
        on_stack.insert(start.clone());
        while let Some((node, idx)) = stack.last().cloned() {
            let next = edges.get(&node).and_then(|v| v.get(idx)).cloned();
            match next {
                Some(neighbor) => {
                    let last_mut = stack
                        .last_mut()
                        .expect("stack non-empty inside cycle DFS");
                    last_mut.1 += 1;
                    if on_stack.contains(&neighbor) {
                        cycles.insert(neighbor);
                        continue;
                    }
                    on_stack.insert(neighbor.clone());
                    stack.push((neighbor, 0));
                }
                None => {
                    on_stack.remove(&node);
                    stack.pop();
                }
            }
        }
    }
    cycles.into_iter().collect()
}

/// Build the graph index from `<project_root>/.claude/graph/` — **pure**.
///
/// Reads the vault, parses ids/kinds/edges, and records orphan + cycle
/// warnings. Performs **no writes**: the `index.md` MOC and the skill-alias
/// injection are side effects owned by the explicit `graph-index` command
/// ([`materialise_index`]). A missing directory degrades to an empty
/// [`GraphIndex`] (no warnings, no panic) — it is *not* created here.
#[must_use]
pub fn build_index(project_root: &Path) -> GraphIndex {
    let mut index = GraphIndex::default();
    let Ok(paths) = ClaudePaths::for_project(project_root) else {
        return index;
    };
    let graph_dir = paths.graph_dir();
    // A missing vault is the cold-start case → empty index, no disk mutation.
    if !graph_dir.exists() {
        return index;
    }

    for (abs, rel) in collect_markdown(&graph_dir) {
        // Skip the MOC itself — it has no `id:` and is regenerated on demand.
        if rel == "index.md" {
            continue;
        }
        let Ok(content) = mfs::read_to_string(&abs) else {
            continue;
        };
        let Some(id) = parse_frontmatter_id(&content) else {
            index
                .warnings
                .push(format!("warning: {rel} has no frontmatter id — skipped"));
            continue;
        };
        if index.nodes.contains_key(&id) {
            index.warnings.push(format!(
                "warning: duplicate id {id} (second occurrence in {rel})"
            ));
            continue;
        }
        let kind = parse_frontmatter_kind(&content);
        let raw_edges = extract_edges(&content);
        index.edges.insert(id.clone(), raw_edges);
        index.kinds.insert(id.clone(), kind);
        index.nodes.insert(id, rel);
    }

    // Orphan detection — `[[id]]` edge whose target is not in the index.
    for (from, neighbors) in &index.edges {
        for to in neighbors {
            if !index.nodes.contains_key(to) {
                index
                    .warnings
                    .push(format!("warning: orphan edge {from} -> {to}"));
            }
        }
    }

    // Cycle detection — reported as warnings; the adjacency is left intact.
    for cyc in detect_cycles(&index.edges) {
        index
            .warnings
            .push(format!("warning: cycle includes {cyc}"));
    }

    index
}

/// Build the index **and** materialise its side effects: ensure the vault
/// directory exists, inject `aliases:[id]` into matching `SKILL.md` files, and
/// write the `index.md` MOC. Returns the index with `aliased_skills` populated
/// and any write failures appended to `warnings`.
///
/// This is the *only* entry point that writes to disk. It lives here (rather
/// than as a free side effect of [`build_index`]) so the resolver — which calls
/// [`build_index`] on every cache-miss — never rewrites the MOC. The explicit
/// `graph-index` command is the sole caller.
#[must_use]
pub fn materialise_index(project_root: &Path) -> GraphIndex {
    let Ok(paths) = ClaudePaths::for_project(project_root) else {
        return GraphIndex::default();
    };
    let graph_dir = paths.graph_dir();
    // Cold-start: create the vault so the MOC is always materialised.
    if !graph_dir.exists() && mfs::create_dir_all(&graph_dir).is_err() {
        return GraphIndex::default();
    }

    let mut index = build_index(project_root);

    // Inject `aliases:[id]` into matching `SKILL.md` files (best-effort).
    index.aliased_skills = inject_skill_aliases(project_root, &index.nodes);

    // Render + write the MOC. Failures are silent — the in-memory index still
    // wins; the caller can inspect `warnings` for the failure.
    let moc = render_moc(&index.nodes, &index.kinds);
    let moc_path = graph_dir.join("index.md");
    if mfs::write_atomic(&moc_path, moc.as_bytes()).is_err() {
        index
            .warnings
            .push(format!("warning: failed to write {}", moc_path.display()));
    }

    index
}

/// Render the MOC markdown: nodes grouped by `kind`, sorted by id.
fn render_moc(nodes: &BTreeMap<String, String>, id_to_kind: &BTreeMap<String, String>) -> String {
    let mut by_kind: BTreeMap<String, Vec<(&String, &String)>> = BTreeMap::new();
    for (id, rel) in nodes {
        let kind = id_to_kind
            .get(id)
            .cloned()
            .unwrap_or_else(|| "node".to_string());
        by_kind.entry(kind).or_default().push((id, rel));
    }
    let mut out = String::new();
    out.push_str("# Map of Content\n\n");
    let _ = writeln!(out, "Total nodes: **{}**\n", nodes.len());
    if nodes.is_empty() {
        out.push_str("_Empty graph — no concept-nodes yet._\n");
        return out;
    }
    for (kind, mut rows) in by_kind {
        let _ = writeln!(out, "## {kind}\n");
        rows.sort_by(|a, b| a.0.cmp(b.0));
        for (id, rel) in rows {
            let _ = writeln!(out, "- [{id}]({rel})");
        }
        out.push('\n');
    }
    out
}

/// Inject `aliases:[id]` into every `.claude/skills/*/SKILL.md` whose
/// directory name maps to a known skill-kind id. Idempotent: re-running on
/// a SKILL.md that already carries the alias is a no-op.
///
/// The skill id convention mirrors the concept-node convention:
/// `{sub}.skill.{slug}` where `sub = "_root"` for root-level skills and
/// `slug = directory name`.
fn inject_skill_aliases(project_root: &Path, nodes: &BTreeMap<String, String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let Ok(paths) = ClaudePaths::for_project(project_root) else {
        return out;
    };
    let skills_dir = paths.skills_dir();
    let Ok(entries) = mfs::read_dir(&skills_dir) else {
        return out;
    };
    for entry in entries {
        if !entry.is_dir {
            continue;
        }
        let skill_md = entry.path.join("SKILL.md");
        if !skill_md.exists() {
            continue;
        }
        let Ok(body) = mfs::read_to_string(&skill_md) else {
            continue;
        };
        let alias_id = format!("_root.skill.{}", super::interpret::slugify(&entry.file_name));
        // Only inject when the skill id is part of the live graph — keeps the
        // alias surface in sync with what the resolver can dereference.
        if !nodes.contains_key(&alias_id) {
            continue;
        }
        if let Some(updated) = ensure_alias_in_frontmatter(&body, &alias_id) {
            if mfs::write_atomic(&skill_md, updated.as_bytes()).is_ok() {
                out.push(alias_id);
            }
        }
    }
    out
}

/// Append `aliases:[<id>]` to a SKILL.md frontmatter when missing. Returns
/// `None` when the alias is already present (the caller skips the write).
fn ensure_alias_in_frontmatter(body: &str, id: &str) -> Option<String> {
    let stripped = body.strip_prefix("---\n")?;
    let end = stripped.find("\n---")?;
    let (fm, rest) = stripped.split_at(end);
    // Already aliased? Bail (idempotent).
    for line in fm.lines() {
        if let Some(after) = line.strip_prefix("aliases:") {
            if after.contains(id) {
                return None;
            }
        }
    }
    let alias_line = format!("aliases: [{id}]\n");
    let new_fm = if fm.ends_with('\n') {
        format!("{fm}{alias_line}")
    } else {
        format!("{fm}\n{alias_line}")
    };
    Some(format!("---\n{new_fm}{rest}"))
}

// ---------------------------------------------------------------------------
// Wave-5 — backlink write-back, applied inference, dead-node listing
// ---------------------------------------------------------------------------

/// Format a single backlink edge as the on-disk markdown line.
fn format_backlink_line(edge: &SpecBacklinkEdge) -> String {
    match edge.kind {
        EdgeKind::Injected => format!("- [[{}]] <!-- kind: injected -->", edge.target),
        EdgeKind::Applied => {
            let conf = edge.confidence.unwrap_or(0.0);
            // Two-decimal stable format keeps the JSON byte-stable when the
            // section is read back by `parse_backlinks`.
            format!(
                "- [[{}]] <!-- kind: applied confidence: {conf:.2} -->",
                edge.target
            )
        }
    }
}

/// Render the auto-managed `## Backlinks` block (fenced by the open/close
/// markers) given a sorted, deduplicated set of edges.
fn render_backlinks_block(edges: &[SpecBacklinkEdge]) -> String {
    let mut body = String::new();
    body.push_str(BACKLINKS_HEADING);
    body.push_str("\n\n");
    body.push_str(BACKLINKS_OPEN);
    body.push('\n');
    if edges.is_empty() {
        body.push_str("_No backlinks recorded._\n");
    } else {
        for edge in edges {
            body.push_str(&format_backlink_line(edge));
            body.push('\n');
        }
    }
    body.push_str(BACKLINKS_CLOSE);
    body.push('\n');
    body
}

/// Splice a freshly-rendered backlinks block into `body`, replacing any prior
/// auto-managed block. The block lives at the bottom of the file by
/// convention; the splice keeps any user-written content above it untouched.
///
/// Strategy:
/// - If the open/close markers are present, replace the heading + block + the
///   trailing close marker with the new render.
/// - Otherwise, append the new block (with a leading blank line to separate
///   it from prior content).
fn splice_backlinks_block(body: &str, new_block: &str) -> String {
    if let Some(open_idx) = body.find(BACKLINKS_OPEN) {
        // Walk back to the `## Backlinks` heading to drop the heading + a
        // single blank line before the open marker — keeps the file tidy
        // across repeated write-backs.
        let mut start = open_idx;
        if let Some(heading_idx) = body[..open_idx].rfind(BACKLINKS_HEADING) {
            start = heading_idx;
        }
        if let Some(close_offset) = body[open_idx..].find(BACKLINKS_CLOSE) {
            let end = open_idx + close_offset + BACKLINKS_CLOSE.len();
            // Consume the trailing newline of the close marker, if any.
            let end = if body[end..].starts_with('\n') { end + 1 } else { end };
            let mut out = String::with_capacity(body.len() + new_block.len());
            out.push_str(&body[..start]);
            // Trim trailing whitespace before splicing so we do not leave
            // an excess blank line.
            let head = out.trim_end_matches(['\n', ' ']).to_string();
            out.clear();
            out.push_str(&head);
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(new_block);
            out.push_str(&body[end..]);
            return out;
        }
    }

    // No prior block — append at the end with a separating blank line.
    let mut out = body.trim_end_matches(['\n', ' ']).to_string();
    if !out.is_empty() {
        out.push_str("\n\n");
    }
    out.push_str(new_block);
    out
}

/// Parse the auto-managed backlinks block back into structured edges. Used by
/// the write-back step (idempotency check) and by callers that want to read
/// what is already linked. Returns an empty vector when no block exists.
#[must_use]
pub fn parse_backlinks(body: &str) -> Vec<SpecBacklinkEdge> {
    let mut out: Vec<SpecBacklinkEdge> = Vec::new();
    let Some(open) = body.find(BACKLINKS_OPEN) else {
        return out;
    };
    let after = &body[open + BACKLINKS_OPEN.len()..];
    let close = after.find(BACKLINKS_CLOSE).unwrap_or(after.len());
    let block = &after[..close];
    for line in block.lines() {
        let trimmed = line.trim();
        // Match: `- [[id]] <!-- kind: X [confidence: Y] -->`
        let Some(rest) = trimmed.strip_prefix("- [[") else { continue };
        let Some(end_id) = rest.find("]]") else { continue };
        let target = rest[..end_id].to_string();
        let after_id = &rest[end_id + 2..];
        let kind = if after_id.contains("kind: applied") {
            EdgeKind::Applied
        } else if after_id.contains("kind: injected") {
            EdgeKind::Injected
        } else {
            continue;
        };
        let confidence = if kind == EdgeKind::Applied {
            after_id
                .find("confidence:")
                .and_then(|i| {
                    let tail = &after_id[i + "confidence:".len()..];
                    let val: String = tail
                        .trim_start()
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    val.parse::<f32>().ok()
                })
        } else {
            None
        };
        out.push(SpecBacklinkEdge {
            target,
            kind,
            confidence,
        });
    }
    out
}

/// Write a heterogeneous edge set (mix of `injected` + `applied`) into the
/// spec's auto-managed `## Backlinks` block, replacing any prior block.
///
/// The post-EXECUTE write-back ([`crate::commands::event::emit_phase`]) composes
/// the full edge set first via [`merge_edges`] (so a later `applied` inference
/// never demotes a recorded `injected` edge), then calls this once.
///
/// Sorting + dedup is stable: edges sort by target id, then kind (`injected`
/// before `applied`), and an id present as both keeps the `injected` entry
/// (fact wins over inference). The splice replaces the auto-block wholesale, so
/// running it twice with the same edge set leaves the file byte-identical
/// (idempotent). Returns the number of edges written. Fail-open: a missing
/// `spec.md` or any IO error returns `Ok(0)`/the deduped count without
/// aborting a phase transition.
pub fn write_back_edges(spec_path: &Path, edges: &[SpecBacklinkEdge]) -> std::io::Result<usize> {
    // Missing spec.md → nothing to write back; fail-open.
    let Ok(body) = mfs::read_to_string(spec_path) else {
        return Ok(0);
    };
    let mut sorted = edges.to_vec();
    // Stable ordering: by target id (asc), then kind (Injected before Applied
    // since "fact before inference" reads better).
    sorted.sort_by(|a, b| {
        a.target
            .cmp(&b.target)
            .then_with(|| a.kind.as_str().cmp(b.kind.as_str()))
    });
    // Dedup: an id present as both `injected` and `applied` keeps only the
    // injected entry (fact wins over inference).
    let mut seen: BTreeSet<String> = BTreeSet::new();
    let mut deduped: Vec<SpecBacklinkEdge> = Vec::with_capacity(sorted.len());
    for edge in sorted {
        if !seen.insert(edge.target.clone()) {
            // First entry for this id was already pushed; injected sorts
            // first lexicographically (`applied` < `injected` in ASCII, so
            // we explicitly compare).
            if edge.kind == EdgeKind::Injected {
                if let Some(prev) = deduped.iter_mut().find(|e| e.target == edge.target) {
                    *prev = edge;
                }
            }
            continue;
        }
        deduped.push(edge);
    }
    let block = render_backlinks_block(&deduped);
    let updated = splice_backlinks_block(&body, &block);
    if updated == body {
        return Ok(deduped.len());
    }
    mfs::write_atomic(spec_path, updated.as_bytes())
        .map_err(|e| std::io::Error::other(format!("write_back_edges: {e}")))?;
    Ok(deduped.len())
}

/// Merge a new edge set with the edges already present in the spec body.
/// `new_edges` wins on collision when the kind is `Injected`; an existing
/// `Injected` edge is preserved when the new edge is `Applied` (the EXECUTE
/// write-back of `injected` should not be silently demoted by a later
/// `applied` inference).
///
/// The post-EXECUTE write-back ([`crate::commands::event::emit_phase`])
/// composes the `injected` closure with the `applied` inference through this
/// merge before [`write_back_edges`], so a later `applied` inference never
/// silently demotes a recorded `injected` edge.
#[must_use]
pub fn merge_edges(existing: &[SpecBacklinkEdge], new_edges: &[SpecBacklinkEdge]) -> Vec<SpecBacklinkEdge> {
    let mut by_target: BTreeMap<String, SpecBacklinkEdge> = BTreeMap::new();
    for edge in existing.iter().chain(new_edges.iter()) {
        match by_target.get(&edge.target) {
            Some(prev) if prev.kind == EdgeKind::Injected && edge.kind == EdgeKind::Applied => {
                // Keep the injected — fact beats inference.
            }
            _ => {
                by_target.insert(edge.target.clone(), edge.clone());
            }
        }
    }
    by_target.into_values().collect()
}

/// Heuristic `applied` inference: a concept-node is considered applied to a
/// wave's diff when at least one file in `files_changed` lives under (or is
/// referenced by) the node's path or any path mentioned in its body.
///
/// Each match yields a confidence score:
/// - `1.0` when a file in the diff is the exact path the node describes
///   (rare — concept-nodes are usually folders/conventions, not single files).
/// - `0.50` when a file in the diff lives under a folder path mentioned in
///   the node's body (the common case for `conv` nodes describing
///   `apps/rt/src/run/scan/`).
/// - `0.25` when only the path's basename appears in the node body
///   (weakest signal; included so leaves are surfaced for human review).
///
/// `files_changed` are project-relative POSIX-style paths. Nodes the
/// `closure_ids` set already covers as `injected` are skipped — applied is
/// only meaningful for nodes outside the resolver's deterministic closure.
///
/// Called by the post-EXECUTE write-back ([`crate::commands::event::emit_phase`]):
/// when a spec leaves EXECUTE, the files the wave touched (per-spec NDJSON
/// `target.file` + git diff vs the parent branch) are matched against every
/// concept-node, and the `applied` edges are merged with the resolver's
/// `injected` closure before being written back to `spec.md`.
#[must_use]
pub fn infer_applied_edges(
    project_root: &Path,
    files_changed: &[String],
    closure_ids: &[String],
) -> Vec<SpecBacklinkEdge> {
    if files_changed.is_empty() {
        return Vec::new();
    }
    let index = build_index(project_root);
    let injected: BTreeSet<&str> = closure_ids.iter().map(String::as_str).collect();
    let Ok(paths) = ClaudePaths::for_project(project_root) else {
        return Vec::new();
    };
    let graph_root = paths.graph_dir();

    let mut out: Vec<SpecBacklinkEdge> = Vec::new();
    for (id, rel) in &index.nodes {
        if injected.contains(id.as_str()) {
            continue;
        }
        let abs = graph_root.join(rel);
        let body = mfs::read_to_string(&abs).unwrap_or_default();
        let mut best: f32 = 0.0;
        for file in files_changed {
            let normalised = file.replace('\\', "/");
            if body.contains(&normalised) {
                // Exact match — body literally names the changed file.
                best = best.max(1.0);
                continue;
            }
            // Folder-path mention. Walk every `apps/.../path/` token in body.
            for path_like in extract_paths(&body) {
                let p = path_like.trim_end_matches('/');
                if !p.is_empty() && (normalised.starts_with(&format!("{p}/")) || normalised == p) {
                    best = best.max(0.5);
                }
            }
            // Basename mention — weakest signal.
            if let Some(base) = std::path::Path::new(&normalised)
                .file_name()
                .and_then(|s| s.to_str())
            {
                if body.contains(base) && best < 0.25 {
                    best = 0.25;
                }
            }
        }
        if best > 0.0 {
            out.push(SpecBacklinkEdge {
                target: id.clone(),
                kind: EdgeKind::Applied,
                confidence: Some(best),
            });
        }
    }
    out.sort_by(|a, b| a.target.cmp(&b.target));
    out
}

/// Scan a node body for `apps/…/` or `packages/…/` path-like tokens.
/// Called only by [`infer_applied_edges`].
fn extract_paths(body: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for token in body.split(|c: char| c.is_whitespace() || c == '`' || c == '\'' || c == '"' || c == '(' || c == ')') {
        let trimmed = token.trim_matches(|c: char| c == '.' || c == ',' || c == ';' || c == ':');
        if trimmed.starts_with("apps/") || trimmed.starts_with("packages/") {
            // Strip a trailing fragment that is not part of the path.
            let cut: String = trimmed
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || matches!(*c, '/' | '_' | '-' | '.'))
                .collect();
            if !cut.is_empty() {
                out.push(cut);
            }
        }
    }
    out
}

/// List every concept-node id that has no spec backlink. Walks
/// `<project>/.claude/spec/*/spec.md` plus every `wave-*/spec.md`, parses
/// the auto-managed backlinks block, and returns the difference against
/// `GraphIndex::nodes`. Sorted ascending, byte-stable.
#[must_use]
pub fn dead_node_ids(project_root: &Path) -> Vec<String> {
    let index = build_index(project_root);
    let mut linked: BTreeSet<String> = BTreeSet::new();
    let Ok(paths) = ClaudePaths::for_project(project_root) else {
        return Vec::new();
    };
    let spec_root = paths.spec_dir();
    let mut stack: Vec<PathBuf> = vec![spec_root.clone()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = mfs::read_dir(&dir) else {
            continue;
        };
        for entry in entries {
            if entry.is_dir {
                stack.push(entry.path.clone());
                continue;
            }
            if entry.file_name != "spec.md" {
                continue;
            }
            let Ok(body) = mfs::read_to_string(&entry.path) else {
                continue;
            };
            for edge in parse_backlinks(&body) {
                linked.insert(edge.target);
            }
        }
    }
    index
        .nodes
        .keys()
        .filter(|id| !linked.contains(id.as_str()))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn write(path: &Path, body: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, body).unwrap();
    }

    #[test]
    fn extract_edges_recognises_namespaced_ids() {
        let body = "see [[apps-rt.entity.user]] and [[apps-rt.enum.role]]\n[[bare-name]]";
        let edges = extract_edges(body);
        assert_eq!(
            edges,
            vec![
                "apps-rt.entity.user".to_string(),
                "apps-rt.enum.role".to_string(),
                "bare-name".to_string(),
            ]
        );
    }

    /// The single canonical scanner sees every `[[…]]` token (footer context);
    /// the graph consumer's id-charset filter then drops free-text tokens that
    /// are not namespaced concept-ids — so the two systems share one scanner.
    #[test]
    fn extract_edges_filters_free_text_via_concept_id() {
        let body = "free [[my note]] then id [[rt.conv.scan]] and [[bad/slash]]";
        // The shared scanner finds all three tokens (footer-style).
        let scanned = mustard_core::io::atomic_md::scan_links(body);
        assert_eq!(
            scanned,
            vec![
                "my note".to_string(),
                "rt.conv.scan".to_string(),
                "bad/slash".to_string(),
            ]
        );
        // The graph filter keeps only the concept-id.
        let edges = extract_edges(body);
        assert_eq!(edges, vec!["rt.conv.scan".to_string()]);
    }

    #[test]
    fn parse_frontmatter_id_handles_minimal_block() {
        let body = "---\nid: foo.entity.bar\nkind: entity\n---\nbody";
        assert_eq!(parse_frontmatter_id(body).as_deref(), Some("foo.entity.bar"));
        assert_eq!(parse_frontmatter_kind(body), "entity");
        assert!(parse_frontmatter_id("no frontmatter").is_none());
    }

    /// AC-2: id→path resolves every edge; missing targets become warnings;
    /// cycles never panic (recorded as a warning).
    #[test]
    fn graph_validation() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let graph_dir = root.join(".claude").join("graph");

        // a → b → c (clean)
        write(
            &graph_dir.join("foo.entity.a.md"),
            "---\nid: foo.entity.a\nkind: entity\n---\n# A\n[[foo.entity.b]]",
        );
        write(
            &graph_dir.join("foo.entity.b.md"),
            "---\nid: foo.entity.b\nkind: entity\n---\n# B\n[[foo.entity.c]]",
        );
        write(
            &graph_dir.join("foo.entity.c.md"),
            "---\nid: foo.entity.c\nkind: entity\n---\n# C\n",
        );
        // orphan edge target.
        write(
            &graph_dir.join("foo.entity.d.md"),
            "---\nid: foo.entity.d\nkind: entity\n---\n# D\n[[foo.entity.missing]]",
        );
        // cycle: e → f → e
        write(
            &graph_dir.join("foo.entity.e.md"),
            "---\nid: foo.entity.e\nkind: entity\n---\n# E\n[[foo.entity.f]]",
        );
        write(
            &graph_dir.join("foo.entity.f.md"),
            "---\nid: foo.entity.f\nkind: entity\n---\n# F\n[[foo.entity.e]]",
        );

        // Pure build: indexes every node but writes nothing to disk.
        let index = build_index(root);
        assert_eq!(index.nodes.len(), 6, "every well-formed node indexed");
        assert!(
            !root.join(".claude/graph/index.md").exists(),
            "pure build_index must NOT write the MOC"
        );

        // The MOC is materialised only by the explicit side-effecting entry.
        let materialised = materialise_index(root);
        assert_eq!(materialised.nodes.len(), 6, "materialise indexes the same set");
        assert!(
            root.join(".claude/graph/index.md").exists(),
            "materialise_index writes the MOC"
        );

        // Edge id-table coverage: every non-orphan edge target is in the table.
        for (from, neighbors) in &index.edges {
            for to in neighbors {
                if to == "foo.entity.missing" {
                    continue;
                }
                assert!(
                    index.nodes.contains_key(to),
                    "edge {from} -> {to} must resolve"
                );
            }
        }

        let has_orphan = index
            .warnings
            .iter()
            .any(|w| w.contains("orphan edge") && w.contains("foo.entity.missing"));
        assert!(has_orphan, "orphan must surface as a warning");
        let has_cycle = index
            .warnings
            .iter()
            .any(|w| w.contains("cycle includes"));
        assert!(has_cycle, "cycle must surface as a warning");
    }

    /// AC-3: ids must be unique — a duplicate id surfaces as a warning rather
    /// than overwriting the first entry.
    #[test]
    fn graph_ids_unique() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let graph_dir = root.join(".claude").join("graph");

        write(
            &graph_dir.join("foo.entity.a.md"),
            "---\nid: foo.entity.a\nkind: entity\n---\n# A\n",
        );
        // Duplicate id under a different filename.
        write(
            &graph_dir.join("foo.entity.a.dup.md"),
            "---\nid: foo.entity.a\nkind: entity\n---\n# A again\n",
        );
        // Distinct id.
        write(
            &graph_dir.join("foo.entity.b.md"),
            "---\nid: foo.entity.b\nkind: entity\n---\n# B\n",
        );

        let index = build_index(root);
        // Each unique id appears exactly once.
        let ids: BTreeSet<&String> = index.nodes.keys().collect();
        assert_eq!(ids.len(), index.nodes.len());
        assert_eq!(index.nodes.len(), 2, "duplicate id is skipped, not appended");
        let has_dup_warning = index
            .warnings
            .iter()
            .any(|w| w.contains("duplicate id foo.entity.a"));
        assert!(has_dup_warning, "duplicate id must surface as a warning");
    }

    // ----- Wave-5: write-back + edge schema + dead-node + applied -----

    fn seed_spec(root: &Path, slug: &str, body: &str) -> std::path::PathBuf {
        let path = root.join(".claude").join("spec").join(slug).join("spec.md");
        write(&path, body);
        path
    }

    /// Build the `injected` edge set the resolver closure maps to — the same
    /// composition the post-EXECUTE write-back performs before merging in the
    /// `applied` inferences.
    fn injected_edges(ids: &[&str]) -> Vec<SpecBacklinkEdge> {
        ids.iter()
            .map(|id| SpecBacklinkEdge {
                target: (*id).to_string(),
                kind: EdgeKind::Injected,
                confidence: None,
            })
            .collect()
    }

    /// AC-1: write_back_edges writes a `## Backlinks` block carrying every
    /// closure id with `kind: injected`, and re-running with the same closure
    /// produces a byte-identical file (idempotency).
    #[test]
    fn writeback_injected_edges() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let spec = seed_spec(
            root,
            "demo-wave",
            "# Demo\n\n### Stage: Execute\n### Outcome: Active\n\n## Body\nhello\n",
        );
        let closure = injected_edges(&["rt.entity.user", "rt.conv.repo-pattern"]);
        let written = write_back_edges(&spec, &closure).expect("ok");
        assert_eq!(written, 2);
        let body = std::fs::read_to_string(&spec).unwrap();
        assert!(body.contains("## Backlinks"));
        assert!(body.contains("<!-- mustard:backlinks:start -->"));
        assert!(body.contains("[[rt.entity.user]] <!-- kind: injected -->"));
        assert!(body.contains("[[rt.conv.repo-pattern]] <!-- kind: injected -->"));
        // Idempotency: a second write with the same closure leaves the file
        // unchanged byte-for-byte.
        let again = write_back_edges(&spec, &closure).expect("ok");
        assert_eq!(again, 2);
        let body2 = std::fs::read_to_string(&spec).unwrap();
        assert_eq!(body, body2, "second write must be a byte-stable no-op");
        // Round-trip: parse_backlinks recovers the same edge set.
        let parsed = parse_backlinks(&body);
        assert_eq!(parsed.len(), 2);
        for edge in &parsed {
            assert_eq!(edge.kind, EdgeKind::Injected);
            assert!(edge.confidence.is_none());
        }
    }

    /// AC-2: the edge schema distinguishes `injected` (fact) from `applied`
    /// (inferred, with a confidence score). Both reach disk; a re-read
    /// preserves the distinction.
    #[test]
    fn edge_kind_injected_vs_applied() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let spec = seed_spec(root, "edge-demo", "# Edge\n\n## Body\n");
        let edges = vec![
            SpecBacklinkEdge {
                target: "rt.entity.user".to_string(),
                kind: EdgeKind::Injected,
                confidence: None,
            },
            SpecBacklinkEdge {
                target: "rt.conv.repo-pattern".to_string(),
                kind: EdgeKind::Applied,
                confidence: Some(0.5),
            },
        ];
        write_back_edges(&spec, &edges).expect("ok");
        let body = std::fs::read_to_string(&spec).unwrap();
        assert!(body.contains("[[rt.entity.user]] <!-- kind: injected -->"));
        assert!(body.contains("[[rt.conv.repo-pattern]] <!-- kind: applied confidence: 0.50 -->"));

        let parsed = parse_backlinks(&body);
        let injected = parsed
            .iter()
            .find(|e| e.target == "rt.entity.user")
            .expect("injected present");
        assert_eq!(injected.kind, EdgeKind::Injected);
        assert!(injected.confidence.is_none(), "injected has no confidence");
        let applied = parsed
            .iter()
            .find(|e| e.target == "rt.conv.repo-pattern")
            .expect("applied present");
        assert_eq!(applied.kind, EdgeKind::Applied);
        assert_eq!(applied.confidence, Some(0.5));
        // Schema-level sanity: the two kinds serialise to distinct tokens.
        assert_eq!(EdgeKind::Injected.as_str(), "injected");
        assert_eq!(EdgeKind::Applied.as_str(), "applied");
        assert_ne!(EdgeKind::Injected.as_str(), EdgeKind::Applied.as_str());
    }

    /// AC-3: dead_node_ids surfaces concept-nodes that no spec links to.
    #[test]
    fn dead_node_detection() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let graph_dir = root.join(".claude").join("graph");
        write(
            &graph_dir.join("rt.conv.linked.md"),
            "---\nid: rt.conv.linked\nkind: conv\n---\nbody\n",
        );
        write(
            &graph_dir.join("rt.conv.orphan.md"),
            "---\nid: rt.conv.orphan\nkind: conv\n---\nbody\n",
        );
        // One spec backlinks the first node only.
        let spec = seed_spec(root, "live-spec", "# Live\n\n## Body\n");
        write_back_edges(&spec, &injected_edges(&["rt.conv.linked"])).expect("ok");
        let dead = dead_node_ids(root);
        assert!(dead.contains(&"rt.conv.orphan".to_string()));
        assert!(!dead.contains(&"rt.conv.linked".to_string()));
    }

    /// Sanity for the applied inference: a file in the diff that lives under
    /// a folder path mentioned in a node body yields an `applied` edge with
    /// the 0.50 mid-confidence score.
    #[test]
    fn applied_inference_matches_folder_path() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let graph_dir = root.join(".claude").join("graph");
        write(
            &graph_dir.join("rt.conv.scan.md"),
            "---\nid: rt.conv.scan\nkind: conv\n---\nThe scan subsystem lives at apps/rt/src/run/scan/.\n",
        );
        let edges = infer_applied_edges(
            root,
            &["apps/rt/src/run/scan/graph.rs".to_string()],
            &[],
        );
        assert_eq!(edges.len(), 1);
        assert_eq!(edges[0].target, "rt.conv.scan");
        assert_eq!(edges[0].kind, EdgeKind::Applied);
        assert!(edges[0].confidence.unwrap_or(0.0) >= 0.5);
    }

    /// Post-EXECUTE write-back path: given a wave's touched files, the spec's
    /// backlinks carry the `applied` inference ALONGSIDE the resolver's
    /// `injected` closure, and `injected` (fact) wins over `applied`
    /// (inference) on a target collision. This mirrors what
    /// `emit_phase::write_back_after_execute` composes (infer → merge → write),
    /// exercised here without the env-driven `project_dir()`/cache plumbing.
    #[test]
    fn writeback_merges_applied_alongside_injected() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let graph_dir = root.join(".claude").join("graph");
        // Node A: folder path matches the wave's diff → applied-only.
        write(
            &graph_dir.join("rt.conv.scan.md"),
            "---\nid: rt.conv.scan\nkind: conv\n---\nThe scan subsystem lives at apps/rt/src/run/scan/.\n",
        );
        // Node B: also matched by the diff, but ALSO in the injected closure.
        write(
            &graph_dir.join("rt.entity.user.md"),
            "---\nid: rt.entity.user\nkind: entity\n---\nDefined under apps/rt/src/run/scan/.\n",
        );
        let spec = seed_spec(root, "exec-wave", "# Exec\n\n## Body\n");

        // The resolver injected `rt.entity.user` this session (fact).
        let closure_ids = ["rt.entity.user".to_string()];
        let files_changed = ["apps/rt/src/run/scan/graph.rs".to_string()];

        // infer → merge → write, exactly as the post-EXECUTE write-back does.
        let applied = infer_applied_edges(root, &files_changed, &closure_ids);
        // `rt.entity.user` is in the injected closure, so the inference skips it
        // — applied surfaces only the node OUTSIDE the deterministic closure.
        assert!(applied.iter().all(|e| e.target != "rt.entity.user"));
        assert!(applied.iter().any(|e| e.target == "rt.conv.scan"));

        let injected = injected_edges(&["rt.entity.user"]);
        let merged = merge_edges(&injected, &applied);
        write_back_edges(&spec, &merged).expect("ok");

        let body = std::fs::read_to_string(&spec).unwrap();
        // Applied edge is present (inference) with a confidence score.
        assert!(
            body.contains("[[rt.conv.scan]] <!-- kind: applied confidence:"),
            "applied edge missing: {body}"
        );
        // Injected edge is present (fact) with NO confidence.
        assert!(
            body.contains("[[rt.entity.user]] <!-- kind: injected -->"),
            "injected edge missing: {body}"
        );

        // Round-trip: parse_backlinks recovers both, with `injected` winning.
        let parsed = parse_backlinks(&body);
        let user = parsed
            .iter()
            .find(|e| e.target == "rt.entity.user")
            .expect("injected present");
        assert_eq!(user.kind, EdgeKind::Injected, "injected wins over applied");
        assert!(user.confidence.is_none());
        let scan = parsed
            .iter()
            .find(|e| e.target == "rt.conv.scan")
            .expect("applied present");
        assert_eq!(scan.kind, EdgeKind::Applied);
    }

    /// Collision guard at the merge layer: when the SAME target appears as
    /// both `injected` and `applied`, the merged set keeps the `injected`
    /// entry (fact beats inference) — never two edges for one target.
    #[test]
    fn merge_injected_wins_over_applied_on_collision() {
        let injected = injected_edges(&["rt.entity.user"]);
        let applied = vec![SpecBacklinkEdge {
            target: "rt.entity.user".to_string(),
            kind: EdgeKind::Applied,
            confidence: Some(0.5),
        }];
        let merged = merge_edges(&injected, &applied);
        assert_eq!(merged.len(), 1, "one edge per target");
        assert_eq!(merged[0].kind, EdgeKind::Injected);
        assert!(merged[0].confidence.is_none());
    }

    #[test]
    fn skill_alias_injection_is_idempotent() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // Skill on disk.
        let skill = root
            .join(".claude")
            .join("skills")
            .join("my-skill")
            .join("SKILL.md");
        write(
            &skill,
            "---\nname: my-skill\ndescription: \"x\"\n---\nbody",
        );
        // Graph node mirrors the skill id convention.
        let graph_node = root
            .join(".claude")
            .join("graph")
            .join("_root.skill.my-skill.md");
        write(
            &graph_node,
            "---\nid: _root.skill.my-skill\nkind: skill\n---\n# my-skill\n",
        );

        // Alias injection is a write side effect → only `materialise_index`.
        let first = materialise_index(root);
        assert_eq!(first.aliased_skills, vec!["_root.skill.my-skill".to_string()]);
        let after_first = std::fs::read_to_string(&skill).unwrap();
        assert!(after_first.contains("aliases: [_root.skill.my-skill]"));

        // The pure build never injects aliases (no disk mutation).
        let pure = build_index(root);
        assert!(
            pure.aliased_skills.is_empty(),
            "pure build_index must not inject aliases"
        );

        // Second materialise is a no-op — the alias is already there.
        let second = materialise_index(root);
        assert!(second.aliased_skills.is_empty());
        let after_second = std::fs::read_to_string(&skill).unwrap();
        assert_eq!(after_first, after_second, "second run must not rewrite");
    }
}
