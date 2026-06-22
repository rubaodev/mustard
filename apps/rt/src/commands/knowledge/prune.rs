//! `mustard-rt run knowledge prune` — the safe, reusable garbage collector for
//! the unified knowledge store.
//!
//! ## Why
//!
//! The capture path used to persist sinks of pure noise — agent "memories" with
//! an empty body and a `"interrupted mid-task"` placeholder summary, context
//! echoes, Guards citations. The write gate
//! ([`Knowledge::is_substantive`](mustard_core::domain::model::knowledge::Knowledge::is_substantive))
//! now rejects that at capture and the read gate
//! ([`KnowledgeStore::read_all`]) hides whatever already slipped onto disk. This
//! subcommand is the third leg: it **physically removes** the legacy junk so the
//! store stops carrying dead weight on disk (and in backups, greps, diffs).
//!
//! ## The single criterion (SOLID)
//!
//! The delete decision is exactly
//! [`Knowledge::is_substantive`](mustard_core::domain::model::knowledge::Knowledge::is_substantive)
//! — the *same* rule the write gate and the read gate consult. This command
//! never re-implements "is this junk?"; it parses each file through
//! [`KnowledgeStore::read`] and deletes iff `!is_substantive()`. The per-file
//! *reason* string (`empty` / `placeholder` / `echo`) is cosmetic reporting
//! only and never overrides the decision.
//!
//! ## Safety
//!
//! - Scans **only** the four content-addressed store directories under `<root>`:
//!   `memory/agent`, `memory/decisions`, `memory/lessons`, `knowledge`. It does
//!   **not** touch `spec/{spec}/memory` — that is the name-addressed per-spec
//!   store; the read filter already hides echoes from there without destroying
//!   anything.
//! - A file that parses as **substantive** is never deleted.
//! - A file that is not a `.md` under one of the four dirs is never deleted.
//! - Dry-run by default: it only **lists** candidates (path + reason). `--apply`
//!   is required to mutate the filesystem.
//! - Fail-open, no panic, no `unwrap`/`expect` outside tests. A parse/IO error
//!   on one file degrades that file out of the candidate set (kept, never
//!   deleted) — absence of signal is "keep".
//!
//! ## Output (byte-stable)
//!
//! Candidates are sorted by their `<root>`-relative path. Dry-run prints one
//! `would remove: <rel> (<reason>)` line per candidate then a summary; `--apply`
//! prints `removed: <rel> (<reason>)` for each file actually deleted. An empty
//! sweep prints `(nothing to prune)`. No timestamps, no absolute paths.

use std::path::{Path, PathBuf};

use mustard_core::domain::model::knowledge::Knowledge;
use mustard_core::io::atomic_md::MarkdownStore;
use mustard_core::io::fs;
use mustard_core::io::knowledge_store::{KnowledgeStore, STORE_DIRS};
use mustard_core::time::now_iso8601;

use super::{effective_confidence, DECAY_WINDOW_DAYS};
// The four content-addressed store sub-directories `prune` is allowed to sweep
// come from the store itself (`STORE_DIRS`) — the one source of truth, shared
// with `KnowledgeStore::read_all`. `spec/{spec}/memory` is deliberately **not**
// in that set — it is the name-addressed per-spec store and is never deleted
// here.

/// Effective-confidence floor below which a record is considered *decayed to
/// nothing*. A record this faded carries no remaining signal — it is a prune
/// candidate ONLY when it is also unused (see [`is_decayed_and_unused`]). Kept
/// just above the float-equality boundary so the full-window-old case (factor
/// exactly 0) is caught.
const DECAY_PRUNE_FLOOR: f64 = 0.001;

/// Parsed inputs for one prune invocation.
pub struct PruneOpts {
    /// The `.claude/` root whose store to sweep (taken verbatim, like
    /// `knowledge recall --root`).
    pub root: PathBuf,
    /// When `true`, candidates are deleted; when `false` (the default), they are
    /// only listed.
    pub apply: bool,
}

