//! Deterministic capability DIGEST — a small, AI-sized projection of the model.
//!
//! The full `grain.model.json` is large (every module + declaration + the whole
//! graph). A decomposition/elicitation step (the `feature` flow) must NOT read
//! it — that would blow the low-consumption budget. The digest is the bounded
//! "capability catalog" it queries instead: the recurring slices, roles, shared
//! contracts, registration hubs, the high-fan-in (often *injected*) contracts,
//! the projects, and a domain-term index so a request like "contas a receber"
//! can be looked up by term without reading any source.
//!
//! It is a pure projection of the (deterministic) model, so the digest is
//! deterministic too. Nothing here is language- or framework-specific.

use crate::model::ProjectModel;
use serde::Serialize;
use std::collections::BTreeMap;

/// Caps keep the digest bounded regardless of repo size.
const MAX_ROLES: usize = 30;
const MAX_TOUCHPOINTS: usize = 20;
const MAX_FAN_IN: usize = 15;
const MAX_TERMS: usize = 120;
const MAX_TERM_SAMPLES: usize = 3;
/// Tighter caps for a per-query response so each lookup stays a few KB.
const Q_MAX_TERMS: usize = 25;
const Q_MAX_SLICES: usize = 12;
const Q_MAX_HUBS: usize = 8;
const Q_MAX_TOUCHPOINTS: usize = 10;

#[derive(Serialize)]
pub struct CapabilityDigest {
    pub root: String,
    pub languages: Vec<LangD>,
    pub frameworks: Vec<String>,
    pub projects: Vec<ProjD>,
    /// Top role affixes by frequency; `roles_omitted` is the truncated tail.
    pub roles: Vec<RoleD>,
    pub roles_omitted: usize,
    /// Recurring vertical slices — the build patterns available to compose.
    pub slices: Vec<SliceD>,
    /// Base types many entities inherit/implement (mined supertypes).
    pub shared_contracts: Vec<ContractD>,
    pub graph: GraphD,
    /// Domain-term index: token -> frequency + sample files. The search surface
    /// for mapping a free-text request onto where it lives in the repo.
    pub terms: Vec<TermD>,
}

#[derive(Serialize)]
pub struct LangD {
    pub language: String,
    pub files: usize,
    pub loc: usize,
}

#[derive(Serialize)]
pub struct ProjD {
    pub name: String,
    pub dir: String,
    pub kind: String,
    pub code_files: usize,
}

#[derive(Serialize)]
pub struct RoleD {
    pub affix: String,
    pub kind: String,
    pub count: usize,
    pub common_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub implements: Option<String>,
}

#[derive(Serialize)]
pub struct SliceD {
    /// Core role affixes joined with '+', e.g. "Handler+Validator".
    pub label: String,
    pub recurrence: usize,
    pub confidence: f32,
    pub entities: Vec<String>,
    pub optional_roles: Vec<String>,
}

#[derive(Serialize)]
pub struct ContractD {
    pub name: String,
    pub implementors: usize,
}

#[derive(Serialize)]
pub struct GraphD {
    pub nodes: usize,
    pub edges: usize,
    pub cyclic: bool,
    pub layers: Vec<LayerD>,
    pub touchpoints: Vec<TouchD>,
    /// Highest fan-in modules — where cross-cutting *injected* contracts live
    /// (e.g. a current-tenant accessor). These never show up as `shared_contracts`
    /// because they are dependencies, not supertypes, so they are surfaced here.
    pub top_fan_in: Vec<HubD>,
}

#[derive(Serialize)]
pub struct LayerD {
    pub name: String,
    pub modules: usize,
}

#[derive(Serialize)]
pub struct TouchD {
    pub module: String,
    pub fan_out: usize,
    pub breadth: usize,
}

#[derive(Serialize)]
pub struct HubD {
    pub module: String,
    pub degree: usize,
}

#[derive(Serialize)]
pub struct TermD {
    pub term: String,
    pub count: usize,
    pub samples: Vec<String>,
}

/// A focused slice of the digest matching some domain terms — the cheap
/// per-interaction lookup a `feature` step does (a few KB, not the whole
/// catalog). `miss=true` means nothing matched: the caller MUST NOT conclude
/// "no precedent" (the term index has false negatives) — it should confirm by
/// reading, or treat the area as net-new.
#[derive(Serialize)]
pub struct QueryResult {
    pub query: Vec<String>,
    pub matched_terms: Vec<TermD>,
    /// Terms that matched but were trimmed by the per-query cap (no silent loss).
    pub terms_omitted: usize,
    pub slices: Vec<SliceD>,
    pub contracts: Vec<ContractD>,
    /// High fan-in modules whose path carries a query term — surfaces *injected*
    /// cross-cutting contracts (e.g. a current-tenant accessor) for `--invariant`.
    pub hubs: Vec<HubD>,
    pub touchpoints: Vec<TouchD>,
    /// Real files to read next (anchor candidates): hubs (injected contracts)
    /// first, then matched-term samples, then touchpoints — so this is never
    /// empty when something matched. The handful the feature reads for ground
    /// truth instead of the repo.
    pub files: Vec<String>,
    pub miss: bool,
}

