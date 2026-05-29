//! Deterministic `guards.md` SEED generation (min-IA / max-Rust).
//!
//! A sibling of [`crate::commands::scan::scan_structural`] (stack.md) and
//! [`crate::commands::scan::scan_skill_render`] (per-cluster SKILL.md): the scan
//! orchestrator calls this to write each changed subproject's `guards.md` from
//! deterministic signals only — the architecture style the registry detected
//! (`_patterns.{stack}.architecture`) turned into boundary rules, plus a "follow
//! the discovered convention" pointer per cluster. NO LLM.
//!
//! The `--enrich` agent later APPENDS project-specific DO/DON'T on top of this
//! seed, so the seed is written only when `guards.md` is absent (a non-force
//! scan never clobbers an enriched file) or under `--force`.

use mustard_core::domain::entity_registry::EntityRegistry;
use mustard_core::io::fs as mfs;
use mustard_core::ClaudePaths;
use serde_json::Value;
use std::fmt::Write as _;
use std::path::Path;

/// Architecture-derived boundary guards for a detected style. Generic,
/// language- and stack-agnostic phrasing keyed off the four styles the detector
/// emits (`clean` / `hexagonal` / `layered` / `ddd`). Empty for `unknown` /
/// absent — nothing is invented for an undetected architecture.
#[must_use]
pub fn architecture_boundary_rules(style: &str) -> &'static [&'static str] {
    match style {
        "clean" | "hexagonal" => &[
            "DON'T import infrastructure / adapter code from the domain (core) — dependencies point inward only.",
            "DO keep ports (interfaces) in the domain and their adapters (implementations) at the edges.",
        ],
        "layered" => &["DON'T let a lower layer import a higher one — keep layer dependencies one-directional."],
        "ddd" => &["DON'T leak persistence or transport concerns into domain entities — keep aggregates pure."],
        _ => &[],
    }
}

/// Compose the deterministic `guards.md` body for one subproject, or `None` when
/// there is no signal to seed (no detected architecture AND no frameworks AND no
/// clusters). Pure — no filesystem access, so it is directly unit-testable.
#[must_use]
fn build_body(path: &str, style: &str, frameworks: &[&str], clusters: &[&Value]) -> Option<String> {
    let arch_rules = architecture_boundary_rules(style);
    if arch_rules.is_empty() && frameworks.is_empty() && clusters.is_empty() {
        return None;
    }
    let mut body = String::from("<!-- mustard:generated -->\n");
    let _ = writeln!(body, "# Guards — `{path}`\n");
    body.push_str(
        "> Deterministic seed. `/scan --enrich` appends project-specific DO/DON'T inferred from real files.\n\n",
    );
    if !arch_rules.is_empty() {
        let _ = writeln!(body, "## Architecture: {style}");
        for rule in arch_rules {
            let _ = writeln!(body, "- {rule}");
        }
        body.push('\n');
    }
    if !frameworks.is_empty() {
        body.push_str("## Frameworks detected\n");
        for framework in frameworks {
            let _ = writeln!(body, "- DO follow the {framework} conventions.");
        }
        body.push('\n');
    }
    if !clusters.is_empty() {
        body.push_str("## Follow the discovered conventions\n");
        for c in clusters {
            let Some(label) = c.get("label").and_then(Value::as_str).filter(|s| !s.is_empty()) else {
                continue;
            };
            let count = c.get("fileCount").and_then(Value::as_u64).unwrap_or(0);
            let _ = writeln!(
                body,
                "- DO match the `{label}` convention ({count} files) — a generated skill under `.claude/skills/` documents it."
            );
        }
        body.push('\n');
    }
    Some(body)
}

