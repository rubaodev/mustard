//! `mustard-rt run hardcode-gate` — advisory gate against stack-knowledge
//! hardcoding in the agnostic surfaces.
//!
//! The scan/core pair must stay stack-agnostic: every framework-specific
//! literal belongs in the embedded stack registry
//! ([`StackRegistry::builtin`]), never inline in detection code. This gate
//! reads the registry's literals (`manifest_deps` + `path_markers` +
//! `code_signatures` of every stack — stack **names** are deliberately
//! excluded: `next` collides with common identifiers), runs `git diff HEAD`
//! (working tree + staged) restricted to `apps/scan/src` and
//! `packages/core/src` `.rs` files, and reports every ADDED line (`+` prefix)
//! containing any literal.
//!
//! Output is one deterministic JSON object
//! `{"ok":bool,"hits":[{"file","line","literal"}]}` ordered by
//! (file, line, literal). The exit code is ALWAYS `0` — the gate is advisory
//! and the orchestrator decides; `ok` is `false` when hits exist.
//!
//! Fail-safe: a missing `git` binary or a non-repo cwd degrades to
//! `{"ok":true,"hits":[],"note":"git unavailable"}` — the `note` field keeps
//! the degradation visible instead of masking it as a verified-clean run.
//!
//! Known limitation (v1): the diff scan does not distinguish `#[cfg(test)]`
//! code — a registry literal added inside a test module still reports a hit.

use mustard_core::domain::vocabulary::stacks::StackRegistry;
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The agnosticism-sensitive directories the gate watches.
const WATCHED_DIRS: [&str; 2] = ["apps/scan/src", "packages/core/src"];

/// One added line containing a registry literal.
#[derive(Debug, Clone, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct Hit {
    /// Repo-relative path (`b/`-stripped, forward slashes — as git emits it).
    pub file: String,
    /// 1-based line number in the NEW file.
    pub line: u32,
    /// The registry literal found on the line.
    pub literal: String,
}

/// The gate's JSON report. Field order is the serialization order (struct
/// derive), so the output stays byte-stable.
#[derive(Debug, Serialize)]
pub struct GateReport {
    /// `false` when any hit exists. Stays `true` on degradation — see `note`.
    pub ok: bool,
    /// Added lines containing registry literals, ordered (file, line, literal).
    pub hits: Vec<Hit>,
    /// Present only on degradation (`"git unavailable"` / registry failure) so
    /// a degraded run is never mistaken for a verified-clean one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

/// Collect the registry literals to scan for: every stack's `manifest_deps`,
/// `path_markers`, and `code_signatures`, sorted + deduped for determinism.
/// Stack names stay OUT (e.g. `next` would flood on ordinary identifiers).
fn registry_literals() -> Vec<String> {
    let Ok(reg) = StackRegistry::builtin() else {
        return Vec::new();
    };
    let mut out: Vec<String> = Vec::new();
    for stack in reg.stacks() {
        out.extend(stack.manifest_deps.iter().cloned());
        out.extend(stack.path_markers.iter().cloned());
        out.extend(stack.code_signatures.iter().cloned());
    }
    out.sort();
    out.dedup();
    out
}

/// Run `git diff HEAD` scoped to [`WATCHED_DIRS`], returning the raw unified
/// diff (zero context lines so hunk headers map added lines exactly).
///
/// Calls `git` directly — NOT through `rtk_command` — because the gate parses
/// the raw unified-diff shape (hunk headers + `+` prefixes) and RTK's `git
/// diff` filter rewrites that shape into its compact form.
///
/// `None` when git cannot answer (binary missing, not a repo, unborn HEAD) —
/// the caller degrades with the `note` field.
fn git_diff(cwd: &Path) -> Option<String> {
    let mut args: Vec<&str> = vec!["diff", "HEAD", "--no-color", "--unified=0", "--"];
    args.extend(WATCHED_DIRS);
    std::process::Command::new("git")
        .args(&args)
        .current_dir(cwd)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|o| String::from_utf8(o.stdout).ok())
}

