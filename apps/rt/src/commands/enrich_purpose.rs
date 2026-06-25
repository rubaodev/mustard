//! `mustard-rt run enrich-purpose` — T2 (render) + T3 (apply).
//!
//! **Render** (`--render`): reads the grain model, filters `method`/`function`
//! declarations, slices their source bodies (~55 lines), and emits a byte-stable
//! batch prompt to stdout asking an LLM to write a one-sentence business-action
//! summary for each.
//!
//! **Apply** (`--apply <file>`): reads a JSON array
//! `[{"id":"<module_path>#<name>#<line>","purpose":"..."}]` produced by the LLM,
//! finds each declaration in the model at `modules[].declarations[]`, computes a
//! SHA-256 of the current body, and writes `purpose` + `body_hash` back
//! atomically — skipping unchanged bodies (incremental).
//!
//! No LLM/network calls in this binary — pure data in/out. The grain model
//! uses `modules[].path` as the module identifier; declarations carry `kind`,
//! `name`, `line` (+ the new additive `purpose`/`body_hash` fields from T2/T3).

use std::collections::BTreeMap;
use std::path::Path;

use crate::util::sha256::Sha256;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract ~`cap` lines starting at `start_line` (1-based), following braces.
fn slice_body(source: &str, start_line: usize, cap: usize) -> String {
    let lines: Vec<&str> = source.lines().collect();
    let start = start_line.saturating_sub(1);
    if start >= lines.len() {
        return String::new();
    }
    let mut depth = 0i32;
    let mut end = start;
    let mut found_open = false;
    for (i, line) in lines[start..].iter().enumerate() {
        if i >= cap {
            break;
        }
        for ch in line.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    found_open = true;
                }
                '}' => {
                    depth -= 1;
                }
                _ => {}
            }
        }
        end = start + i;
        if found_open && depth <= 0 {
            break;
        }
    }
    lines[start..=end].join("\n")
}

/// Human-readable locale name for the prompt.
fn lang_display(lang: &str) -> &str {
    if lang.starts_with("pt") {
        "Portuguese (pt-BR)"
    } else if lang.starts_with("en") {
        "English"
    } else {
        lang
    }
}

/// Detect the project language from `mustard.json` at `root`.
fn resolve_lang(root: &Path) -> String {
    let cfg = mustard_core::domain::config::ProjectConfig::load(root);
    cfg.lang.or(cfg.spec_lang).unwrap_or_else(|| "en".to_string())
}

// ---------------------------------------------------------------------------
// T2 — render
// ---------------------------------------------------------------------------

