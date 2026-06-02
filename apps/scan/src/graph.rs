//! Layer 3 — Dependency graph.
//!
//! The declared architecture lives in folder names; the *real* architecture
//! lives in the import edges. We resolve imports to internal modules and ask
//! objective, vocabulary-free questions: are there cycles? god modules? and
//! what is the *emergent* layering — i.e. how deep is each module in the
//! dependency order the code itself defines?
//!
//! Layering is derived, not named: condense cycles into a DAG, then take each
//! module's longest dependency chain as its depth (L0 = most depended-upon /
//! innermost). The only direction-violation topology can prove without a
//! hardcoded layer vocabulary is a dependency cycle, so that is what we count.
//!
//! Resolution is heuristic and language-agnostic: every import is tried against
//! a union of resolution shapes and the ones that don't apply return nothing.
//!   * namespace/package match — an import that names a declared namespace
//!     (the shape used by C#, Java, Kotlin, ...);
//!   * module-prefixed path — strip a declared module prefix, match a directory
//!     (the shape used by Go);
//!   * file path — resolve a relative/path-ish import to a module file (the
//!     shape used by TS/JS, Dart, Python, ...).
//! Nothing here switches on a language name, so a new language needs no change.
//! Imports that resolve to nothing internal are treated as external deps.

use crate::model::{GraphStats, LayerInfo, Module, NodeDegree, Touchpoint};
use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::Direction;
use std::collections::{BTreeMap, HashMap, HashSet};

/// Longest dependency chain below an SCC = its emergent depth (L0 = innermost).
fn scc_depth(c: usize, succ: &[HashSet<usize>], memo: &mut [Option<usize>]) -> usize {
    if let Some(d) = memo[c] {
        return d;
    }
    memo[c] = Some(0); // DAG guard; condensed graph has no cycles
    let mut d = 0;
    for &s in &succ[c] {
        d = d.max(1 + scc_depth(s, succ, memo));
    }
    memo[c] = Some(d);
    d
}