/// One file flagged for removal.
struct Candidate {
    /// Absolute path to the `.md` file.
    path: PathBuf,
    /// `<root>`-relative path for byte-stable display.
    rel: String,
    /// Cosmetic reason category — `empty` | `placeholder` | `echo` | `decayed`.
    reason: &'static str,
}

/// Dispatch entry: scan, then list (dry-run) or delete (`--apply`). Never
/// panics; always returns to the caller normally. Reads the wall clock ONCE
/// here (for the decay arm) and threads it into the pure [`scan`].
pub fn run(opts: PruneOpts) {
    let candidates = scan(&opts.root, &now_iso8601());
    print!("{}", render(&candidates, opts.apply));
}

/// Collect every prunable `.md` under the four store dirs of `root`, sorted by
/// `<root>`-relative path (byte-stable). A file that cannot be parsed is left out
/// of the set entirely (fail-open — never a delete candidate).
///
/// Two independent removal criteria, applied per file:
/// 1. **Not substantive** — empty body / placeholder label / context echo (the
///    single [`Knowledge::is_substantive`] gate, shared with read+write).
/// 2. **Decayed and unused** — its `effective_confidence` (the SHARED decay
///    curve) has faded to ~0 relative to `now` AND it was never surfaced
///    recently (`last_used` absent or older than the decay window). A record
///    surfaced recently is ALWAYS kept, however low its confidence — the feedback
///    loop protects live knowledge. Deterministic given `now`.
fn scan(root: &Path, now_iso: &str) -> Vec<Candidate> {
    let store = KnowledgeStore::new(root);
    let mut out: Vec<Candidate> = Vec::new();
    for segs in STORE_DIRS {
        let mut dir = root.to_path_buf();
        for seg in segs {
            dir.push(seg);
        }
        for path in md_files(&dir) {
            // Parse through the store. A parse error degrades to "keep" (skip).
            let Ok(k) = store.read(&path) else { continue };
            let reason = if !k.is_substantive() {
                reason_for(&k)
            } else if is_decayed_and_unused(&path, &k, now_iso) {
                "decayed"
            } else {
                continue;
            };
            let rel = rel_display(root, &path);
            out.push(Candidate { reason, rel, path });
        }
    }
    // Deterministic order independent of filesystem enumeration.
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

/// Whether a *substantive* record at `path` has decayed to nothing AND is unused
/// — the second, conservative prune arm. True only when BOTH hold:
///
/// - the record's [`effective_confidence`] (the shared curve, decayed on the
///   most recent of its `last_used`/`captured_at`) is at/below
///   [`DECAY_PRUNE_FLOOR`] (faded to ~0), and
/// - it was not surfaced recently — its `last_used` (read from the raw
///   frontmatter, the legacy alias the model does not carry) is absent or older
///   than the decay window.
///
/// Fail-open: an unreadable file or unparseable timestamps degrade to `false`
/// (keep). A record with no reference timestamp at all never decays (its
/// effective confidence is the un-decayed value), so it is kept too. The recency
/// guard means a recently-touched record is never a candidate, regardless of its
/// stored confidence.
fn is_decayed_and_unused(path: &Path, k: &Knowledge, now_iso: &str) -> bool {
    // `last_used` lives only in the legacy frontmatter alias (the `Knowledge`
    // model carries `captured_at`); read it raw. Reference timestamp for decay is
    // the most-recent signal we have: `last_used` when present, else `captured_at`.
    let last_used = read_last_used(path);
    let reference = last_used.as_deref().unwrap_or(k.origin.captured_at.as_str());
    let eff = effective_confidence(f64::from(k.confidence), Some(reference), now_iso);
    if eff > DECAY_PRUNE_FLOOR {
        return false;
    }
    // Conservative guard: a record surfaced within the decay window is alive —
    // keep it however low its stored confidence.
    if let Some(used) = last_used.as_deref() {
        if !is_older_than_window(used, now_iso) {
            return false;
        }
    }
    true
}

/// Read the `last_used` frontmatter value of the file at `path`, if present.
/// Fail-open: an unreadable/frontmatter-less file yields `None`.
fn read_last_used(path: &Path) -> Option<String> {
    MarkdownStore::read_one(path)
        .ok()?
        .frontmatter?
        .get_str("last_used")
        .map(str::to_string)
}

/// Whether `ts` is older than [`DECAY_WINDOW_DAYS`] before `now_iso`. Unparseable
/// timestamps degrade to `false` (treated as "not provably old" → keep).
fn is_older_than_window(ts: &str, now_iso: &str) -> bool {
    let (Some(ts_ms), Some(now_ms)) = (
        mustard_core::time::parse_iso_millis(ts),
        mustard_core::time::parse_iso_millis(now_iso),
    ) else {
        return false;
    };
    let age_days = ((now_ms - ts_ms) as f64) / 1000.0 / 86_400.0;
    age_days > DECAY_WINDOW_DAYS
}

/// The immediate `*.md` entries of `dir` (non-recursive — the store dirs are
/// flat). A missing/unreadable directory yields an empty vector (fail-open).
fn md_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .into_iter()
        .filter(|e| !e.is_dir)
        .map(|e| e.path)
        .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("md"))
        .collect()
}