/// Resolve the project root for source-file lookups. The scan records the
/// authoritative root in the model's `root` field; the `module_path`s are
/// relative to it. The Windows `\\?\` extended-length prefix is stripped so the
/// forward-slash module paths join cleanly. Falls back to the model file's
/// parent dir (correct when model and sources are co-located, e.g. in tests).
fn workspace_root_from_model(model: &serde_json::Value, model_path: &Path) -> std::path::PathBuf {
    if let Some(r) = model.get("root").and_then(|v| v.as_str()) {
        let r = r.strip_prefix(r"\\?\").unwrap_or(r);
        if !r.is_empty() {
            return std::path::PathBuf::from(r);
        }
    }
    model_path.parent().unwrap_or_else(|| Path::new(".")).to_path_buf()
}

pub fn run_render(model_path: &Path, root: &Path) {
    // Fail-open: a missing or unparseable model → empty output, exit 0.
    let raw = match std::fs::read_to_string(model_path) {
        Ok(s) => s,
        Err(_) => return,
    };
    let model: serde_json::Value = match serde_json::from_str(&raw) {
        Ok(m) => m,
        Err(_) => return,
    };

    let lang = resolve_lang(root);
    // Source files are relative to the project root the scan recorded.
    let workspace_root = workspace_root_from_model(&model, model_path);

    // Collect all method/function declarations across all modules.
    // id = "<module_path>#<name>#<line>" — sorted via BTreeMap for byte-stability.
    let mut entries: BTreeMap<String, (String, usize)> = BTreeMap::new(); // id -> (module_path, line)

    if let Some(modules) = model.get("modules").and_then(|v| v.as_array()) {
        for module in modules {
            let module_path = module.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if module_path.is_empty() {
                continue;
            }
            if let Some(decls) = module.get("declarations").and_then(|v| v.as_array()) {
                for decl in decls {
                    let kind = decl.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                    if kind != "method" && kind != "function" {
                        continue;
                    }
                    let name = decl.get("name").and_then(|v| v.as_str()).unwrap_or("");
                    let line = decl.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                    if name.is_empty() {
                        continue;
                    }
                    let id = format!("{}#{}#{}", module_path, name, line);
                    entries.insert(id, (module_path.to_string(), line));
                }
            }
        }
    }

    if entries.is_empty() {
        return;
    }

    // Build the prompt. BTreeMap iteration is sorted → byte-stable.
    let header = format!(
        "You are summarizing code functions. For each function below, write EXACTLY one sentence in {} describing the business action it performs. Output a JSON array: [{{\"id\": \"...\", \"purpose\": \"...\"}}].\n",
        lang_display(&lang)
    );

    let mut body = String::new();
    for (id, (module_path, line)) in &entries {
        // Source file is relative to workspace root.
        let src_path = workspace_root.join(module_path);
        let source = match std::fs::read_to_string(&src_path) {
            Ok(s) => s,
            Err(_) => continue, // fail-open: skip unreadable files
        };
        let snippet = slice_body(&source, *line, 55);
        if snippet.is_empty() {
            continue;
        }
        body.push_str(&format!("\n### {}\n```\n{}\n```\n", id, snippet));
    }

    if body.is_empty() {
        return;
    }

    print!("{}{}", header, body);
}

// ---------------------------------------------------------------------------
// T3 — apply
// ---------------------------------------------------------------------------

pub fn run_apply(apply_path: &Path, model_path: &Path) {
    // Read the apply file.
    let apply_raw = match std::fs::read_to_string(apply_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("enrich-purpose apply: cannot read {}: {e}", apply_path.display());
            return;
        }
    };
    let entries: Vec<serde_json::Value> = match serde_json::from_str(&apply_raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("enrich-purpose apply: bad JSON in {}: {e}", apply_path.display());
            return;
        }
    };

    // Read the current model.
    let model_raw = match std::fs::read_to_string(model_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("enrich-purpose apply: cannot read model {}: {e}", model_path.display());
            return;
        }
    };
    let mut model: serde_json::Value = match serde_json::from_str(&model_raw) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("enrich-purpose apply: cannot parse model {}: {e}", model_path.display());
            return;
        }
    };

    // Source files are relative to the project root the scan recorded.
    let workspace_root = workspace_root_from_model(&model, model_path);

    // Apply each entry.
    for entry in &entries {
        let id = match entry.get("id").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };
        let purpose = match entry.get("purpose").and_then(|v| v.as_str()) {
            Some(s) => s,
            None => continue,
        };

        // id = "<module_path>#<name>#<line>"
        let parts: Vec<&str> = id.splitn(3, '#').collect();
        if parts.len() < 3 {
            continue;
        }
        let (module_path, name, line_str) = (parts[0], parts[1], parts[2]);
        let line: usize = match line_str.parse() {
            Ok(n) => n,
            Err(_) => continue,
        };

        // Read source to compute current body hash.
        let src_path = workspace_root.join(module_path);
        let source = match std::fs::read_to_string(&src_path) {
            Ok(s) => s,
            Err(_) => continue, // fail-open
        };
        let body = slice_body(&source, line, 55);
        let mut hasher = Sha256::new();
        hasher.update(body.as_bytes());
        let current_hash = hasher.hex_digest();

        // Find and update the matching declaration in model.modules[].declarations[].
        let modules = match model.get_mut("modules").and_then(|v| v.as_array_mut()) {
            Some(arr) => arr,
            None => continue,
        };
        let mut found = false;
        'outer: for module in modules.iter_mut() {
            let m_path = module.get("path").and_then(|v| v.as_str()).unwrap_or("");
            if m_path != module_path {
                continue;
            }
            let decls = match module.get_mut("declarations").and_then(|v| v.as_array_mut()) {
                Some(arr) => arr,
                None => continue,
            };
            for decl in decls.iter_mut() {
                let d_kind = decl.get("kind").and_then(|v| v.as_str()).unwrap_or("");
                if d_kind != "method" && d_kind != "function" {
                    continue;
                }
                let d_name = decl.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let d_line = decl.get("line").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                if d_name != name || d_line != line {
                    continue;
                }
                // Incremental: skip if body_hash matches current.
                let stored_hash = decl.get("body_hash").and_then(|v| v.as_str()).unwrap_or("");
                if stored_hash == current_hash {
                    found = true;
                    break 'outer;
                }
                // Update purpose and body_hash.
                if let Some(obj) = decl.as_object_mut() {
                    obj.insert("purpose".to_string(), serde_json::Value::String(purpose.to_string()));
                    obj.insert("body_hash".to_string(), serde_json::Value::String(current_hash.clone()));
                }
                found = true;
                break 'outer;
            }
        }
        if !found {
            eprintln!("enrich-purpose apply: id not found in model: {id}");
        }
    }

    // Serialize and write atomically.
    let out = match serde_json::to_string_pretty(&model) {
        Ok(s) => s + "\n",
        Err(e) => {
            eprintln!("enrich-purpose apply: cannot serialize model: {e}");
            return;
        }
    };
    if let Err(e) = mustard_core::io::fs::write_atomic(model_path, out.as_bytes()) {
        eprintln!("enrich-purpose apply: cannot write model {}: {e}", model_path.display());
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run(render: bool, apply: Option<&Path>, model_path: &Path, root: &Path) {
    if let Some(apply_path) = apply {
        run_apply(apply_path, model_path);
    } else if render {
        run_render(model_path, root);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn enrich_purpose_render() {
        let dir = tempdir().unwrap();
        // Write a source file relative to the tempdir (workspace root).
        let src_path = dir.path().join("src").join("payments.rs");
        fs::create_dir_all(src_path.parent().unwrap()).unwrap();
        fs::write(
            &src_path,
            "fn process_payment(amount: f64) {\n    // stub\n}\n\nstruct PaymentRecord {}\n",
        )
        .unwrap();

        let model = serde_json::json!({
            "root": dir.path().to_str().unwrap(),
            "modules": [
                {
                    "path": "src/payments.rs",
                    "language": "rust",
                    "loc": 5,
                    "imports": [],
                    "namespaces": [],
                    "declarations": [
                        { "kind": "function", "name": "process_payment", "line": 1 },
                        { "kind": "struct",   "name": "PaymentRecord",   "line": 5 }
                    ]
                }
            ]
        });
        let model_path = dir.path().join("model.json");
        fs::write(&model_path, serde_json::to_string_pretty(&model).unwrap()).unwrap();

        // Test slice_body determinism (byte-stability).
        let src_content = fs::read_to_string(&src_path).unwrap();
        let s1 = slice_body(&src_content, 1, 55);
        let s2 = slice_body(&src_content, 1, 55);
        assert_eq!(s1, s2, "slice_body must be deterministic");
        assert!(s1.contains("process_payment"), "body must contain function name");

        // lang_display correctness.
        assert_eq!(lang_display("pt-BR"), "Portuguese (pt-BR)");
        assert_eq!(lang_display("en"), "English");
        assert_eq!(lang_display("de"), "de");

        // run render without panicking.
        run(true, None, &model_path, dir.path());
    }

    #[test]
    fn enrich_purpose_apply_incremental() {
        let dir = tempdir().unwrap();
        // Source file in the workspace root (same dir as model).
        let src_path = dir.path().join("lib.rs");
        fs::write(&src_path, "fn activate_payment() {\n    // pay\n}\n").unwrap();

        let model = serde_json::json!({
            "root": dir.path().to_str().unwrap(),
            "modules": [
                {
                    "path": "lib.rs",
                    "language": "rust",
                    "loc": 3,
                    "imports": [],
                    "namespaces": [],
                    "declarations": [
                        { "kind": "function", "name": "activate_payment", "line": 1 }
                    ]
                }
            ]
        });
        let model_path = dir.path().join("model.json");
        fs::write(&model_path, serde_json::to_string_pretty(&model).unwrap()).unwrap();

        // First apply: sets purpose + body_hash.
        let apply_data = serde_json::json!([
            { "id": "lib.rs#activate_payment#1", "purpose": "Activates a payment transaction." }
        ]);
        let apply_path = dir.path().join("apply.json");
        fs::write(&apply_path, serde_json::to_string(&apply_data).unwrap()).unwrap();

        run(false, Some(&apply_path), &model_path, dir.path());

        let updated: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&model_path).unwrap()).unwrap();
        let decl = &updated["modules"][0]["declarations"][0];
        assert_eq!(decl["purpose"].as_str().unwrap(), "Activates a payment transaction.");
        let hash1 = decl["body_hash"].as_str().unwrap().to_string();
        assert!(!hash1.is_empty(), "body_hash must be set");

        // Second apply (same body, different purpose text): should be a no-op.
        let apply_data2 = serde_json::json!([
            { "id": "lib.rs#activate_payment#1", "purpose": "DIFFERENT PURPOSE" }
        ]);
        fs::write(&apply_path, serde_json::to_string(&apply_data2).unwrap()).unwrap();
        run(false, Some(&apply_path), &model_path, dir.path());

        let updated2: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&model_path).unwrap()).unwrap();
        let decl2 = &updated2["modules"][0]["declarations"][0];
        // Since body didn't change, hash matches → incremental skip → purpose unchanged.
        assert_eq!(
            decl2["purpose"].as_str().unwrap(),
            "Activates a payment transaction.",
            "incremental: unchanged body must not overwrite purpose"
        );
    }

    #[test]
    fn enrich_purpose_language_agnostic() {
        // Test lang_display with different inputs.
        assert_eq!(lang_display("en"), "English");
        assert_eq!(lang_display("en-US"), "English");
        assert_eq!(lang_display("pt-BR"), "Portuguese (pt-BR)");
        assert_eq!(lang_display("pt"), "Portuguese (pt-BR)");
        assert_eq!(lang_display("es"), "es");

        // resolve_lang with no mustard.json → defaults to "en".
        let dir = tempdir().unwrap();
        let lang = resolve_lang(dir.path());
        assert_eq!(lang, "en", "no mustard.json → default lang en");

        // With mustard.json specifying lang.
        fs::write(
            dir.path().join("mustard.json"),
            r#"{"lang":"pt-BR"}"#,
        )
        .unwrap();
        let lang2 = resolve_lang(dir.path());
        assert_eq!(lang2, "pt-BR");

        // With specLang (and no lang).
        let dir2 = tempdir().unwrap();
        fs::write(
            dir2.path().join("mustard.json"),
            r#"{"specLang":"pt-BR"}"#,
        )
        .unwrap();
        let lang3 = resolve_lang(dir2.path());
        assert_eq!(lang3, "pt-BR");
    }
}
