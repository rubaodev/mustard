//! `scan-guards-list` — enumerate every subproject `CLAUDE.md` whose `## Guards`
//! block is still `pending` and emit a JSON worklist for the enrich agent.
//!
//! A file is *pending* iff it contains [`scan_claude::GUARDS_PENDING_OPEN`]. The
//! workspace-root `CLAUDE.md` (the unit whose directory is the repo root) is
//! excluded — Wave 1 never seeds the pending block there. For each pending
//! file the facts line (`<!-- facts: kind=...; frameworks=... -->`) is parsed so
//! the agent has grounding context.
//!
//! Output: a JSON array `[{path, subproject, kind, frameworks}]` to stdout.
//! Fail-open: any IO error degrades to `[]` and exit 0.

use std::path::Path;

use mustard_core::io::fs;
use serde_json::{json, Value};

use crate::commands::scan_claude::GUARDS_PENDING_OPEN;

/// Directories never descended into — mirrors `docs_stale_check::IGNORE_DIRS`
/// so the walk stays cheap and never explodes into build/vendor trees.
const IGNORE_DIRS: &[&str] = &[
    "node_modules", ".git", "dist", "bin", "obj", ".next", "vendor",
    "__pycache__", ".nuxt", ".output", "build", "coverage", "target",
    "migrations", ".vs", ".idea", "worktrees", ".worktrees",
];

/// Directory recursion depth cap — a working copy this deep is pathological.
const MAX_DEPTH: usize = 12;

/// One pending-guards worklist entry.
struct Pending {
    /// Path to the `CLAUDE.md`, as a string (lossy on non-UTF-8).
    path: String,
    /// Subproject directory relative to `root` (forward-slashed). Empty for the
    /// root unit — but the root is excluded, so this is always non-empty here.
    subproject: String,
    /// Project kind mined by Wave 1 (e.g. `rust`).
    kind: String,
    /// Frameworks mined by Wave 1, in caller order. Empty when none.
    frameworks: Vec<String>,
}

/// Run `scan-guards-list`. Prints a JSON array to stdout; exit 0 always.
pub fn run(root: &Path) {
    let mut out: Vec<Pending> = Vec::new();
    walk(root, root, &mut out, 0);
    // Stable order so the worklist is deterministic across runs.
    out.sort_by(|a, b| a.path.cmp(&b.path));
    let arr: Vec<Value> = out
        .iter()
        .map(|p| {
            json!({
                "path": p.path,
                "subproject": p.subproject,
                "kind": p.kind,
                "frameworks": p.frameworks,
            })
        })
        .collect();
    // `to_string` cannot fail for this shape; fall back to `[]` defensively.
    println!("{}", serde_json::to_string(&arr).unwrap_or_else(|_| "[]".to_string()));
}

/// Recursively walk `dir`, collecting pending subproject `CLAUDE.md` files.
/// Fail-open: an unreadable directory is skipped, never propagated.
fn walk(dir: &Path, root: &Path, out: &mut Vec<Pending>, depth: usize) {
    if depth > MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries {
        if entry.is_dir {
            let name = entry.file_name.as_str();
            if IGNORE_DIRS.contains(&name) {
                continue;
            }
            // Hidden dirs are skipped (build caches, VCS); `.claude` holds no
            // CLAUDE.md of its own, so excluding all dot-dirs is safe here.
            if name.starts_with('.') {
                continue;
            }
            walk(&entry.path, root, out, depth + 1);
        } else if entry.file_name == "CLAUDE.md" {
            if let Some(p) = classify(&entry.path, root) {
                out.push(p);
            }
        }
    }
}

/// Classify a `CLAUDE.md`: returns a [`Pending`] entry iff the file carries the
/// pending marker AND is NOT the workspace-root unit. `None` otherwise.
fn classify(path: &Path, root: &Path) -> Option<Pending> {
    // The root `CLAUDE.md` (directly under `root`) is excluded from enrich.
    let subproject = subproject_of(path, root);
    if subproject.is_empty() {
        return None;
    }
    let text = fs::read_to_string(path).ok()?;
    if !text.contains(GUARDS_PENDING_OPEN) {
        return None;
    }
    let (kind, frameworks) = parse_facts(&text);
    Some(Pending {
        path: path.to_string_lossy().into_owned(),
        subproject,
        kind,
        frameworks,
    })
}

