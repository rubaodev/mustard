//! `run knowledge recall` — the CLI face of [`super::recall`].
//!
//! Exposes the unified knowledge store's BM25 relevance recall as a measurable,
//! byte-stable command so the recall can be tested against the real stores and
//! the orchestrator can pull cross-spec knowledge on demand. The render already
//! calls [`super::recall::recall`] in-process; this is the human/measurement
//! entry point over the same function (the scored variant, so each hit shows the
//! integer BM25 score the threshold cut on).
//!
//! ## Output (deterministic)
//!
//! One pair of lines per recalled record, best-first:
//! ```text
//! [{kind}] ({scope}) score={n} — {label}
//!   {first ~120 chars of content, single-line}
//! ```
//! No timestamps, no absolute paths, no volatile fields — the same query over
//! the same store prints identical bytes. An empty recall prints a single
//! `(no matches)` line. Never panics; fail-open mirrors [`super::recall`].

use std::path::{Path, PathBuf};

use mustard_core::domain::model::knowledge::{Knowledge, Scope};
use mustard_core::io::claude_paths::ClaudePaths;

use super::recall::recall_scored;

/// Parsed inputs for one recall invocation.
pub struct RecallOpts {
    /// The relevance query (role + task text, free-form).
    pub query: String,
    /// Optional scope filter, pre-parsed from the `--scope` string.
    pub scope: Option<Scope>,
    /// Optional `.claude/` root override. `None` ⇒ resolve from the cwd.
    pub root: Option<PathBuf>,
    /// Result cap.
    pub max: usize,
}

/// Max snippet length (chars) of the `content` echoed under each hit. Keeps the
/// line scannable without dumping a whole record body.
const SNIPPET_CHARS: usize = 120;

/// Dispatch entry: resolve the `.claude/` root, recall, print. Never panics;
/// always exits the caller normally (the `run` face wraps this).
pub fn run(opts: RecallOpts) {
    let claude_root = resolve_claude_root(opts.root.as_deref());
    let hits = recall_scored(&claude_root, &opts.query, opts.scope.as_ref(), opts.max);
    print!("{}", render(&hits));
}