pub fn build(modules: &[Module], go_module: &Option<String>) -> (GraphStats, HashMap<String, (usize, usize)>, HashMap<String, usize>) {
    let mut g: DiGraph<String, ()> = DiGraph::new();
    let mut idx: HashMap<String, NodeIndex> = HashMap::new();
    for m in modules {
        let n = g.add_node(m.path.clone());
        idx.insert(m.path.clone(), n);
    }

    // Indexes for resolution.
    let mut ns_index: HashMap<String, Vec<String>> = HashMap::new(); // namespace -> module paths
    let mut dir_index: HashMap<String, Vec<String>> = HashMap::new(); // dir -> module paths
    for m in modules {
        for ns in &m.namespaces {
            ns_index.entry(ns.clone()).or_default().push(m.path.clone());
        }
        let dir = parent_dir(&m.path);
        dir_index.entry(dir).or_default().push(m.path.clone());
    }
    let module_paths: HashSet<&str> = modules.iter().map(|m| m.path.as_str()).collect();
    let stem_index = build_stem_index(modules);

    let mut edge_set: HashSet<(NodeIndex, NodeIndex)> = HashSet::new();

    for m in modules {
        let src = idx[&m.path];
        for imp in &m.imports {
            let targets = resolve(imp, &m.path, &ns_index, &stem_index, &dir_index, &module_paths, go_module);
            for t in targets {
                if let Some(&dst) = idx.get(&t) {
                    if dst != src {
                        edge_set.insert((src, dst));
                    }
                }
            }
        }
    }
    for (a, b) in &edge_set {
        g.add_edge(*a, *b, ());
    }

    // Cycles via SCC.
    let sccs = petgraph::algo::tarjan_scc(&g);
    let mut cycles: Vec<Vec<String>> = Vec::new();
    for scc in &sccs {
        if scc.len() > 1 {
            cycles.push(scc.iter().map(|n| g[*n].clone()).collect());
        }
    }
    let self_loop = edge_set.iter().any(|(a, b)| a == b);
    let cyclic = !cycles.is_empty() || self_loop;

    // Fan-in / fan-out.
    let mut fan_in: Vec<NodeDegree> = Vec::new();
    let mut fan_out: Vec<NodeDegree> = Vec::new();
    let mut degree_map: HashMap<String, (usize, usize)> = HashMap::new();
    for n in g.node_indices() {
        let fi = g.neighbors_directed(n, Direction::Incoming).count();
        let fo = g.neighbors_directed(n, Direction::Outgoing).count();
        degree_map.insert(g[n].clone(), (fi, fo));
        if fi > 0 {
            fan_in.push(NodeDegree { module: g[n].clone(), degree: fi });
        }
        if fo > 0 {
            fan_out.push(NodeDegree { module: g[n].clone(), degree: fo });
        }
    }
    fan_in.sort_by(|a, b| b.degree.cmp(&a.degree));
    fan_out.sort_by(|a, b| b.degree.cmp(&a.degree));
    fan_in.truncate(8);
    fan_out.truncate(8);

    // Emergent layering: condense cycles into a DAG, then depth = longest
    // dependency chain. No layer names — just the order the imports define.
    let mut scc_of = vec![0usize; g.node_count()];
    for (i, comp) in sccs.iter().enumerate() {
        for &n in comp {
            scc_of[n.index()] = i;
        }
    }
    let mut succ: Vec<HashSet<usize>> = vec![HashSet::new(); sccs.len()];
    let mut cyclic_edges = 0usize;
    for (a, b) in &edge_set {
        let (ca, cb) = (scc_of[a.index()], scc_of[b.index()]);
        if ca != cb {
            succ[ca].insert(cb);
        } else if sccs[ca].len() > 1 || a == b {
            cyclic_edges += 1; // a dependency that closes a cycle
        }
    }
    let mut memo = vec![None; sccs.len()];
    let mut per_depth: BTreeMap<usize, usize> = BTreeMap::new();
    let mut depth_by_path: HashMap<String, usize> = HashMap::new();
    for n in g.node_indices() {
        let d = scc_depth(scc_of[n.index()], &succ, &mut memo);
        *per_depth.entry(d).or_default() += 1;
        depth_by_path.insert(g[n].clone(), d);
    }
    let layers: Vec<LayerInfo> = per_depth
        .into_iter()
        .map(|(d, modules)| LayerInfo { name: format!("L{d}"), modules })
        .collect();

    // Touchpoints: hubs that import across many directories — the registration
    // points you edit when adding an entity (DI container, menu, barrels). Ranked
    // by breadth (distinct dirs imported) then fan-out; tests excluded because
    // they import broadly but register nothing. Frequency-derived, no catalog.
    let mut src_targets: HashMap<&str, Vec<&str>> = HashMap::new();
    for (a, b) in &edge_set {
        src_targets.entry(g[*a].as_str()).or_default().push(g[*b].as_str());
    }
    let mut touchpoints: Vec<Touchpoint> = src_targets
        .iter()
        .filter(|(src, _)| !is_test_path(src))
        .map(|(src, tgts)| {
            let breadth = tgts.iter().map(|t| parent_dir(t)).collect::<HashSet<_>>().len();
            Touchpoint { module: (*src).to_string(), fan_out: tgts.len(), breadth }
        })
        .collect();
    touchpoints.sort_by(|a, b| b.breadth.cmp(&a.breadth).then(b.fan_out.cmp(&a.fan_out)).then(a.module.cmp(&b.module)));
    touchpoints.truncate(120); // keep enough so per-project hubs (e.g. a frontend menu) aren't crowded out by a larger project

    let stats = GraphStats {
        nodes: g.node_count(),
        edges: edge_set.len(),
        cyclic,
        cycles,
        top_fan_in: fan_in,
        top_fan_out: fan_out,
        layers,
        cyclic_edges,
        total_edges: edge_set.len(),
        touchpoints,
    };
    (stats, degree_map, depth_by_path)
}

