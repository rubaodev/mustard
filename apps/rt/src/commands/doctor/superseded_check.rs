//! `mustard-rt run doctor --check superseded` — prune/accumulation linter.
//!
//! Roadmap #6 ("accumulates faster than it prunes"). Specs accrete; nothing
//! deterministically surfaces the ones that are safe to archive. This check
//! walks `.claude/spec/*/` (flat layout) and flags two kinds of prune
//! candidates — by **verification**, never by vibe:
//!
//! 1. **terminal** — `meta.json#outcome` is a terminal outcome
//!    (`Completed` / `Cancelled` / `Abandoned` / `Superseded` / `Absorbed`).
//!    The spec is done; it is archivable.
//! 2. **stale-anchored active** — an `Active` spec whose `## Files` anchors
//!    point at paths that no longer exist on disk. When the code a spec claims
//!    to touch has moved/been deleted, the spec is very likely superseded —
//!    the anchors are the evidence, the staleRatio is the measure.
//!
//! This is **read-only**: doctor never deletes. It only reports slugs and the
//! stale paths so the maintainer can decide. Output is byte-stable — slugs are
//! sorted, the ratio is fixed-point (×1000, no float), and every path is
//! repo-relative (no absolute/volatile paths, no mtime).
//!
//! Fail-open: a spec whose `meta.json` or `spec.md` is unreadable is skipped
//! and recorded in `scannedErrors`; the scan continues and never panics.

use crate::commands::wave::wave_lib::parse_files_section;
use mustard_core::io::fs;
use serde::Serialize;
use std::path::Path;

/// Terminal (archivable) outcomes — mirror of the `is_valid_combo` Close set in
/// `doctor.rs`. An `Active` spec is, by definition, not terminal.
const TERMINAL_OUTCOMES: &[&str] =
    &["Completed", "Cancelled", "Abandoned", "Superseded", "Absorbed"];

/// staleRatio threshold (fixed-point ×1000) above which an Active spec is a
/// prune candidate: > 0.5 means a majority of its `## Files` anchors are gone.
const STALE_RATIO_THRESHOLD_X1000: u32 = 500;

/// One prune candidate — either terminal, or an Active spec whose anchors went
/// stale. Serialized in slug order for byte-stability.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct PruneCandidate {
    pub slug: String,
    /// The spec's `meta.json#outcome` (e.g. `Completed`, or `Active` for a
    /// stale-anchored active spec).
    pub outcome: String,
    /// Why this spec is a candidate: `terminal` or `stale-anchors`.
    pub reason: &'static str,
    /// Repo-relative `## Files` paths that no longer exist (sorted). Empty for
    /// a terminal spec with no missing anchors.
    pub stale_files: Vec<String>,
    /// Fraction of `## Files` anchors that are missing, ×1000 (integer, no
    /// float). `0` when the spec lists no files.
    #[serde(rename = "staleRatio_x1000")]
    pub stale_ratio_x1000: u32,
}

/// The full prune-candidate report. All vectors are slug-sorted.
#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct SupersededReport {
    /// `true` when there is nothing to prune (no candidates, no stale-active).
    pub ok: bool,
    pub total_specs: usize,
    pub active: usize,
    pub terminal: usize,
    /// Archivable / likely-superseded specs (terminal OR high staleRatio).
    pub prune_candidates: Vec<PruneCandidate>,
    /// Active specs with some — but sub-threshold — stale anchors. Surfaced as
    /// an early-warning list; not yet prune candidates.
    pub stale_active: Vec<PruneCandidate>,
    /// Per-spec read errors (unreadable meta/spec). Sorted; fail-open evidence.
    pub scanned_errors: Vec<String>,
}

