//! Deterministic CLAUDE.md generator for subprojects — no AI, no source reads.
//!
//! Invoked by `scan::run` after `grain.model.json` is written:
//! - `--full`: (re)generates `{root}/{dir}/CLAUDE.md` per subproject, preserving
//!   any existing `## Guards` section verbatim.
//! - default: reports files exceeding [`CLAUDE_MD_WARN_BYTES`] as oversized.

use std::path::{Path, PathBuf};

/// Files larger than this threshold trigger a warning in default (non-full) mode.
pub const CLAUDE_MD_WARN_BYTES: usize = 2048;

/// Result of running the CLAUDE.md pass over a set of projects.
pub struct ClaudeMdResult {
    /// Paths regenerated (full mode).
    pub regenerated: Vec<String>,
    /// Oversized files (default mode): path + byte count.
    pub oversized: Vec<OversizedEntry>,
}

pub struct OversizedEntry {
    pub path: String,
    pub bytes: usize,
}

/// Title-case the first character of `s`, leaving the rest unchanged.
fn title_case(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => {
            let mut out = first.to_uppercase().to_string();
            out.push_str(chars.as_str());
            out
        }
    }
}

/// Extract the `## Guards` section from `content` (from the `## Guards` heading
/// up to the next `## ` heading or EOF). Returns `None` when no section exists.
pub fn extract_guards(content: &str) -> Option<String> {
    let mut in_guards = false;
    let mut lines: Vec<&str> = Vec::new();

    for line in content.lines() {
        if line.trim_start().starts_with("## Guards") && !in_guards {
            in_guards = true;
            // Skip the heading line itself — we re-emit it in render()
            continue;
        }
        if in_guards {
            if line.starts_with("## ") {
                break;
            }
            lines.push(line);
        }
    }

    if in_guards {
        // Trim leading/trailing blank lines from the captured block
        let trimmed = lines
            .iter()
            .copied()
            .skip_while(|l| l.trim().is_empty())
            .collect::<Vec<_>>();
        // Trim trailing blanks
        let end = trimmed
            .iter()
            .rposition(|l| !l.trim().is_empty())
            .map(|i| i + 1)
            .unwrap_or(0);
        let body = trimmed[..end].join("\n");
        Some(body)
    } else {
        None
    }
}

/// Render a lean CLAUDE.md for a subproject.
///
/// - `name`: subproject name (will be title-cased for the H1 heading)
/// - `kind`: grain project kind (e.g. `rust`, `typescript`, …)
/// - `code_files`: number of code files grain counted
/// - `existing`: current content of the CLAUDE.md (if the file exists)
///
/// Guards from `existing` are preserved verbatim; everything else is regenerated
/// deterministically.
pub fn render(name: &str, kind: &str, code_files: usize, existing: Option<&str>) -> String {
    let title = title_case(name);
    let guards_body = existing
        .and_then(extract_guards)
        .unwrap_or_default();
    let guards_content = if guards_body.is_empty() {
        "<!-- seed DO/DON'T aqui -->".to_string()
    } else {
        guards_body
    };

    format!(
        "# {title}\n\
         \n\
         > Parent: [../CLAUDE.md](../CLAUDE.md) | Orchestrator: [../.claude/CLAUDE.md](../.claude/CLAUDE.md)\n\
         \n\
         <!-- mustard:scan-map -->\n\
         Tipo: {kind} · {code_files} arquivos\n\
         Pesquise via `mustard-rt run feature` (digest) — não leia o repo direto.\n\
         <!-- /mustard:scan-map -->\n\
         \n\
         ## Guards\n\
         \n\
         {guards_content}\n"
    )
}

/// Run the CLAUDE.md pass (full or default) over all subprojects.
///
/// Returns a [`ClaudeMdResult`] whose fields populate the JSON response in
/// `scan::run`. In `full` mode also creates `{root}/{dir}/.claude/` if absent.
pub fn run_pass(
    root: &Path,
    projects: &[mustard_core::domain::scan::Project],
    full: bool,
) -> ClaudeMdResult {
    if full {
        run_full(root, projects)
    } else {
        run_default(root, projects)
    }
}

