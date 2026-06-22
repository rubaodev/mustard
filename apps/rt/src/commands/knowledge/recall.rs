//! `recall` — the single deterministic relevance-recall over the unified
//! [`Knowledge`] store.
//!
//! ## Why this exists
//!
//! The render used to inject a *passive pointer* to cross-wave memory
//! ("pull on demand, skip unless needed"). The knowledge existed but rarely
//! flowed, and cross-spec knowledge never reached the agents at all. This
//! module turns that passive transfer into a **relevant push**: given a query
//! (the agent's role + task text) and a [`Scope`], it returns the most relevant
//! records, ranked by BM25 and bounded by a relevance threshold — never a count
//! or a size.
//!
//! ## Contract
//!
//! [`recall`] is THE function for relevance recall over the knowledge store.
//! The render calls it; it does NOT re-implement ranking. The BM25 arithmetic
//! is the core's byte-stable fixed-point math
//! ([`mustard_core::domain::ranking`]) — no second BM25, no substring matcher.
//!
//! ## Determinism + fail-open
//!
//! - The corpus comes from [`KnowledgeStore::read_all`] (itself sorted by path).
//! - Ties break on the record's [`Knowledge::slug`] (byte-stable), so the order
//!   never depends on filesystem enumeration.
//! - An empty store, an empty query, or a query whose terms hit nothing all
//!   yield `vec![]` — the render then collapses the empty block.

use std::collections::HashSet;
use std::path::Path;

use mustard_core::domain::model::knowledge::{Knowledge, Scope};
use mustard_core::domain::ranking::{avgdl_x1024, bm25_x1024_default};
use mustard_core::io::knowledge_store::KnowledgeStore;
use mustard_core::time::now_iso8601;

use super::effective_confidence;

/// Fixed-point scale for the decayed-confidence weight (×1024, mirroring the
/// BM25 fixed-point convention so the whole score stays integer arithmetic and
/// byte-stable). A record at full confidence keeps its raw BM25; a fully-decayed
/// record's score collapses to zero and sinks to the bottom of the ranking.
const CONFIDENCE_SCALE: u64 = 1024;

/// Floor the confidence weight at this fraction of full (×1024) so a genuinely
/// relevant but low-confidence record is *demoted*, never erased from recall —
/// the relevance threshold, not the decay, owns membership. (`0.05 × 1024`.)
const MIN_CONFIDENCE_WEIGHT: u64 = 51;

/// Relevance floor as a fraction of the best score — the anti-bloat bound that
/// mirrors `context_inject::MEMORY_RELEVANCE_FLOOR_FRACTION`. A record survives
/// only when it scores at least this share of the top match: this drops the
/// weak tail that would otherwise flood the prompt, while a lone genuine match
/// (its own top) always clears its own bar. Relevance bounds the set —
/// precision, never a number, decides membership.
const RELEVANCE_FLOOR_FRACTION: u64 = 348; // 0.34 ×1024 (rounded), fixed-point

/// Minimum content-token length: shorter tokens (`the`, `a`, `is`) match too
/// broadly to discriminate, so the query is tokenised on ≥4-char lowercase
/// alphanumeric runs (one char looser than name stems — knowledge bodies carry
/// more common words than a terse file name).
const MIN_TERM_LEN: usize = 4;

/// Recall the records most relevant to `query` from the knowledge store rooted
/// at `claude_root` (the project's `.claude/` directory), restricted to `scope`
/// when `Some`.
///
/// ## Ranking
///
/// The query is tokenised into distinct content terms (≥[`MIN_TERM_LEN`] chars,
/// lowercased). The corpus is every record from
/// [`KnowledgeStore::read_all`]`(scope)`, each document being its `label` +
/// `content`. For each document the score is the sum over the query's terms of
/// [`bm25_x1024_default`]`(tf, dl, avgdl)` — the core's byte-stable fixed-point
/// BM25. Documents are ranked score-desc, slug-asc on ties.
///
/// ## Threshold
///
/// Only documents scoring at least [`RELEVANCE_FLOOR_FRACTION`] of the top score
/// survive (anti-bloat — the weak tail is cut). The survivors are then truncated
/// to `max`. Relevance is the only filter; the truncation is a safety cap on an
/// already-relevant set, never the membership decision.
///
/// ## Fail-open
///
/// An empty/whitespace query, an empty store, or a query that matches nothing
/// returns `vec![]`. Never panics.
#[must_use]
pub fn recall(claude_root: &Path, query: &str, scope: Option<&Scope>, max: usize) -> Vec<Knowledge> {
    recall_scored(claude_root, query, scope, max)
        .into_iter()
        .map(|(_, k)| k)
        .collect()
}