/// Write the `guards.md` seed for one detected subproject. Returns the posix
/// relative path written (for the orchestrator's `generated[]`), or `None` when
/// nothing was written (already present without `--force`, no signal to seed, or
/// a filesystem error). Fail-open — never panics.
#[must_use]
pub fn render_guards_seed(
    root: &Path,
    detect_sub: &Value,
    registry: &EntityRegistry,
    force: bool,
) -> Option<String> {
    let name = detect_sub.get("name").and_then(Value::as_str).unwrap_or("");
    let path = detect_sub.get("path").and_then(Value::as_str).unwrap_or(name);
    let abs_sub = root.join(path);
    let guards_path = ClaudePaths::for_project(&abs_sub).ok()?.commands_dir().join("guards.md");

    // Don't clobber an existing (possibly `--enrich`-extended) guards.md unless
    // the user asked for a full re-scan.
    if guards_path.exists() && !force {
        return None;
    }

    // Architecture style + framework labels the registry recorded for this
    // subproject's stack. Both reuse the canonical core accessors so there is a
    // single source of truth (no re-detection here).
    let stack = crate::commands::scan::detect_stack(&abs_sub);
    let style = stack.and_then(|s| registry.architecture(s)).unwrap_or("");
    let frameworks: Vec<&str> = stack.map(|s| registry.frameworks(s)).unwrap_or_default();
    // Clusters scoped to this subproject — the canonical accessor on the core
    // registry (single source of truth for cluster scoping).
    let clusters = registry.clusters_for_subproject(path);

    let body = build_body(path, style, &frameworks, &clusters)?;
    if mfs::create_dir_all(guards_path.parent()?).is_err() {
        return None;
    }
    // Write through the enrich helper so guards.md carries a (preserved) purpose
    // block — the AI fills the deterministic seed's rationale in place.
    if !crate::commands::scan::enrich_block::write_enrichable(&guards_path, &body) {
        return None;
    }
    Some(format!("{path}/.claude/commands/guards.md"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn boundary_rules_cover_known_styles_and_skip_unknown() {
        assert!(!architecture_boundary_rules("clean").is_empty());
        assert!(!architecture_boundary_rules("hexagonal").is_empty());
        assert!(!architecture_boundary_rules("layered").is_empty());
        assert!(!architecture_boundary_rules("ddd").is_empty());
        assert!(architecture_boundary_rules("unknown").is_empty());
        assert!(architecture_boundary_rules("").is_empty());
    }

    #[test]
    fn body_none_without_any_signal() {
        assert!(build_body("apps/api", "", &[], &[]).is_none());
    }

    #[test]
    fn body_renders_architecture_and_conventions() {
        let cluster = json!({ "label": "Service", "fileCount": 5 });
        let clusters = vec![&cluster];
        let body = build_body("apps/api", "layered", &[], &clusters).expect("body");
        assert!(body.starts_with("<!-- mustard:generated -->"));
        assert!(body.contains("## Architecture: layered"));
        assert!(body.contains("one-directional"));
        assert!(body.contains("## Follow the discovered conventions"));
        assert!(body.contains("`Service` convention (5 files)"));
    }

    #[test]
    fn body_renders_conventions_even_without_architecture() {
        let cluster = json!({ "label": "Handler", "fileCount": 3 });
        let body = build_body("apps/api", "", &[], &[&cluster]).expect("body");
        assert!(!body.contains("## Architecture"));
        assert!(body.contains("`Handler` convention (3 files)"));
    }

    #[test]
    fn body_renders_frameworks_section() {
        // Labels come from the caller (registry → vocabulary), never literals
        // here — the section just renders whatever distinct labels arrived.
        let body = build_body("apps/api", "", &["di", "orm"], &[]).expect("body");
        assert!(body.contains("## Frameworks detected"));
        assert!(body.contains("- DO follow the di conventions."));
        assert!(body.contains("- DO follow the orm conventions."));
        assert!(!body.contains("## Architecture"));
    }

    #[test]
    fn body_frameworks_alone_seed_a_file() {
        // No architecture, no clusters — a framework signal alone is enough.
        assert!(build_body("apps/api", "", &["framework"], &[]).is_some());
    }
}