fn run_full(root: &Path, projects: &[mustard_core::domain::scan::Project]) -> ClaudeMdResult {
    let mut regenerated: Vec<String> = Vec::new();

    for project in projects {
        let dir = root.join(&project.dir);
        let claude_md_path = dir.join("CLAUDE.md");
        let claude_dir = dir.join(".claude");

        // Read existing content to preserve guards — fail-open
        let existing = std::fs::read_to_string(&claude_md_path).ok();
        let content = render(
            &project.name,
            &project.kind,
            project.code_files,
            existing.as_deref(),
        );

        // Ensure .claude/ subdir exists
        if let Err(e) = std::fs::create_dir_all(&claude_dir) {
            eprintln!(
                "scan --full: could not create {:?}: {e}",
                claude_dir.display()
            );
        }

        // Write CLAUDE.md (use mustard_core atomic write for safety)
        let write_result =
            mustard_core::io::fs::write_atomic(&claude_md_path, content.as_bytes());
        match write_result {
            Ok(()) => {
                regenerated.push(path_to_string(&claude_md_path));
            }
            Err(e) => {
                eprintln!(
                    "scan --full: could not write {:?}: {e}",
                    claude_md_path.display()
                );
            }
        }
    }

    ClaudeMdResult {
        regenerated,
        oversized: Vec::new(),
    }
}

fn run_default(root: &Path, projects: &[mustard_core::domain::scan::Project]) -> ClaudeMdResult {
    let mut oversized: Vec<OversizedEntry> = Vec::new();

    for project in projects {
        let claude_md_path = root.join(&project.dir).join("CLAUDE.md");
        if let Ok(meta) = std::fs::metadata(&claude_md_path) {
            let bytes = meta.len() as usize;
            if bytes > CLAUDE_MD_WARN_BYTES {
                oversized.push(OversizedEntry {
                    path: path_to_string(&claude_md_path),
                    bytes,
                });
            }
        }
    }

    ClaudeMdResult {
        regenerated: Vec::new(),
        oversized,
    }
}

fn path_to_string(path: &PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_without_existing_creates_scaffold() {
        let out = render("dashboard", "typescript", 42, None);
        assert!(out.contains("# Dashboard"), "header missing: {out}");
        assert!(out.contains("Tipo: typescript · 42 arquivos"), "map missing: {out}");
        assert!(out.contains("<!-- mustard:scan-map -->"), "scan-map open missing: {out}");
        assert!(out.contains("<!-- /mustard:scan-map -->"), "scan-map close missing: {out}");
        assert!(out.contains("## Guards"), "guards heading missing: {out}");
        assert!(out.contains("<!-- seed DO/DON'T aqui -->"), "seed placeholder missing: {out}");
        assert!(out.ends_with('\n'), "missing trailing newline");
    }

    #[test]
    fn render_with_existing_guards_preserves_them() {
        let existing = "\
# Dashboard

<!-- mustard:scan-map -->
Tipo: typescript · 10 arquivos
<!-- /mustard:scan-map -->

## Guards

- Never import from `../apps/cli`
- Always use `Result<T, anyhow::Error>`

## Other Section

Some text
";
        let out = render("dashboard", "rust", 99, Some(existing));
        // Guards preserved
        assert!(out.contains("Never import from"), "guard line 1 missing: {out}");
        assert!(out.contains("Always use `Result<T, anyhow::Error>`"), "guard line 2 missing: {out}");
        // Map block regenerated
        assert!(out.contains("Tipo: rust · 99 arquivos"), "map not refreshed: {out}");
        // Seed placeholder absent (guards were found)
        assert!(!out.contains("seed DO/DON'T"), "seed placeholder must not appear when guards exist: {out}");
    }

    #[test]
    fn default_mode_collects_oversized_and_ignores_small() {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();

        // Subproject 1: large file
        let sub1 = root.join("apps").join("big");
        std::fs::create_dir_all(&sub1).expect("mkdir big");
        let big_path = sub1.join("CLAUDE.md");
        let big_content = "x".repeat(CLAUDE_MD_WARN_BYTES + 1);
        std::fs::File::create(&big_path)
            .and_then(|mut f| f.write_all(big_content.as_bytes()))
            .expect("write big");

        // Subproject 2: small file
        let sub2 = root.join("apps").join("small");
        std::fs::create_dir_all(&sub2).expect("mkdir small");
        let small_path = sub2.join("CLAUDE.md");
        std::fs::File::create(&small_path)
            .and_then(|mut f| f.write_all(b"tiny"))
            .expect("write small");

        let projects = vec![
            mustard_core::domain::scan::Project {
                name: "big".into(),
                dir: "apps/big".into(),
                kind: "rust".into(),
                code_files: 1,
            },
            mustard_core::domain::scan::Project {
                name: "small".into(),
                dir: "apps/small".into(),
                kind: "rust".into(),
                code_files: 1,
            },
        ];

        let result = run_default(root, &projects);
        assert_eq!(result.oversized.len(), 1, "only the big file should be flagged");
        assert!(result.oversized[0].path.contains("big"), "wrong file flagged");
        assert!(result.oversized[0].bytes > CLAUDE_MD_WARN_BYTES);
        assert!(result.regenerated.is_empty());
    }
}