/// Like [`recall`], but each survivor is paired with its raw BM25 score
/// (×1024 fixed-point — the same byte-stable integer the threshold cut on).
///
/// The CLI face renders the score so a recall result is *measurable* — the
/// number, not a yes/no, is the diagnostic. [`recall`] is the convenience
/// wrapper that drops the score for callers (the render) that only want the
/// records. The same fail-open / determinism contract applies: empty inputs
/// yield `vec![]`, ties break on slug, never panics.
#[must_use]
pub fn recall_scored(
    claude_root: &Path,
    query: &str,
    scope: Option<&Scope>,
    max: usize,
) -> Vec<(u64, Knowledge)> {
    // Read the wall clock ONCE here; the actual ranking is the pure, clock-
    // injected `recall_scored_at`, so the decay weighting is deterministic given
    // `now` and a test can pin it for a stable, reproducible ordering.
    recall_scored_at(claude_root, query, scope, max, &now_iso8601())
}

/// The pure core of [`recall_scored`] with the clock injected as `now_iso` (an
/// ISO-8601 instant). Same contract as [`recall_scored`], but deterministic
/// given `now_iso`: the only non-determinism (the wall clock that drives the
/// confidence decay) is lifted to a parameter so tests pin it. The public
/// wrapper reads `now` once and forwards it here.
#[must_use]
pub fn recall_scored_at(
    claude_root: &Path,
    query: &str,
    scope: Option<&Scope>,
    max: usize,
    now_iso: &str,
) -> Vec<(u64, Knowledge)> {
    if max == 0 {
        return Vec::new();
    }
    let terms = query_terms(query);
    if terms.is_empty() {
        return Vec::new();
    }

    let corpus = KnowledgeStore::new(claude_root).read_all(scope);
    if corpus.is_empty() {
        return Vec::new();
    }

    // Tokenise each document once; the corpus stats (total length, doc count)
    // feed the shared `avgdl` so BM25's length normalisation is corpus-aware.
    let docs: Vec<Vec<String>> = corpus.iter().map(doc_terms).collect();
    let total_len: usize = docs.iter().map(Vec::len).sum();
    let avgdl = avgdl_x1024(total_len, docs.len());

    // Score every document: sum the per-term BM25 over the query terms, then
    // weight by the record's *decayed* confidence (the feedback loop — an old or
    // low-confidence note sinks below a fresh, trusted one with the same textual
    // match). A term absent from the document contributes 0 (bm25 of tf=0), so a
    // document with no query overlap scores 0 and is dropped below.
    let mut scored: Vec<(u64, Knowledge)> = Vec::with_capacity(corpus.len());
    for (doc, record) in docs.iter().zip(corpus) {
        let dl = doc.len();
        let mut bm25 = 0_u64;
        for term in &terms {
            let tf = doc.iter().filter(|t| *t == term).count();
            bm25 = bm25.saturating_add(bm25_x1024_default(tf, dl, avgdl));
        }
        if bm25 == 0 {
            continue;
        }
        // Decay weight (×1024) from the SAME shared curve `prune`/`memory` use:
        // `confidence` attenuated by the age of `captured_at` relative to `now`.
        // Floored so a relevant low-confidence record is demoted, never erased
        // (the relevance threshold owns membership). Fixed-point throughout so
        // the score stays a byte-stable integer.
        let eff = effective_confidence(
            f64::from(record.confidence),
            Some(record.origin.captured_at.as_str()),
            now_iso,
        );
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let weight = ((eff * CONFIDENCE_SCALE as f64).round() as u64).max(MIN_CONFIDENCE_WEIGHT);
        let score = bm25.saturating_mul(weight) / CONFIDENCE_SCALE;
        if score > 0 {
            scored.push((score, record));
        }
    }
    if scored.is_empty() {
        return Vec::new();
    }

    // Best-first; slug-asc breaks ties for a byte-stable, deterministic order
    // independent of the store's enumeration.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.slug().cmp(&b.1.slug())));

    // Relevance threshold (anti-bloat): keep only records within
    // RELEVANCE_FLOOR_FRACTION of the top score. Fixed-point: `score * frac`
    // ×1024, compared against `top * SCALE` — no float ever enters the cut.
    if let Some(&(top, _)) = scored.first() {
        let floor_x1024 = top.saturating_mul(RELEVANCE_FLOOR_FRACTION);
        scored.retain(|(score, _)| score.saturating_mul(1024) >= floor_x1024);
    }
    scored.truncate(max);
    scored
}