/// Walk `.claude/spec/*/` under `root` and build the prune-candidate report.
///
/// `root` is the workspace root (the directory holding `.claude/`). Only the
/// top-level spec dirs are categorised — wave subdirs inherit their parent's
/// lifecycle and are not independent prune units.
#[must_use]
pub fn run(root: &Path) -> SupersededReport {
    let spec_root = root.join(".claude").join("spec");
    let mut candidates: Vec<PruneCandidate> = Vec::new();
    let mut stale_active: Vec<PruneCandidate> = Vec::new();
    let mut errors: Vec<String> = Vec::new();
    let mut total_specs = 0usize;
    let mut active = 0usize;
    let mut terminal = 0usize;

    let Ok(entries) = fs::read_dir(&spec_root) else {
        // No spec/ dir — clean install, nothing to prune.
        return SupersededReport {
            ok: true,
            total_specs: 0,
            active: 0,
            terminal: 0,
            prune_candidates: Vec::new(),
            stale_active: Vec::new(),
            scanned_errors: Vec::new(),
        };
    };

    // Collect + sort slugs first so iteration order (and thus error order) is
    // deterministic regardless of the OS readdir order.
    let mut slugs: Vec<String> =
        entries.into_iter().filter(|e| e.is_dir).map(|e| e.file_name).collect();
    slugs.sort();

    for slug in &slugs {
        let spec_dir = spec_root.join(slug);
        let meta_path = spec_dir.join("meta.json");

        // meta.json is the lifecycle source of truth. Unreadable / missing →
        // skip this spec, record the error, continue (fail-open).
        let Some(meta) = mustard_core::read_meta(&meta_path) else {
            errors.push(format!("{slug}: meta.json missing or unreadable"));
            continue;
        };
        total_specs += 1;
        let outcome = meta.outcome.as_deref().unwrap_or("").to_string();
        let is_terminal = TERMINAL_OUTCOMES.contains(&outcome.as_str());
        if is_terminal {
            terminal += 1;
        } else {
            active += 1;
        }

        // Read the anchors. A missing/unreadable spec.md is non-fatal: we keep
        // the lifecycle categorisation (already counted) but cannot compute a
        // staleRatio, so anchors are treated as empty.
        let (stale_files, stale_ratio_x1000) = match fs::read_to_string(spec_dir.join("spec.md")) {
            Ok(text) => compute_stale(&text, root),
            Err(_) => {
                errors.push(format!("{slug}: spec.md unreadable (anchors skipped)"));
                (Vec::new(), 0)
            }
        };

        if is_terminal {
            // Terminal specs are always prune candidates (archivable), stale
            // anchors or not.
            candidates.push(PruneCandidate {
                slug: slug.clone(),
                outcome,
                reason: "terminal",
                stale_files,
                stale_ratio_x1000,
            });
        } else if stale_ratio_x1000 > STALE_RATIO_THRESHOLD_X1000 {
            // Active spec whose code moved away — likely superseded.
            candidates.push(PruneCandidate {
                slug: slug.clone(),
                outcome,
                reason: "stale-anchors",
                stale_files,
                stale_ratio_x1000,
            });
        } else if !stale_files.is_empty() {
            // Active spec with some stale anchors but below threshold — surface
            // as early warning, not yet a prune candidate.
            stale_active.push(PruneCandidate {
                slug: slug.clone(),
                outcome,
                reason: "stale-anchors",
                stale_files,
                stale_ratio_x1000,
            });
        }
    }

    // Slugs were iterated in sorted order, so the candidate/stale lists are
    // already slug-sorted. Errors follow the same order. Be defensive anyway:
    candidates.sort_by(|a, b| a.slug.cmp(&b.slug));
    stale_active.sort_by(|a, b| a.slug.cmp(&b.slug));
    errors.sort();

    let ok = candidates.is_empty() && stale_active.is_empty();
    SupersededReport {
        ok,
        total_specs,
        active,
        terminal,
        prune_candidates: candidates,
        stale_active,
        scanned_errors: errors,
    }
}

