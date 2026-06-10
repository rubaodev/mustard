//! Integration contract over the digest's tier-ladder matching, driven
//! through the binary (`digest --query [--lang]` over a synthetic
//! `grain.model.json`):
//!   * the prefix>=4 false cognates are DEAD: a request token never matches
//!     an index token it merely truncates onto ("cores" ~ "core",
//!     "cancelado" ~ "cancel"), in any language configuration;
//!   * the whole identifier is indexed lowercased, so a same-case/concatenated
//!     request term lands at tier `exact` ("parentid");
//!   * same-language stems bridge real morphology only ("studies" ~ "study"),
//!     reported as tier `stem` with the language named;
//!   * the bilingual seed lexicon bridges across languages with the tier and
//!     pair reported ("cancelado" -> "cancel" via pt-en), and ONLY when the
//!     request language activates the pair — no glossary, no bridge;
//!   * the answer carries the per-term report (term, tier, lang, files) plus
//!     the aggregate matched k/n and reason; byte-stable across runs.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Write a synthetic `grain.model.json` into a temp dir owned by the test.
/// Mirrors `term_index.rs`; the `label` keeps parallel tests' dirs distinct.
fn write_model(label: &str, modules: serde_json::Value) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!("scan-match-tiers-{}-{}", label, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let model = dir.join("grain.model.json");
    let v = serde_json::json!({ "root": dir.to_string_lossy(), "modules": modules });
    std::fs::write(&model, serde_json::to_string_pretty(&v).unwrap()).unwrap();
    (dir, model)
}

/// One synthetic module carrying the given declaration names.
fn module(path: &str, decls: &[&str]) -> serde_json::Value {
    let declarations: Vec<serde_json::Value> =
        decls.iter().map(|n| serde_json::json!({ "kind": "class", "name": n, "line": 1 })).collect();
    serde_json::json!({ "path": path, "declarations": declarations })
}

/// Run `digest --query` (with an explicit `--lang`; empty = the binary reads
/// the root config, absent in these temp dirs) and return raw bytes + JSON.
fn run_query(model: &Path, query: &str, lang: &str, out_name: &str) -> (String, serde_json::Value) {
    let out_file = model.parent().unwrap().join(out_name);
    let out = Command::new(env!("CARGO_BIN_EXE_scan"))
        .args(["digest", model.to_str().unwrap(), "--query", query, "--lang", lang, "--out", out_file.to_str().unwrap()])
        .output()
        .expect("run digest --query over model");
    assert!(out.status.success(), "stderr: {}", String::from_utf8_lossy(&out.stderr));
    let raw = std::fs::read_to_string(&out_file).expect("read query result");
    let v = serde_json::from_str(&raw).expect("valid query JSON");
    (raw, v)
}

/// The single per-term report entry of a one-term query.
fn sole_report_term(q: &serde_json::Value) -> &serde_json::Value {
    let terms = q["report"]["terms"].as_array().expect("report.terms");
    assert_eq!(terms.len(), 1, "one request term, one report entry: {q}");
    &terms[0]
}