/// The distinct query terms: ≥[`MIN_TERM_LEN`]-char lowercase alphanumeric runs,
/// deduplicated, order-preserving.
fn query_terms(query: &str) -> Vec<String> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::new();
    for tok in query
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
    {
        if tok.len() >= MIN_TERM_LEN && seen.insert(tok.to_string()) {
            out.push(tok.to_string());
        }
    }
    out
}

/// Tokenise a document (`label` + `content`) into its term bag — every ≥
/// [`MIN_TERM_LEN`]-char lowercase alphanumeric run, WITH duplicates (term
/// frequency is the BM25 signal). The label is folded in so a record whose
/// headline carries the keyword still ranks even when the body is terse.
fn doc_terms(k: &Knowledge) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let combined = format!("{} {}", k.label, k.content);
    for tok in combined
        .to_ascii_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
    {
        if tok.len() >= MIN_TERM_LEN {
            out.push(tok.to_string());
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustard_core::domain::model::knowledge::{Kind, Origin, Status};
    use tempfile::tempdir;

    fn write(store: &KnowledgeStore, kind: Kind, scope: Scope, label: &str, content: &str) {
        write_full(store, kind, scope, label, content, 0.6, "2026-06-15T00:00:00.000Z");
    }

    /// Like [`write`] but with explicit `confidence` and `captured_at`, so a test
    /// can pin the two inputs the decay weighting consumes.
    #[allow(clippy::too_many_arguments)]
    fn write_full(
        store: &KnowledgeStore,
        kind: Kind,
        scope: Scope,
        label: &str,
        content: &str,
        confidence: f32,
        captured_at: &str,
    ) {
        let origin = Origin {
            spec: scope.spec().map(str::to_string),
            wave: scope.wave(),
            captured_at: captured_at.into(),
            ..Origin::default()
        };
        store
            .write(&Knowledge {
                kind,
                scope,
                label: label.into(),
                content: content.into(),
                origin,
                confidence,
                status: Status::Active,
            })
            .expect("write knowledge");
    }

    #[test]
    fn empty_store_yields_empty() {
        let dir = tempdir().unwrap();
        // Point at a `.claude/` that has never been written to.
        assert!(recall(&dir.path().join(".claude"), "anything at all", None, 3).is_empty());
    }

    #[test]
    fn empty_query_yields_empty() {
        let dir = tempdir().unwrap();
        let store = KnowledgeStore::new(dir.path());
        write(&store, Kind::Lesson, Scope::Global, "x", "real content here");
        // No ≥4-char term → no recall (fail-open, not a match-all).
        assert!(recall(dir.path(), "a is on", None, 3).is_empty());
        assert!(recall(dir.path(), "   ", None, 3).is_empty());
    }

    #[test]
    fn bm25_ranks_by_relevance() {
        let dir = tempdir().unwrap();
        let store = KnowledgeStore::new(dir.path());
        // Strong: the query term appears, focused.
        write(
            &store,
            Kind::Decision,
            Scope::Global,
            "caching policy",
            "the dashboard caching layer keys on path length mtime",
        );
        // Weak: mentions the topic once, buried in unrelated text.
        write(
            &store,
            Kind::Lesson,
            Scope::Global,
            "misc notes",
            "many unrelated words about routing and tabs and caching once here too",
        );
        // None: no overlap at all — must never surface.
        write(
            &store,
            Kind::Lesson,
            Scope::Global,
            "totally other",
            "kubernetes deployment manifests and ingress rules",
        );

        let hits = recall(dir.path(), "fix the caching layer", None, 5);
        assert!(!hits.is_empty(), "the caching records must surface");
        assert_eq!(hits[0].label, "caching policy", "most relevant ranks first");
        assert!(
            !hits.iter().any(|k| k.label == "totally other"),
            "a zero-overlap record must never surface"
        );
    }

    #[test]
    fn threshold_drops_the_weak_tail() {
        let dir = tempdir().unwrap();
        let store = KnowledgeStore::new(dir.path());
        // Strong match: the query terms saturate a short, focused doc.
        write(
            &store,
            Kind::Decision,
            Scope::Global,
            "recall ranking design",
            "recall ranking recall ranking bm25 threshold relevance recall",
        );
        // Weak match: a single coincidental term in a long, noisy doc — below
        // the relevance floor relative to the strong match.
        write(
            &store,
            Kind::Lesson,
            Scope::Global,
            "sprawling log",
            &format!("{} ranking", "noise unrelated padding chatter filler ".repeat(20)),
        );

        let hits = recall(dir.path(), "recall ranking relevance", None, 10);
        assert_eq!(hits.len(), 1, "the weak tail is cut by the threshold: {:?}",
            hits.iter().map(|k| &k.label).collect::<Vec<_>>());
        assert_eq!(hits[0].label, "recall ranking design");
    }

    #[test]
    fn scope_filter_restricts_the_corpus() {
        let dir = tempdir().unwrap();
        let store = KnowledgeStore::new(dir.path());
        write(
            &store,
            Kind::Summary,
            Scope::Wave { spec: "demo".into(), wave: 1 },
            "wave one summary",
            "delivered the caching subcommand for the dashboard",
        );
        write(
            &store,
            Kind::Decision,
            Scope::Global,
            "global decision",
            "caching is the right approach for the dashboard",
        );
        // Wave scope → only the wave-1 record is eligible.
        let only_wave = recall(
            dir.path(),
            "caching dashboard",
            Some(&Scope::Wave { spec: "demo".into(), wave: 1 }),
            5,
        );
        assert_eq!(only_wave.len(), 1);
        assert_eq!(only_wave[0].label, "wave one summary");
        // Global scope → only the global record.
        let only_global = recall(dir.path(), "caching dashboard", Some(&Scope::Global), 5);
        assert_eq!(only_global.len(), 1);
        assert_eq!(only_global[0].label, "global decision");
    }

    #[test]
    fn max_caps_an_already_relevant_set() {
        let dir = tempdir().unwrap();
        let store = KnowledgeStore::new(dir.path());
        // Three near-equally relevant records — all clear the threshold.
        for i in 0..3 {
            write(
                &store,
                Kind::Lesson,
                Scope::Global,
                &format!("caching note {i}"),
                "caching layer caching layer caching relevance",
            );
        }
        let hits = recall(dir.path(), "caching relevance", None, 2);
        assert_eq!(hits.len(), 2, "max caps the relevant set");
    }

    #[test]
    fn order_is_deterministic_across_calls() {
        let dir = tempdir().unwrap();
        let store = KnowledgeStore::new(dir.path());
        for i in 0..4 {
            write(
                &store,
                Kind::Lesson,
                Scope::Global,
                &format!("caching note {i}"),
                "caching layer relevance ranking",
            );
        }
        let a = recall(dir.path(), "caching relevance ranking", None, 10);
        let b = recall(dir.path(), "caching relevance ranking", None, 10);
        let labels = |v: &[Knowledge]| v.iter().map(|k| k.label.clone()).collect::<Vec<_>>();
        assert_eq!(labels(&a), labels(&b), "slug-tiebreak makes order stable");
    }

    #[test]
    fn decayed_confidence_demotes_an_old_record_with_the_same_match() {
        // Two records with the SAME textual match (identical label+body → same
        // BM25), differing only in confidence and age. The fresh, high-confidence
        // one must rank ABOVE the stale, low-confidence one. Clock is pinned, so
        // the decay weighting is deterministic.
        let now = "2026-06-15T00:00:00.000Z";
        let dir = tempdir().unwrap();
        let store = KnowledgeStore::new(dir.path());
        // Fresh + high confidence (captured "now", confidence 0.9).
        write_full(
            &store,
            Kind::Decision,
            Scope::Global,
            "caching layer fresh",
            "the caching layer keys on path length mtime",
            0.9,
            now,
        );
        // Old + low confidence (captured 25 days earlier, confidence 0.2) — same
        // searchable text, so identical raw BM25; only the decay weight differs.
        write_full(
            &store,
            Kind::Decision,
            Scope::Global,
            "caching layer stale",
            "the caching layer keys on path length mtime",
            0.2,
            "2026-05-21T00:00:00.000Z",
        );

        let hits = recall_scored_at(dir.path(), "caching layer mtime", None, 5, now);
        let labels: Vec<&str> = hits.iter().map(|(_, k)| k.label.as_str()).collect();
        // The fresh record is first; the stale one is demoted strictly below it —
        // either it ranks after fresh, or the decay sank it under the relevance
        // floor and it dropped out entirely. Both are correct "demoted" outcomes.
        assert_eq!(
            labels.first().copied(),
            Some("caching layer fresh"),
            "the fresh, high-confidence record outranks the stale one: {labels:?}",
        );
        let pos_fresh = labels
            .iter()
            .position(|l| *l == "caching layer fresh")
            .expect("fresh record present");
        match labels.iter().position(|l| *l == "caching layer stale") {
            Some(pos_stale) => assert!(
                pos_fresh < pos_stale,
                "fresh must precede stale on identical BM25: {labels:?}",
            ),
            None => { /* stale demoted below the relevance floor — the strongest form */ }
        }
    }

    #[test]
    fn age_alone_demotes_on_equal_confidence_and_match() {
        // Isolate the DECAY axis: two records with identical confidence AND
        // identical textual match (same raw BM25), differing only in age. The
        // fresh one must outrank the aged one — proving time, not confidence,
        // moved the ranking. Clock pinned for determinism.
        let now = "2026-06-15T00:00:00.000Z";
        let dir = tempdir().unwrap();
        let store = KnowledgeStore::new(dir.path());
        write_full(
            &store,
            Kind::Decision,
            Scope::Global,
            "caching layer fresh",
            "the caching layer keys on path length mtime",
            0.8,
            now,
        );
        // Same confidence (0.8), captured 12 days earlier → mild decay, survives
        // the floor but ranks below the fresh twin.
        write_full(
            &store,
            Kind::Decision,
            Scope::Global,
            "caching layer aged",
            "the caching layer keys on path length mtime",
            0.8,
            "2026-06-03T00:00:00.000Z",
        );

        let hits = recall_scored_at(dir.path(), "caching layer mtime", None, 5, now);
        let labels: Vec<&str> = hits.iter().map(|(_, k)| k.label.as_str()).collect();
        let pos_fresh = labels.iter().position(|l| *l == "caching layer fresh");
        let pos_aged = labels.iter().position(|l| *l == "caching layer aged");
        assert_eq!(pos_fresh, Some(0), "the fresh twin ranks first: {labels:?}");
        // The aged twin survives (mild decay) but strictly below the fresh one —
        // the only difference between them is age.
        assert!(
            matches!((pos_fresh, pos_aged), (Some(f), Some(a)) if f < a),
            "equal confidence + match: the fresher record wins on decay alone: {labels:?}",
        );
    }

    #[test]
    fn recent_high_confidence_decays_near_one_and_order_is_stable() {
        // A record captured at `now` with confidence ~1 keeps essentially its
        // full BM25 (decay factor ≈ 1), and repeat calls are byte-stable.
        let now = "2026-06-15T00:00:00.000Z";
        let dir = tempdir().unwrap();
        let store = KnowledgeStore::new(dir.path());
        write_full(
            &store,
            Kind::Lesson,
            Scope::Global,
            "fresh note",
            "recall ranking relevance bm25 decay weighting",
            1.0,
            now,
        );
        let a = recall_scored_at(dir.path(), "recall ranking relevance", None, 5, now);
        let b = recall_scored_at(dir.path(), "recall ranking relevance", None, 5, now);
        assert_eq!(a.len(), 1, "the fresh record surfaces");
        assert_eq!(
            a.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            b.iter().map(|(s, _)| *s).collect::<Vec<_>>(),
            "scores are deterministic given the pinned clock",
        );
    }
}