/// Resolve the knowledge store's `.claude/` directory.
///
/// - `--root <path>` is taken verbatim as the `.claude/` dir (the contract: the
///   flag already points at `.claude/`, e.g. `C:/Atiz/mustard/.claude`).
/// - Without it, anchor on the workspace root via [`ClaudePaths`] and take its
///   `.claude/`; fail-open to `./.claude` when no workspace resolves.
fn resolve_claude_root(root: Option<&Path>) -> PathBuf {
    if let Some(explicit) = root {
        return explicit.to_path_buf();
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let anchor = mustard_core::io::workspace::workspace_root(&cwd).unwrap_or(cwd);
    ClaudePaths::for_project(&anchor)
        .map(|p| p.claude_dir())
        .unwrap_or_else(|_| anchor.join(".claude"))
}

/// Render the scored hits into the byte-stable two-line-per-hit block.
fn render(hits: &[(u64, Knowledge)]) -> String {
    if hits.is_empty() {
        return "(no matches)\n".to_string();
    }
    let mut out = String::new();
    for (score, k) in hits {
        out.push_str(&format!(
            "[{}] ({}) score={} — {}\n",
            k.kind.as_str(),
            scope_label(&k.scope),
            score,
            k.label
        ));
        out.push_str("  ");
        out.push_str(&snippet(&k.content));
        out.push('\n');
    }
    out
}

/// Stable, single-line label for a [`Scope`] — matches the `--scope` grammar
/// (`global` / `spec:NAME` / `wave:NAME:N`).
fn scope_label(scope: &Scope) -> String {
    match scope {
        Scope::Global => "global".to_string(),
        Scope::Spec { spec } => format!("spec:{spec}"),
        Scope::Wave { spec, wave } => format!("wave:{spec}:{wave}"),
    }
}

/// First [`SNIPPET_CHARS`] chars of `content`, newlines/tabs collapsed to single
/// spaces so the snippet stays on one line. Truncation is on char boundaries.
fn snippet(content: &str) -> String {
    let flattened: String = content
        .chars()
        .map(|c| if c.is_whitespace() { ' ' } else { c })
        .collect();
    let trimmed = flattened.split_whitespace().collect::<Vec<_>>().join(" ");
    let truncated: String = trimmed.chars().take(SNIPPET_CHARS).collect();
    if trimmed.chars().count() > SNIPPET_CHARS {
        format!("{truncated}…")
    } else {
        truncated
    }
}

/// Parse the `--scope` string into a [`Scope`].
///
/// Grammar: `global` | `spec:<NAME>` | `wave:<NAME>:<N>`. Returns `Err` with a
/// short reason on a malformed value so the dispatcher can surface a usage
/// error instead of silently widening to all-scopes.
pub fn parse_scope(raw: &str) -> Result<Scope, String> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("global") {
        return Ok(Scope::Global);
    }
    if let Some(rest) = raw.strip_prefix("spec:") {
        let name = rest.trim();
        if name.is_empty() {
            return Err("scope `spec:` needs a spec name (spec:NAME)".to_string());
        }
        return Ok(Scope::Spec { spec: name.to_string() });
    }
    if let Some(rest) = raw.strip_prefix("wave:") {
        // Split on the LAST colon so spec slugs may themselves contain colons.
        let (name, num) = rest
            .rsplit_once(':')
            .ok_or_else(|| "scope `wave:` needs spec and wave (wave:NAME:N)".to_string())?;
        let name = name.trim();
        if name.is_empty() {
            return Err("scope `wave:` needs a spec name (wave:NAME:N)".to_string());
        }
        let wave: u32 = num
            .trim()
            .parse()
            .map_err(|_| format!("wave number `{num}` is not a non-negative integer"))?;
        return Ok(Scope::Wave { spec: name.to_string(), wave });
    }
    Err(format!(
        "unknown scope `{raw}` — expected global | spec:NAME | wave:NAME:N"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustard_core::domain::model::knowledge::{Kind, Origin, Status};

    fn k(kind: Kind, scope: Scope, label: &str, content: &str) -> Knowledge {
        Knowledge {
            kind,
            scope,
            label: label.into(),
            content: content.into(),
            origin: Origin { captured_at: "2026-06-15T00:00:00.000Z".into(), ..Origin::default() },
            confidence: 0.6,
            status: Status::Active,
        }
    }

    #[test]
    fn empty_hits_render_no_matches() {
        assert_eq!(render(&[]), "(no matches)\n");
    }

    #[test]
    fn render_is_two_lines_per_hit_and_byte_stable() {
        let hits = vec![
            (4242, k(Kind::Decision, Scope::Global, "caching policy", "keys on path length mtime")),
            (
                1000,
                k(
                    Kind::Lesson,
                    Scope::Wave { spec: "demo".into(), wave: 2 },
                    "wave note",
                    "delivered the subcommand",
                ),
            ),
        ];
        let out = render(&hits);
        assert_eq!(
            out,
            "[decision] (global) score=4242 — caching policy\n  keys on path length mtime\n\
             [lesson] (wave:demo:2) score=1000 — wave note\n  delivered the subcommand\n"
        );
        // Determinism: identical input → identical bytes.
        assert_eq!(render(&hits), out);
    }

    #[test]
    fn snippet_flattens_newlines_and_truncates() {
        let body = "line one\nline two\twith tab";
        assert_eq!(snippet(body), "line one line two with tab");
        let long = "x ".repeat(200);
        let s = snippet(&long);
        assert!(s.ends_with('…'));
        // 120 chars + the ellipsis.
        assert_eq!(s.chars().count(), SNIPPET_CHARS + 1);
    }

    #[test]
    fn parse_scope_accepts_the_three_forms() {
        assert_eq!(parse_scope("global").unwrap(), Scope::Global);
        assert_eq!(parse_scope("GLOBAL").unwrap(), Scope::Global);
        assert_eq!(
            parse_scope("spec:alinhar-aba").unwrap(),
            Scope::Spec { spec: "alinhar-aba".into() }
        );
        assert_eq!(
            parse_scope("wave:alinhar-aba:3").unwrap(),
            Scope::Wave { spec: "alinhar-aba".into(), wave: 3 }
        );
    }

    #[test]
    fn parse_scope_rejects_garbage() {
        assert!(parse_scope("nonsense").is_err());
        assert!(parse_scope("spec:").is_err());
        assert!(parse_scope("wave:x").is_err());
        assert!(parse_scope("wave:x:notanumber").is_err());
    }

    #[test]
    fn scope_label_round_trips_with_parse_scope() {
        for s in [
            Scope::Global,
            Scope::Spec { spec: "demo".into() },
            Scope::Wave { spec: "demo".into(), wave: 1 },
        ] {
            assert_eq!(parse_scope(&scope_label(&s)).unwrap(), s);
        }
    }
}