/// A cosmetic reason category for a non-substantive record, mirroring the three
/// `is_substantive` rejection arms. Reporting only — the delete decision is
/// already `!is_substantive()`; this never changes it.
fn reason_for(k: &Knowledge) -> &'static str {
    if k.content.trim().is_empty() {
        "empty"
    } else if is_placeholder_label(&k.label) {
        "placeholder"
    } else {
        // The remaining `is_substantive` arm is the context-echo body.
        "echo"
    }
}

/// Mirror of the model's placeholder-label set (kept private there). Used only
/// to *label* a candidate; the keep/delete decision stays `is_substantive`.
fn is_placeholder_label(label: &str) -> bool {
    matches!(
        label.trim().to_ascii_lowercase().as_str(),
        "" | "interrupted"
            | "interrupted mid-task"
            | "interrupted mid task"
            | "(no summary)"
    )
}

/// `<root>`-relative, forward-slash path for byte-stable display across
/// platforms. Falls back to the file name when the path is not under `root`.
fn rel_display(root: &Path, path: &Path) -> String {
    let rel = path.strip_prefix(root).unwrap_or(path);
    rel.components()
        .filter_map(|c| c.as_os_str().to_str())
        .collect::<Vec<_>>()
        .join("/")
}

/// Render the sweep result into the byte-stable report. Under `--apply` it also
/// performs the deletes inline, so the report reflects what actually happened
/// (a per-file `FAILED` line on an IO error, fail-open — the sweep continues).
fn render(candidates: &[Candidate], apply: bool) -> String {
    if candidates.is_empty() {
        return "(nothing to prune)\n".to_string();
    }
    // `write!` to a `String` is infallible; discard the `Result` (no unwrap).
    use std::fmt::Write as _;
    let mut out = String::new();
    let mut removed = 0_usize;
    let mut failed = 0_usize;
    for c in candidates {
        if apply {
            match fs::remove_file(&c.path) {
                Ok(()) => {
                    removed += 1;
                    let _ = writeln!(out, "removed: {} ({})", c.rel, c.reason);
                }
                Err(e) => {
                    failed += 1;
                    let _ = writeln!(out, "FAILED: {} ({}) — {e}", c.rel, c.reason);
                }
            }
        } else {
            let _ = writeln!(out, "would remove: {} ({})", c.rel, c.reason);
        }
    }
    if apply {
        let _ = writeln!(out, "pruned {removed} file(s); {failed} failed; scanned 4 store dirs");
    } else {
        let _ = writeln!(
            out,
            "{} candidate(s) — dry-run, nothing deleted (pass --apply to remove)",
            candidates.len()
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustard_core::domain::model::knowledge::{Kind, Origin, Scope, Status};
    use tempfile::tempdir;

    /// Write a record through the store, bypassing the write-gate by planting
    /// raw bytes — so we can seed legacy junk that the gate would now reject.
    fn plant(root: &Path, dir_segs: &[&str], name: &str, body: &str) {
        let mut dir = root.to_path_buf();
        for s in dir_segs {
            dir.push(s);
        }
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(name), body.as_bytes()).unwrap();
    }

    /// A pinned clock close to the fixtures' `captured_at` so the existing
    /// non-substantive sweeps are unaffected by the decay arm (a 12-hour-old
    /// 0.6-confidence record is nowhere near the floor).
    const NOW: &str = "2026-06-15T12:00:00.000Z";

    fn substantive() -> Knowledge {
        Knowledge {
            kind: Kind::Decision,
            scope: Scope::Global,
            label: "use atomic write".into(),
            content: "chose atomic_md write because it avoids torn files".into(),
            origin: Origin { captured_at: "2026-06-15T00:00:00.000Z".into(), ..Origin::default() },
            confidence: 0.6,
            status: Status::Active,
        }
    }

    #[test]
    fn dry_run_lists_but_deletes_nothing() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // The exact sialia junk: empty body + placeholder summary, in agent dir.
        plant(
            root,
            &["memory", "agent"],
            "junk.md",
            "---\nsummary: interrupted mid-task\nat: 2026-06-15T00:00:00.000Z\n---\n",
        );
        let out = render(&scan(root, NOW), /* apply */ false);
        // Empty body is the first rejection arm, so the reason is `empty`
        // (the placeholder label is moot once the body is empty).
        assert!(out.contains("would remove: memory/agent/junk.md (empty)"), "{out}");
        assert!(out.contains("dry-run, nothing deleted"), "{out}");
        // The file is still on disk — dry-run mutates nothing.
        assert!(root.join("memory").join("agent").join("junk.md").exists());
    }

    #[test]
    fn apply_deletes_only_non_substantive() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // A real decision (must survive).
        let store = KnowledgeStore::new(root);
        let good = store.write(&substantive()).unwrap().unwrap();
        // Three flavours of junk across three store dirs.
        plant(root, &["memory", "agent"], "empty.md", "---\nsummary: delivered the thing\n---\n");
        plant(
            root,
            &["memory", "lessons"],
            "placeholder.md",
            "---\nkind: lesson\nlabel: interrupted mid-task\n---\nsome body here\n",
        );
        plant(
            root,
            &["knowledge"],
            "echo.md",
            "---\nname: ctx\n---\nCONTEXT: the orchestrator routes intent\n",
        );

        let out = render(&scan(root, NOW), /* apply */ true);
        assert!(out.contains("removed: knowledge/echo.md (echo)"), "{out}");
        assert!(out.contains("removed: memory/agent/empty.md (empty)"), "{out}");
        assert!(out.contains("removed: memory/lessons/placeholder.md (placeholder)"), "{out}");
        assert!(out.contains("pruned 3 file(s); 0 failed"), "{out}");
        // The three junk files are gone; the real decision survives.
        assert!(!root.join("memory").join("agent").join("empty.md").exists());
        assert!(!root.join("memory").join("lessons").join("placeholder.md").exists());
        assert!(!root.join("knowledge").join("echo.md").exists());
        assert!(good.exists(), "a substantive record must never be deleted");
    }

    #[test]
    fn spec_memory_is_never_swept() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // Plant junk under the name-addressed per-spec store — out of scope.
        plant(
            root,
            &["spec", "demo", "memory"],
            "junk.md",
            "---\nname: interrupted\n---\n",
        );
        let out = render(&scan(root, NOW), /* apply */ true);
        assert_eq!(out, "(nothing to prune)\n", "spec/*/memory is off-limits: {out}");
        assert!(
            root.join("spec").join("demo").join("memory").join("junk.md").exists(),
            "the per-spec store must be left untouched"
        );
    }

    #[test]
    fn empty_store_prints_nothing_to_prune() {
        let dir = tempdir().unwrap();
        assert_eq!(render(&scan(dir.path(), NOW), false), "(nothing to prune)\n");
        assert_eq!(render(&scan(dir.path(), NOW), true), "(nothing to prune)\n");
    }

    #[test]
    fn missing_root_is_fail_open() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("does-not-exist");
        // No panic, empty sweep.
        assert_eq!(render(&scan(&missing, NOW), true), "(nothing to prune)\n");
    }

    #[test]
    fn decayed_unused_record_is_pruned_but_recently_used_is_preserved() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // `now` is far past the decay window from the captured_at below.
        let now = "2026-09-01T00:00:00.000Z";

        // (1) Substantive but DECAYED + never surfaced: captured ~80 days before
        // `now`, no `last_used`. effective_confidence → 0, last_used absent → prune.
        plant(
            root,
            &["memory", "lessons"],
            "decayed.md",
            "---\nkind: lesson\nlabel: old takeaway\ncaptured_at: 2026-06-13T00:00:00.000Z\nconfidence: 0.6\nstatus: active\n---\nA real but stale lesson nobody used.\n",
        );
        // (2) Substantive, low confidence, but USED RECENTLY (last_used 1 day
        // before `now`) — the feedback loop must protect it from the decay arm.
        plant(
            root,
            &["memory", "lessons"],
            "alive.md",
            "---\nkind: lesson\nlabel: live takeaway\ncaptured_at: 2026-06-13T00:00:00.000Z\nconfidence: 0.2\nstatus: active\nlast_used: 2026-08-31T00:00:00.000Z\n---\nA lesson surfaced just yesterday.\n",
        );
        // (3) Substantive and FRESH (captured at `now`) — never a decay candidate.
        plant(
            root,
            &["memory", "decisions"],
            "fresh.md",
            "---\nkind: decision\nlabel: fresh call\ncaptured_at: 2026-09-01T00:00:00.000Z\nconfidence: 0.6\nstatus: active\n---\nA decision made just now.\n",
        );

        let candidates = scan(root, now);
        let out = render(&candidates, /* apply */ true);
        // Only the decayed+unused record is removed.
        assert!(out.contains("removed: memory/lessons/decayed.md (decayed)"), "{out}");
        assert!(out.contains("pruned 1 file(s); 0 failed"), "{out}");
        assert!(!root.join("memory").join("lessons").join("decayed.md").exists());
        // The recently-used and the fresh records survive — conservative.
        assert!(root.join("memory").join("lessons").join("alive.md").exists(),
            "a recently-surfaced record must survive however low its confidence");
        assert!(root.join("memory").join("decisions").join("fresh.md").exists(),
            "a fresh record is never a decay candidate");
    }

    #[test]
    fn decay_arm_dry_run_deletes_nothing() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        let now = "2026-09-01T00:00:00.000Z";
        plant(
            root,
            &["memory", "lessons"],
            "decayed.md",
            "---\nkind: lesson\nlabel: old takeaway\ncaptured_at: 2026-06-13T00:00:00.000Z\nconfidence: 0.6\nstatus: active\n---\nA real but stale lesson nobody used.\n",
        );
        let out = render(&scan(root, now), /* apply */ false);
        assert!(out.contains("would remove: memory/lessons/decayed.md (decayed)"), "{out}");
        assert!(out.contains("dry-run, nothing deleted"), "{out}");
        // Untouched on disk.
        assert!(root.join("memory").join("lessons").join("decayed.md").exists());
    }
}