/// Whole-token match between a repo token `tk` and a query token `q`, with a
/// length-gated prefix EITHER way so "charge"~"charges" match but a short token
/// like "tax" never matches "taxonomy". Same whole-token discipline the spec
/// compiler uses (`spec::token_seq_in`) — one matching philosophy in the crate.
fn token_match(tk: &str, q: &str) -> bool {
    tk == q || (q.len() >= 4 && tk.starts_with(q)) || (tk.len() >= 4 && q.starts_with(tk))
}

/// Look up the digest by domain term(s) — OR across terms. Returns only the
/// matching slice (a few KB, capped) so the caller spends little per
/// interaction. Query terms shorter than 3 chars are ignored (mirrors the
/// mined-token floor). Deterministic.
pub fn query(model: &ProjectModel, terms: &[String]) -> QueryResult {
    let dig = build(model);
    let ql: Vec<String> = terms.iter().map(|s| s.trim().to_lowercase()).filter(|s| s.len() >= 3).collect();
    // A name/path "hits" when any of its tokens matches any query token.
    let hit = |hay: &str| {
        let toks = tokenize(hay);
        ql.iter().any(|q| toks.iter().any(|tk| token_match(tk, q)))
    };

    let mut matched_terms: Vec<TermD> = dig.terms.into_iter().filter(|t| ql.iter().any(|q| token_match(&t.term, q))).collect();
    let terms_omitted = matched_terms.len().saturating_sub(Q_MAX_TERMS);
    matched_terms.truncate(Q_MAX_TERMS);

    let mut slices: Vec<SliceD> = dig.slices.into_iter().filter(|s| hit(&s.label) || s.entities.iter().any(|e| hit(e))).collect();
    slices.truncate(Q_MAX_SLICES);
    let contracts: Vec<ContractD> = dig.shared_contracts.into_iter().filter(|c| hit(&c.name)).collect();
    let mut hubs: Vec<HubD> = dig.graph.top_fan_in.into_iter().filter(|h| hit(&h.module)).collect();
    hubs.truncate(Q_MAX_HUBS);
    let mut touchpoints: Vec<TouchD> = dig.graph.touchpoints.into_iter().filter(|t| hit(&t.module)).collect();
    touchpoints.truncate(Q_MAX_TOUCHPOINTS);

    // Anchor candidates in priority order (never empty when something matched):
    // hubs (injected contracts) -> matched-term samples -> touchpoints. Order-
    // preserving dedup keeps the priority; capped to ~a dozen.
    let mut files: Vec<String> = Vec::new();
    let src = hubs
        .iter()
        .map(|h| h.module.clone())
        .chain(matched_terms.iter().flat_map(|t| t.samples.iter().cloned()))
        .chain(touchpoints.iter().map(|t| t.module.clone()));
    for m in src {
        if !files.contains(&m) {
            files.push(m);
        }
    }
    files.truncate(12);

    let miss = matched_terms.is_empty() && slices.is_empty() && contracts.is_empty() && hubs.is_empty() && touchpoints.is_empty();
    QueryResult { query: ql, matched_terms, terms_omitted, slices, contracts, hubs, touchpoints, files, miss }
}

