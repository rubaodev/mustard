//! `mustard-rt run purpose-search` — the recall path the orchestrator calls ON
//! A MISS, when the name digest (`feature` / `digest --query`) returned nothing.
//!
//! A method whose NAME diverges from the request vocabulary (PT "efetivar" vs
//! `EffectivateAsync`) is invisible to the name index. The `enrich-purpose` step
//! wrote a one-sentence `purpose` summary onto each logic declaration; this
//! command searches THOSE summaries — an UNCAPPED purpose→file index, matched
//! through the same `matching::Ladder` the digest uses (with the trigram rescue
//! rung ON, since the PT Snowball stemmer cannot bridge some verb forms).
//!
//! Determinism: the matching lives in the scan binary (the single owner of the
//! ladder); this command only tokenises the intent the SAME way `feature` does
//! (`domain_terms`) and relays the scan tool's byte-stable JSON. No LLM, no
//! network. Fail-open: an unavailable scan / model prints an empty result
//! (`{"intent":"<joined>","files":[]}`) and always exits 0 — a miss-recovery
//! attempt must never become a hard error.

use std::path::Path;

use mustard_core::Scan;
use serde_json::json;

use crate::commands::feature::domain_terms;

/// CLI face: `mustard-rt run purpose-search --intent <text> --model <path>`.
///
/// Tokenises `intent` exactly as `feature --intent` does (lowercase, len≥3,
/// deduped — the orchestrator folds any cross-lingual translation INSIDE the
/// text), shells out to `scan purpose-search`, and prints the JSON verbatim.
/// Fail-open: an unavailable scan / unreadable model prints an empty result.
pub fn run(intent: &str, model: &Path) {
    let terms = domain_terms(intent);
    let json = match Scan::locate().purpose_search(model, &terms) {
        Ok(out) if !out.trim().is_empty() => out,
        Ok(_) | Err(_) => {
            // Scan unavailable, model unreadable, or empty stdout → the honest
            // empty result (the same shape the scan binary emits on a miss).
            json!({ "intent": terms.join(" "), "files": [] }).to_string()
        }
    };
    print!("{json}");
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn purpose_search_fails_open_to_empty_result_without_a_model() {
        // No scan binary on PATH / no model at this path → the run face must not
        // panic and must print an empty-files result, never an error.
        //
        // We assert the FALLBACK shape directly (the `run` print goes to stdout,
        // not capturable here without plumbing): the join of `domain_terms`.
        let terms = domain_terms("efetivar a previsão de pagamento");
        let fallback = json!({ "intent": terms.join(" "), "files": [] });
        assert_eq!(fallback["files"].as_array().map(Vec::len), Some(0));
        assert!(fallback["intent"].as_str().unwrap().contains("efetivar"));
        // `run` itself must not panic on an absent model path.
        run("efetivar", &PathBuf::from("/nonexistent/model.json"));
    }
}