fn build_stem_index(modules: &[Module]) -> HashMap<String, Vec<String>> {
    let mut m: HashMap<String, Vec<String>> = HashMap::new();
    for module in modules {
        let stem = strip_ext(&module.path);
        m.entry(stem).or_default().push(module.path.clone());
    }
    m
}

#[allow(clippy::too_many_arguments)]
fn resolve(
    imp: &str,
    from: &str,
    ns_index: &HashMap<String, Vec<String>>,
    stem_index: &HashMap<String, Vec<String>>,
    dir_index: &HashMap<String, Vec<String>>,
    module_paths: &HashSet<&str>,
    go_module: &Option<String>,
) -> Vec<String> {
    // Try every resolution shape; whichever applies wins. No language switch.
    // 1) Namespace/package match: the import names a declared namespace (the
    //    common case for namespace languages — a using shared by many files).
    if let Some(v) = ns_index.get(imp) {
        return v.clone();
    }
    // 2) Module-prefixed path: strip a declared module prefix and match the
    //    directory it points at (the import-as-package-path shape).
    if let Some(modpath) = go_module {
        if let Some(rest) = imp.strip_prefix(modpath.as_str()) {
            let rest = rest.trim_start_matches('/');
            if let Some(v) = dir_index.get(rest) {
                return v.clone();
            }
        }
    }
    // 3) File path: a relative or path-ish import resolved to a module file.
    let cleaned = imp.strip_prefix("package:").unwrap_or(imp);
    if cleaned.starts_with('.') {
        let joined = join_relative(from, cleaned);
        resolve_path_candidate(&joined, stem_index, dir_index, module_paths)
    } else if cleaned.starts_with('/') || cleaned.contains('/') {
        resolve_path_candidate(cleaned, stem_index, dir_index, module_paths)
    } else {
        Vec::new()
    }
}

fn resolve_path_candidate(
    cand: &str,
    stem_index: &HashMap<String, Vec<String>>,
    dir_index: &HashMap<String, Vec<String>>,
    module_paths: &HashSet<&str>,
) -> Vec<String> {
    let cand = normalize(cand);
    if module_paths.contains(cand.as_str()) {
        return vec![cand];
    }
    let stem = strip_ext(&cand);
    if let Some(v) = stem_index.get(&stem) {
        return v.clone();
    }
    // directory import -> index file
    for index in ["index", "main", "mod"] {
        let probe = format!("{stem}/{index}");
        if let Some(v) = stem_index.get(&probe) {
            return v.clone();
        }
    }
    // dart package suffix: match any module whose path ends with the candidate
    let suffix_matches: Vec<String> = stem_index
        .iter()
        .filter(|(k, _)| k.ends_with(&stem))
        .flat_map(|(_, v)| v.clone())
        .collect();
    if !suffix_matches.is_empty() {
        return suffix_matches;
    }
    let _ = dir_index;
    Vec::new()
}

fn parent_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(i) => path[..i].to_string(),
        None => String::new(),
    }
}

/// Path-segment test detection (language-agnostic): a file under a test/mock/
/// fixture folder, or named `*.test.*`/`*.spec.*`.
fn is_test_path(p: &str) -> bool {
    let l = p.to_lowercase();
    if l.contains(".test.") || l.contains(".spec.") {
        return true;
    }
    l.split('/').any(|s| matches!(s, "test" | "tests" | "__tests__" | "mocks" | "fixtures" | "spec" | "specs"))
}

fn strip_ext(path: &str) -> String {
    match path.rfind('.') {
        Some(i) if !path[i..].contains('/') => path[..i].to_string(),
        _ => path.to_string(),
    }
}

fn join_relative(from: &str, rel: &str) -> String {
    let base = parent_dir(from);
    let mut parts: Vec<&str> = base.split('/').filter(|s| !s.is_empty()).collect();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for seg in path.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}