/// The subproject directory of a `CLAUDE.md`, relative to `root`, forward-
/// slashed. Empty when the file sits directly in `root` (the root unit).
///
/// Single-sourced root rule: a `CLAUDE.md` is the workspace-root unit iff
/// `subproject_of(path, root).is_empty()`. Both `list` (worklist exclusion) and
/// `apply` (root refusal) classify against this same helper so they never drift.
pub(crate) fn subproject_of(claude_md: &Path, root: &Path) -> String {
    let Some(parent) = claude_md.parent() else {
        return String::new();
    };
    match parent.strip_prefix(root) {
        Ok(rel) => rel.to_string_lossy().replace('\\', "/"),
        // Outside `root` (should not happen for a tree walked from `root`) — treat
        // as a subproject so it is not silently dropped.
        Err(_) => parent.to_string_lossy().replace('\\', "/"),
    }
}

/// Parse the `<!-- facts: kind=...; frameworks=a, b -->` line Wave 1 emits.
/// Returns `(kind, frameworks)`; missing fields degrade to `("", vec![])`.
/// `frameworks=(none)` (Wave 1's empty sentinel) yields an empty vec.
fn parse_facts(text: &str) -> (String, Vec<String>) {
    let Some(line) = text.lines().find(|l| l.trim_start().starts_with("<!-- facts:")) else {
        return (String::new(), Vec::new());
    };
    // Strip the comment delimiters and the `facts:` prefix.
    let inner = line
        .trim()
        .trim_start_matches("<!--")
        .trim_end_matches("-->")
        .trim()
        .trim_start_matches("facts:")
        .trim();

    let mut kind = String::new();
    let mut frameworks: Vec<String> = Vec::new();
    for field in inner.split(';') {
        let field = field.trim();
        if let Some(v) = field.strip_prefix("kind=") {
            kind = v.trim().to_string();
        } else if let Some(v) = field.strip_prefix("frameworks=") {
            let v = v.trim();
            if v != "(none)" && !v.is_empty() {
                frameworks = v
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
            }
        }
    }
    (kind, frameworks)
}

/// Collect (without printing) the pending worklist — the testable core of
/// [`run`]. Kept private to the module's tests.
#[cfg(test)]
fn run_collect(root: &Path) -> Vec<Pending> {
    let mut out: Vec<Pending> = Vec::new();
    walk(root, root, &mut out, 0);
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::scan_claude::GUARDS_CLOSE;

    fn pending_block(kind: &str, fw: &str) -> String {
        format!(
            "# Sub\n\n## Guards\n\n{GUARDS_PENDING_OPEN}\n<!-- facts: kind={kind}; frameworks={fw} -->\n{GUARDS_CLOSE}\n"
        )
    }

    #[test]
    fn scan_guards_list_finds_pending_and_excludes_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Root CLAUDE.md carries the pending marker too — but must be EXCLUDED.
        std::fs::write(root.join("CLAUDE.md"), pending_block("rust", "serde")).unwrap();

        // A pending subproject.
        let sub = root.join("apps").join("rt");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("CLAUDE.md"), pending_block("rust", "serde, clap")).unwrap();

        // An already-enriched subproject (no pending marker) — skipped.
        let done = root.join("apps").join("done");
        std::fs::create_dir_all(&done).unwrap();
        std::fs::write(
            done.join("CLAUDE.md"),
            format!("# Done\n\n## Guards\n\n{}\n<!-- facts: kind=rust; frameworks=(none) -->\n{GUARDS_CLOSE}\n",
                crate::commands::scan_claude::GUARDS_DONE_OPEN),
        )
        .unwrap();

        // A build dir that must never be descended.
        let ignored = root.join("target").join("debug");
        std::fs::create_dir_all(&ignored).unwrap();
        std::fs::write(ignored.join("CLAUDE.md"), pending_block("rust", "x")).unwrap();

        let found = run_collect(root);
        assert_eq!(found.len(), 1, "exactly the pending subproject: {:?}", found.iter().map(|p| &p.path).collect::<Vec<_>>());
        let p = &found[0];
        assert_eq!(p.subproject, "apps/rt");
        assert_eq!(p.kind, "rust");
        assert_eq!(p.frameworks, vec!["serde".to_string(), "clap".to_string()]);
    }

    #[test]
    fn scan_guards_list_parses_none_frameworks() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let sub = root.join("packages").join("lib");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(sub.join("CLAUDE.md"), pending_block("rust", "(none)")).unwrap();

        let found = run_collect(root);
        assert_eq!(found.len(), 1);
        assert!(found[0].frameworks.is_empty(), "(none) sentinel must yield an empty vec");
        assert_eq!(found[0].kind, "rust");
    }

    #[test]
    fn scan_guards_list_empty_when_no_pending() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        // Only the root carries a pending marker → excluded → empty worklist.
        std::fs::write(root.join("CLAUDE.md"), pending_block("rust", "serde")).unwrap();
        assert!(run_collect(root).is_empty());
    }

    #[test]
    fn parse_facts_handles_missing_line() {
        let (kind, fw) = parse_facts("# No facts here\n## Guards\n");
        assert!(kind.is_empty());
        assert!(fw.is_empty());
    }
}