/// Parse the `+c[,d]` part of a `@@ -a,b +c,d @@` hunk header into the
/// starting NEW-file line number.
fn hunk_new_start(rest: &str) -> Option<u32> {
    let plus = rest.find('+')?;
    let digits: String = rest[plus + 1..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// Scan a unified diff for added lines (prefix `+`) in `.rs` files that
/// contain any of `literals`. One hit per (line, literal) pair.
fn scan_diff(diff: &str, literals: &[String]) -> Vec<Hit> {
    let mut hits: Vec<Hit> = Vec::new();
    // `None` while inside a non-`.rs` (or deleted-file) section.
    let mut current_file: Option<String> = None;
    let mut new_line: u32 = 0;
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("+++ ") {
            let path = rest.trim();
            let path = path.strip_prefix("b/").unwrap_or(path);
            current_file = (path.ends_with(".rs")).then(|| path.to_string());
            continue;
        }
        if let Some(rest) = line.strip_prefix("@@") {
            new_line = hunk_new_start(rest).unwrap_or(0);
            continue;
        }
        match line.as_bytes().first() {
            Some(b'+') => {
                if let Some(file) = &current_file {
                    let added = &line[1..];
                    for lit in literals {
                        if added.contains(lit.as_str()) {
                            hits.push(Hit {
                                file: file.clone(),
                                line: new_line,
                                literal: lit.clone(),
                            });
                        }
                    }
                }
                new_line = new_line.saturating_add(1);
            }
            // Context lines advance the new-file counter too (none are emitted
            // under `--unified=0`, but stay robust to a different git config).
            Some(b' ') => new_line = new_line.saturating_add(1),
            _ => {}
        }
    }
    hits.sort();
    hits.dedup();
    hits
}

/// Run the gate against `cwd`. Pure aside from the registry read and the git
/// subprocess; extracted so tests can drive it with a temp repo.
fn run_gate(cwd: &Path) -> GateReport {
    let literals = registry_literals();
    if literals.is_empty() {
        // The embedded registry failing to parse is a build defect, but the
        // gate stays advisory: degrade visibly rather than block or mask.
        return GateReport {
            ok: true,
            hits: Vec::new(),
            note: Some("stack registry unavailable".to_string()),
        };
    }
    let Some(diff) = git_diff(cwd) else {
        return GateReport {
            ok: true,
            hits: Vec::new(),
            note: Some("git unavailable".to_string()),
        };
    };
    let hits = scan_diff(&diff, &literals);
    GateReport {
        ok: hits.is_empty(),
        hits,
        note: None,
    }
}

/// Dispatch `mustard-rt run hardcode-gate`. Prints the JSON report and always
/// returns (exit 0) — the verdict lives in the `ok` field, never the exit code.
pub fn run() {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let report = run_gate(&cwd);
    let body = serde_json::to_string(&report)
        .unwrap_or_else(|_| "{\"ok\":true,\"hits\":[],\"note\":\"serialization failed\"}".to_string());
    println!("{body}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::tempdir;

    /// `true` when `git` is on the PATH; tests degrade to a silent pass
    /// otherwise (mirrors `diff_context`'s fail-open test contract).
    fn git_available() -> bool {
        Command::new("git").arg("--version").output().is_ok()
    }

    /// Init a repo with identity + a seed commit so `git diff HEAD` resolves.
    fn init_repo(cwd: &Path) {
        let g = |args: &[&str]| {
            let _ = Command::new("git").args(args).current_dir(cwd).output();
        };
        g(&["init", "-b", "main"]);
        g(&["config", "user.email", "t@e.x"]);
        g(&["config", "user.name", "t"]);
        g(&["config", "commit.gpgsign", "false"]);
        std::fs::write(cwd.join("seed.txt"), "seed").unwrap();
        g(&["add", "-A"]);
        g(&["commit", "-m", "seed"]);
    }

    /// A real literal from the embedded registry (first stack's first code
    /// signature) — read programmatically so registry edits don't break tests.
    fn sample_code_signature() -> String {
        let reg = StackRegistry::builtin().unwrap();
        reg.stacks()[0].code_signatures[0].clone()
    }

    #[test]
    fn hardcode_gate_clean_diff_is_ok() {
        if !git_available() {
            return;
        }
        let dir = tempdir().unwrap();
        init_repo(dir.path());
        // A staged .rs file in a watched dir with NO registry literal.
        let src = dir.path().join("packages/core/src");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("clean.rs"), "// plain module\npub fn noop() {}\n").unwrap();
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .output();

        let report = run_gate(dir.path());
        assert!(report.ok, "{report:?}");
        assert!(report.hits.is_empty(), "{report:?}");
        assert!(report.note.is_none(), "clean run must not carry a note: {report:?}");
    }

    #[test]
    fn hardcode_gate_added_literal_is_a_hit() {
        if !git_available() {
            return;
        }
        let dir = tempdir().unwrap();
        init_repo(dir.path());
        let literal = sample_code_signature();
        // Watched dir: the literal lands on line 2 of a staged new file.
        let core_src = dir.path().join("packages/core/src");
        std::fs::create_dir_all(&core_src).unwrap();
        std::fs::write(
            core_src.join("x.rs"),
            format!("// header\nlet sig = \"{literal}\";\n"),
        )
        .unwrap();
        // Same literal OUTSIDE the watched dirs — must NOT be reported.
        let other = dir.path().join("src");
        std::fs::create_dir_all(&other).unwrap();
        std::fs::write(other.join("other.rs"), format!("// {literal}\n")).unwrap();
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(dir.path())
            .output();

        let report = run_gate(dir.path());
        assert!(!report.ok, "{report:?}");
        assert_eq!(report.hits.len(), 1, "{report:?}");
        assert_eq!(report.hits[0].file, "packages/core/src/x.rs");
        assert_eq!(report.hits[0].line, 2);
        assert_eq!(report.hits[0].literal, literal);
        assert!(report.note.is_none());
    }

    #[test]
    fn hardcode_gate_non_repo_degrades_with_note() {
        // A tempdir that is NOT a git repo: `git diff HEAD` fails → visible
        // degradation, never a fake verified-clean.
        let dir = tempdir().unwrap();
        let report = run_gate(dir.path());
        assert!(report.ok);
        assert!(report.hits.is_empty());
        assert_eq!(report.note.as_deref(), Some("git unavailable"));
    }

    #[test]
    fn hardcode_gate_scan_skips_non_rs_and_removed_lines() {
        let literals = vec!["Schema::create(".to_string()];
        // A removed line + a non-.rs file carrying the literal: no hits.
        let diff = "\
diff --git a/packages/core/src/a.rs b/packages/core/src/a.rs
--- a/packages/core/src/a.rs
+++ b/packages/core/src/a.rs
@@ -3 +2,0 @@
-let gone = \"Schema::create(\";
diff --git a/packages/core/src/notes.md b/packages/core/src/notes.md
--- a/packages/core/src/notes.md
+++ b/packages/core/src/notes.md
@@ -0,0 +1 @@
+Schema::create( in prose
";
        assert!(scan_diff(diff, &literals).is_empty());
        // The same literal ADDED to a .rs file: exactly one hit at line 7.
        let diff_hit = "\
diff --git a/apps/scan/src/b.rs b/apps/scan/src/b.rs
--- a/apps/scan/src/b.rs
+++ b/apps/scan/src/b.rs
@@ -6,0 +7 @@
+    let sig = \"Schema::create(\";
";
        let hits = scan_diff(diff_hit, &literals);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].file, "apps/scan/src/b.rs");
        assert_eq!(hits[0].line, 7);
        assert_eq!(hits[0].literal, "Schema::create(");
    }

    #[test]
    fn hardcode_gate_report_serializes_deterministically() {
        let report = GateReport {
            ok: false,
            hits: vec![Hit {
                file: "packages/core/src/x.rs".to_string(),
                line: 2,
                literal: "Schema::create(".to_string(),
            }],
            note: None,
        };
        let json = serde_json::to_string(&report).unwrap();
        assert_eq!(
            json,
            "{\"ok\":false,\"hits\":[{\"file\":\"packages/core/src/x.rs\",\"line\":2,\"literal\":\"Schema::create(\"}]}"
        );
        let degraded = GateReport { ok: true, hits: Vec::new(), note: Some("git unavailable".to_string()) };
        assert_eq!(
            serde_json::to_string(&degraded).unwrap(),
            "{\"ok\":true,\"hits\":[],\"note\":\"git unavailable\"}"
        );
    }
}
