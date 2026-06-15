//! ranking — pure, domain-agnostic BM25 relevance arithmetic.
//!
//! The single home for the Okapi BM25 score shape, shared by every consumer
//! that ranks documents by a query without re-implementing the math:
//! - the scan crate's `digest` (per-term sample ranking over the repo model), and
//! - the rt crate's persistent-memory recall (knowledge/decision bodies vs the
//!   prompt).
//!
//! Fixed-point integer arithmetic (scores ×1024): floats never enter a
//! comparison, so every ranking is byte-stable across runs and platforms. The
//! tuning constants `k1`/`b` are NOT embedded here — they are passed in (each
//! ×1024), so each caller owns its tuning: scan reads `ranking.toml`; a simple
//! consumer uses the classic 1.2 / 0.75 defaults via [`bm25_x1024_default`].
//! Nothing here knows a language, framework, file name or document kind.

/// Fixed-point scale: scores and ratios carry 10 fractional bits.
pub const SCALE: u64 = 1024;

/// Classic BM25 `k1 = 1.2` ×1024 — term-frequency saturation.
pub const DEFAULT_K1_X1024: u64 = 1229; // round(1.2 * 1024)

/// Classic BM25 `b = 0.75` ×1024 — length-normalization strength.
pub const DEFAULT_B_X1024: u64 = 768; // round(0.75 * 1024)

/// Average document length ×1024 over a corpus of `docs` documents totalling
/// `total_len` tokens. Floored at 1 so it can always divide.
#[must_use]
pub fn avgdl_x1024(total_len: usize, docs: usize) -> u64 {
    if docs == 0 {
        return SCALE;
    }
    ((total_len as u64 * SCALE) / docs as u64).max(1)
}

/// BM25 score ×1024 for `tf` occurrences of a term in a document of length
/// `dl`, against the corpus average `avgdl_x1024`. Classic Okapi shape WITHOUT
/// the IDF factor — a premise that holds PER TERM: when documents compete FOR
/// one term, the term's IDF is a common constant and cancels.
///
/// `k1_x1024` / `b_x1024` are the tuning constants ×1024; the caller MUST clamp
/// `b_x1024` to `[0, SCALE]` (the subtraction relies on it). Pure integer
/// arithmetic; ties are broken by the caller (e.g. path asc) for byte-stable
/// output.
#[must_use]
pub fn bm25_x1024(tf: usize, dl: usize, avgdl_x1024: u64, k1_x1024: u64, b_x1024: u64) -> u64 {
    if tf == 0 {
        return 0;
    }
    let tf = tf as u64;
    // dl / avgdl, ×1024.
    let len_ratio_x1024 = (dl as u64 * SCALE * SCALE) / avgdl_x1024.max(1);
    // (1 - b) + b * dl/avgdl, ×1024. b clamped to [0, SCALE] by the caller, so
    // the subtraction never underflows.
    let norm_x1024 = SCALE - b_x1024 + (b_x1024 * len_ratio_x1024) / SCALE;
    // tf + k1 * norm, ×1024.
    let denom_x1024 = tf * SCALE + (k1_x1024 * norm_x1024) / SCALE;
    // tf * (k1 + 1) / denom, ×1024.
    (tf * (SCALE + k1_x1024) * SCALE) / denom_x1024.max(1)
}

/// [`bm25_x1024`] with the classic 1.2 / 0.75 tuning — the convenience a simple
/// consumer (no `ranking.toml`) uses.
#[must_use]
pub fn bm25_x1024_default(tf: usize, dl: usize, avgdl_x1024: u64) -> u64 {
    bm25_x1024(tf, dl, avgdl_x1024, DEFAULT_K1_X1024, DEFAULT_B_X1024)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bm25_grows_with_tf_and_saturates() {
        let avg = avgdl_x1024(10, 10);
        let s1 = bm25_x1024_default(1, 1, avg);
        let s2 = bm25_x1024_default(2, 1, avg);
        let s8 = bm25_x1024_default(8, 1, avg);
        assert!(s2 > s1, "more tf scores higher");
        assert!(s8 > s2);
        // Saturation: the jump 1→2 exceeds the jump 7→8.
        assert!(s2 - s1 > s8 - bm25_x1024_default(7, 1, avg));
    }

    #[test]
    fn bm25_normalizes_by_document_length() {
        let avg = avgdl_x1024(102, 2); // corpus: one 100-decl + one 2-decl module
        let sprawling = bm25_x1024_default(2, 100, avg);
        let focused = bm25_x1024_default(1, 2, avg);
        assert!(focused > sprawling, "a focused short doc outranks a sprawling one");
    }

    #[test]
    fn bm25_zero_tf_scores_zero() {
        assert_eq!(bm25_x1024_default(0, 5, avgdl_x1024(10, 2)), 0);
    }

    #[test]
    fn avgdl_floors_at_one_and_handles_empty_corpus() {
        assert_eq!(avgdl_x1024(0, 0), SCALE, "empty corpus → SCALE sentinel");
        assert_eq!(avgdl_x1024(0, 5), 1, "floored at 1 so it can always divide");
    }

    #[test]
    fn defaults_match_classic_constants() {
        assert_eq!(DEFAULT_K1_X1024, (1.2_f64 * SCALE as f64).round() as u64);
        assert_eq!(DEFAULT_B_X1024, (0.75_f64 * SCALE as f64).round() as u64);
    }
}