#[test]
fn false_cognate_cores_never_matches_core() {
    // The motivating defect: the old prefix>=4 rule matched the pt word
    // "cores" onto the en identifier token "core". Dead in EVERY language
    // configuration — both stemmers reduce the pair to one key, but a bare
    // truncation relation is never accepted as stem evidence.
    let (dir, model) = write_model("cores", serde_json::json!([module("src/core/engine.rs", &["CoreEngine"])]));
    for (lang, out) in [("", "q-en.json"), ("pt-BR", "q-pt.json")] {
        let (_, q) = run_query(&model, "cores", lang, out);
        assert!(q["matched_terms"].as_array().unwrap().is_empty(), "cores must not match core (lang={lang:?}): {q}");
        assert_eq!(q["miss"], true);
        let t = sole_report_term(&q);
        assert_eq!(t["term"], "cores");
        assert_eq!(t["tier"], "none", "named miss, not silence: {q}");
        assert_eq!(q["report"]["matched"], 0);
        assert_eq!(q["report"]["total"], 1);
        assert_eq!(q["report"]["reason"], "none");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn cancelado_needs_the_glossary_and_reports_its_tier() {
    // "cancelado" vs the identifier token "cancel": a truncation pair, so no
    // tier bridges it by form alone. Without the pt-en pair active (request
    // language en-only) it is an honest miss; with the request declared pt,
    // the seed lexicon bridges it (cancelar -> cancel, the inflected request
    // reaches the entry via the same-language stem) and the report names the
    // tier and the pair.
    let (dir, model) = write_model("cancelado", serde_json::json!([module("src/billing/cancel.rs", &["CancelCharge"])]));

    let (_, without) = run_query(&model, "cancelado", "", "q-nolex.json");
    assert!(without["matched_terms"].as_array().unwrap().is_empty(), "no glossary, no bridge: {without}");
    assert_eq!(sole_report_term(&without)["tier"], "none");

    let (_, with) = run_query(&model, "cancelado", "pt-BR", "q-lex.json");
    let matched: Vec<&str> =
        with["matched_terms"].as_array().unwrap().iter().map(|t| t["term"].as_str().unwrap()).collect();
    assert!(matched.contains(&"cancel"), "glossary bridges onto the repo's own vocabulary: {with}");
    assert_eq!(with["miss"], false);
    let t = sole_report_term(&with);
    assert_eq!(t["tier"], "lexicon", "tier reported: {with}");
    assert_eq!(t["lang"], "pt-en", "pair reported: {with}");
    assert_eq!(t["files"][0], "src/billing/cancel.rs", "report carries the files where the match lives: {with}");
    assert_eq!(with["report"]["matched"], 1);
    assert_eq!(with["report"]["total"], 1);
    // A lexicon-only answer is honest about its strength: no exact/fold hit.
    assert_eq!(with["report"]["reason"], "weak", "derived-only evidence is weak, not false confidence: {with}");
    let files: Vec<&str> = with["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
    assert!(files.contains(&"src/billing/cancel.rs"), "anchor still lands: {with}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn whole_identifier_matches_exactly() {
    // "ParentId" tokenizes to ["parent"] alone (the "id" half is under the
    // token floor) — the OLD index could never answer "parentid". The whole
    // lowercased identifier is now one extra entry per declaration, an exact
    // tier-1 key.
    let (dir, model) = write_model("ident", serde_json::json!([module("src/titles/parent.rs", &["ParentId", "SplitAsync"])]));
    let (_, q) = run_query(&model, "parentid", "", "q-ident.json");

    let matched: Vec<&str> = q["matched_terms"].as_array().unwrap().iter().map(|t| t["term"].as_str().unwrap()).collect();
    assert_eq!(matched, vec!["parentid"], "whole-ident exact entry: {q}");
    let t = sole_report_term(&q);
    assert_eq!(t["tier"], "exact");
    assert_eq!(t["lang"], "", "exact equality carries no language evidence");
    assert_eq!(q["report"]["reason"], "strong");
    let files: Vec<&str> = q["files"].as_array().unwrap().iter().map(|f| f.as_str().unwrap()).collect();
    assert_eq!(files, vec!["src/titles/parent.rs"], "ident match anchors its defining file: {q}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn same_language_stem_bridges_real_morphology_only() {
    // "studies" ~ "study": same-language stems agree AND the surfaces are not
    // a bare truncation pair — tier `stem`, language named. Recall the dead
    // prefix rule never had (neither form is a prefix of the other), gained
    // without resurrecting false cognates.
    let (dir, model) = write_model("stem", serde_json::json!([module("src/plan/study.rs", &["StudyPlan"])]));
    let (_, q) = run_query(&model, "studies", "", "q-stem.json");

    let matched: Vec<&str> = q["matched_terms"].as_array().unwrap().iter().map(|t| t["term"].as_str().unwrap()).collect();
    assert_eq!(matched, vec!["study"], "stem tier finds the morphological variant: {q}");
    let t = sole_report_term(&q);
    assert_eq!(t["tier"], "stem");
    assert_eq!(t["lang"], "en", "the stemmer language is the evidence: {q}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn report_aggregates_matched_k_of_n_and_is_byte_stable() {
    // Two terms, one hit: the aggregate is matched 1/2 and every term gets a
    // named outcome. Two binary invocations emit identical bytes — the whole
    // ladder (stems, lexicon, report) is deterministic.
    let (dir, model) = write_model("aggregate", serde_json::json!([module("src/billing/cancel.rs", &["CancelCharge"])]));
    let (raw1, q) = run_query(&model, "cancelado,hierarquia", "pt-BR", "q1.json");
    let (raw2, _) = run_query(&model, "cancelado,hierarquia", "pt-BR", "q2.json");
    assert_eq!(raw1, raw2, "identical bytes across runs");

    assert_eq!(q["report"]["matched"], 1);
    assert_eq!(q["report"]["total"], 2);
    let terms = q["report"]["terms"].as_array().unwrap();
    assert_eq!(terms.len(), 2);
    assert_eq!(terms[0]["term"], "cancelado");
    assert_eq!(terms[0]["tier"], "lexicon");
    assert_eq!(terms[1]["term"], "hierarquia");
    assert_eq!(terms[1]["tier"], "none", "the missed term is a NAMED miss: {q}");

    let _ = std::fs::remove_dir_all(&dir);
}