/// Project the full model down to the bounded capability digest.
pub fn build(model: &ProjectModel) -> CapabilityDigest {
    let languages = model.languages.iter().map(|l| LangD { language: l.language.clone(), files: l.files, loc: l.loc }).collect();
    let frameworks = model.frameworks.clone();
    let projects = model.projects.iter().map(|p| ProjD { name: p.name.clone(), dir: p.dir.clone(), kind: p.kind.clone(), code_files: p.code_files }).collect();

    // Roles: top by count (stable tie-break by affix), tail counted not dropped silently.
    let mut roles_sorted: Vec<&crate::model::RoleStat> = model.roles.iter().collect();
    roles_sorted.sort_by(|a, b| b.count.cmp(&a.count).then(a.affix.cmp(&b.affix)));
    let roles_omitted = roles_sorted.len().saturating_sub(MAX_ROLES);
    let roles = roles_sorted
        .iter()
        .take(MAX_ROLES)
        .map(|r| RoleD { affix: r.affix.clone(), kind: r.kind.clone(), count: r.count, common_dir: r.common_dir.clone(), implements: r.implements.clone() })
        .collect();

    // Slices: the multi-role conventions, trimmed (drop the verbose steps/examples).
    let mut slices: Vec<SliceD> = model
        .conventions
        .iter()
        .filter(|c| c.is_slice)
        .map(|c| SliceD {
            label: c.roles.iter().map(|s| s.as_str()).filter(|r| *r != "(core)").collect::<Vec<_>>().join("+"),
            recurrence: c.recurrence,
            confidence: c.confidence,
            entities: c.entities.iter().take(5).cloned().collect(),
            optional_roles: c.optional_roles.clone(),
        })
        .collect();
    slices.sort_by(|a, b| b.recurrence.cmp(&a.recurrence).then(a.label.cmp(&b.label)));

    let shared_contracts = model.shared_contracts.iter().map(|s| ContractD { name: s.name.clone(), implementors: s.implementors }).collect();

    let mut top_fan_in: Vec<HubD> = model.graph.top_fan_in.iter().map(|n| HubD { module: n.module.clone(), degree: n.degree }).collect();
    top_fan_in.sort_by(|a, b| b.degree.cmp(&a.degree).then(a.module.cmp(&b.module)));
    top_fan_in.truncate(MAX_FAN_IN);

    let mut touchpoints: Vec<TouchD> = model.graph.touchpoints.iter().map(|t| TouchD { module: t.module.clone(), fan_out: t.fan_out, breadth: t.breadth }).collect();
    touchpoints.sort_by(|a, b| b.fan_out.cmp(&a.fan_out).then(a.module.cmp(&b.module)));
    touchpoints.truncate(MAX_TOUCHPOINTS);

    let graph = GraphD {
        nodes: model.graph.nodes,
        edges: model.graph.edges,
        cyclic: model.graph.cyclic,
        layers: model.graph.layers.iter().map(|l| LayerD { name: l.name.clone(), modules: l.modules }).collect(),
        touchpoints,
        top_fan_in,
    };

    let terms = build_terms(model);

    CapabilityDigest { root: model.root.clone(), languages, frameworks, projects, roles, roles_omitted, slices, shared_contracts, graph, terms }
}

/// Build the domain-term index from declaration names (the repo's own vocabulary).
fn build_terms(model: &ProjectModel) -> Vec<TermD> {
    // term -> (count, sample module paths). BTreeMap for deterministic iteration.
    let mut index: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    for m in &model.modules {
        for d in &m.declarations {
            for tok in tokenize(&d.name) {
                let e = index.entry(tok).or_insert((0, Vec::new()));
                e.0 += 1;
                if e.1.len() < MAX_TERM_SAMPLES && !e.1.contains(&m.path) {
                    e.1.push(m.path.clone());
                }
            }
        }
    }
    let mut terms: Vec<TermD> = index.into_iter().map(|(term, (count, samples))| TermD { term, count, samples }).collect();
    terms.sort_by(|a, b| b.count.cmp(&a.count).then(a.term.cmp(&b.term)));
    terms.truncate(MAX_TERMS);
    terms
}

/// Split an identifier into lowercased domain tokens on case boundaries and
/// non-alphanumerics, handling acronyms. Splits on lower/digit -> Upper AND on
/// Upper -> Upper-followed-by-lower, so "ICurrentTenant" -> ["current","tenant"]
/// and "HTTPServer" -> ["http","server"]. Drops glue tokens (<3 chars) so
/// "ListTransfersByTenantId" yields ["list","transfers","tenant"]. Shared with
/// the spec compiler. No language/framework knowledge.
pub(crate) fn tokenize(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for i in 0..chars.len() {
        let ch = chars[i];
        if !ch.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if !cur.is_empty() {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let boundary =
                // camelCase / digit -> Upper:  "fooBar" -> foo|Bar
                (ch.is_uppercase() && (prev.is_lowercase() || prev.is_ascii_digit()))
                // acronym -> word:  "HTTPServer" -> HTTP|Server
                || (ch.is_uppercase() && prev.is_uppercase() && next.is_some_and(|n| n.is_lowercase()));
            if boundary {
                out.push(std::mem::take(&mut cur));
            }
        }
        cur.push(ch);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out.into_iter().map(|s| s.to_lowercase()).filter(|s| s.len() >= 3 && s.chars().any(|c| c.is_alphabetic())).collect()
}