/// Parse the `## Files` anchors from `spec_text` and test each repo-relative
/// path for existence under `root`. Returns the sorted list of missing paths
/// and the fixed-point staleRatio (×1000). A spec listing no files yields
/// `(vec![], 0)` — no anchors, no staleness signal.
fn compute_stale(spec_text: &str, root: &Path) -> (Vec<String>, u32) {
    let files = parse_files_section(spec_text).unwrap_or_default();
    if files.is_empty() {
        return (Vec::new(), 0);
    }
    let total = files.len();
    let mut stale: Vec<String> = files
        .into_iter()
        .filter(|rel| !root.join(rel).exists())
        .collect();
    stale.sort();
    stale.dedup();
    // staleRatio ×1000 with integer math: (missing * 1000) / total.
    let ratio_x1000 = ((stale.len() as u64 * 1000) / total as u64) as u32;
    (stale, ratio_x1000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    /// Write `.claude/spec/<slug>/{spec.md,meta.json}` under `root`.
    fn make_spec(root: &Path, slug: &str, outcome: &str, files_section: &str) {
        let dir = root.join(".claude").join("spec").join(slug);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("spec.md"),
            format!("# {slug}\n\n## Files\n{files_section}\n"),
        )
        .unwrap();
        std::fs::write(
            dir.join("meta.json"),
            format!(r#"{{"stage":"Close","outcome":"{outcome}"}}"#),
        )
        .unwrap();
    }

    /// Create a real repo-relative file so its `## Files` anchor resolves.
    fn touch(root: &Path, rel: &str) {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, b"x").unwrap();
    }

    #[test]
    fn terminal_spec_is_a_prune_candidate() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        touch(root, "src/done.rs");
        make_spec(root, "finished-thing", "Completed", "- src/done.rs");

        let report = run(root);
        assert!(!report.ok);
        assert_eq!(report.terminal, 1);
        assert_eq!(report.active, 0);
        assert_eq!(report.prune_candidates.len(), 1);
        let c = &report.prune_candidates[0];
        assert_eq!(c.slug, "finished-thing");
        assert_eq!(c.outcome, "Completed");
        assert_eq!(c.reason, "terminal");
        // Anchor exists → no stale files even though it is terminal.
        assert!(c.stale_files.is_empty());
        assert_eq!(c.stale_ratio_x1000, 0);
    }

    #[test]
    fn active_spec_with_existing_anchor_is_not_a_candidate() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        touch(root, "src/live.rs");
        make_spec(root, "in-progress", "Active", "- src/live.rs");

        let report = run(root);
        assert!(report.ok, "{report:?}");
        assert_eq!(report.active, 1);
        assert_eq!(report.terminal, 0);
        assert!(report.prune_candidates.is_empty());
        assert!(report.stale_active.is_empty());
    }

    #[test]
    fn active_spec_with_missing_anchor_is_stale() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // No file on disk → the anchor is stale (ratio 1000 > 500 threshold).
        make_spec(root, "moved-code", "Active", "- src/gone.rs");

        let report = run(root);
        assert!(!report.ok);
        assert_eq!(report.active, 1);
        // Ratio 1000 (> threshold) → a prune candidate, not stale_active.
        assert_eq!(report.prune_candidates.len(), 1);
        let c = &report.prune_candidates[0];
        assert_eq!(c.slug, "moved-code");
        assert_eq!(c.outcome, "Active");
        assert_eq!(c.reason, "stale-anchors");
        assert_eq!(c.stale_files, vec!["src/gone.rs".to_string()]);
        assert_eq!(c.stale_ratio_x1000, 1000);
    }

    #[test]
    fn active_spec_with_minority_stale_is_early_warning_only() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // 1 of 3 anchors missing → ratio 333 (< 500 threshold) → stale_active.
        touch(root, "a.rs");
        touch(root, "b.rs");
        make_spec(root, "mostly-live", "Active", "- a.rs\n- b.rs\n- c.rs");

        let report = run(root);
        assert!(!report.ok);
        assert!(report.prune_candidates.is_empty());
        assert_eq!(report.stale_active.len(), 1);
        let c = &report.stale_active[0];
        assert_eq!(c.slug, "mostly-live");
        assert_eq!(c.stale_files, vec!["c.rs".to_string()]);
        assert_eq!(c.stale_ratio_x1000, 333);
    }

    #[test]
    fn unreadable_meta_is_skipped_and_recorded() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // A spec dir with no meta.json at all.
        let bad = root.join(".claude").join("spec").join("no-meta");
        std::fs::create_dir_all(&bad).unwrap();
        std::fs::write(bad.join("spec.md"), "# no-meta\n\n## Files\n- x.rs\n").unwrap();
        // A healthy terminal spec alongside it.
        touch(root, "ok.rs");
        make_spec(root, "good", "Completed", "- ok.rs");

        let report = run(root);
        // The bad spec is not counted in total_specs but is in scanned_errors.
        assert_eq!(report.total_specs, 1);
        assert_eq!(report.scanned_errors.len(), 1);
        assert!(report.scanned_errors[0].contains("no-meta"));
        assert_eq!(report.prune_candidates.len(), 1);
        assert_eq!(report.prune_candidates[0].slug, "good");
    }

    #[test]
    fn no_spec_dir_is_ok_and_empty() {
        let dir = tempdir().unwrap();
        let report = run(dir.path());
        assert!(report.ok);
        assert_eq!(report.total_specs, 0);
        assert!(report.prune_candidates.is_empty());
        assert!(report.scanned_errors.is_empty());
    }

    #[test]
    fn output_is_byte_stable_across_runs() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        // A mix: terminal, stale-active, healthy — with slugs deliberately out
        // of lexical order on disk to prove sorting.
        touch(root, "z.rs");
        make_spec(root, "zeta", "Completed", "- z.rs");
        make_spec(root, "alpha", "Active", "- missing-a.rs");
        touch(root, "m.rs");
        make_spec(root, "mid", "Active", "- m.rs\n- missing-m.rs");

        let a = run(root);
        let b = run(root);
        let ja = serde_json::to_string(&a).unwrap();
        let jb = serde_json::to_string(&b).unwrap();
        assert_eq!(ja, jb, "two runs must serialize to identical bytes");

        // And the candidate slugs are sorted.
        let slugs: Vec<&str> =
            a.prune_candidates.iter().map(|c| c.slug.as_str()).collect();
        let mut sorted = slugs.clone();
        sorted.sort_unstable();
        assert_eq!(slugs, sorted, "prune_candidates must be slug-sorted");
    }
}
