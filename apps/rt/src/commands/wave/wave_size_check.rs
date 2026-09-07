//! `mustard-rt run wave-size-check` — a port of `scripts/wave-size-check.js`.
//!
//! Advisory audit of per-wave size inside a wave-plan. `exec-rewave-check` only
//! decomposes a flat spec; once a spec is a wave-plan nothing flags an
//! oversized individual wave. This audits each wave and WARNS (never blocks).
//!
//! Output: one JSON line. The `oversizedCount` field is parsed downstream, so
//! the shape is preserved exactly.
//!
//! Port note: the JS version shelled to `wave-tree.js` and `scope-decompose.js`.
//! Both are now in this binary — this port calls the Rust logic directly.

use crate::commands::spec::scope_decompose::decide;
use crate::commands::wave::wave_lib::{detect_role_with, load_role_patterns, parse_files_section};
use mustard_core::RolePattern;
use mustard_core::io::fs;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::path::Path;

/// Resolve the file-count threshold (default 10, floor 3).
fn resolve_limit() -> usize {
    env_limit("MUSTARD_WAVE_SIZE_LIMIT", 10)
}

/// Resolve the task-count threshold (default 10, floor 3).
///
/// A wave is ONE agent in ONE pass. Files measure how wide it reaches; tasks
/// measure how long it has to stay coherent, and they are not the same number —
/// measured in the field, a wave was accepted at 19 files AND 13 tasks, and the
/// audit only ever looked at the files. The failure mode tasks catch is specific:
/// quality falls off at the end of a long list, and a failure at task 11 wastes
/// the ten before it.
fn resolve_task_limit() -> usize {
    env_limit("MUSTARD_WAVE_TASK_LIMIT", 10)
}

/// A `usize` threshold from `var`, floored at 3, defaulting to `default`.
fn env_limit(var: &str, default: usize) -> usize {
    std::env::var(var)
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .map_or(default, |n| if n < 3 { 3 } else { n as usize })
}

/// An enumerated wave folder.
struct WaveFolder {
    folder: String,
}

/// Enumerate wave folders for a spec dir, or `None` when it is not a wave-plan.
fn enumerate_waves(spec_dir: &Path) -> Option<Vec<WaveFolder>> {
    if !spec_dir.join("wave-plan.md").exists() {
        return None;
    }
    let mut folders: Vec<String> = fs::read_dir(spec_dir)
        .map(|entries| {
            entries
                .into_iter()
                .filter(|e| e.is_dir)
                .map(|e| e.file_name)
                .filter(|n| {
                    // `^wave-\d+`
                    let lower = n.to_lowercase();
                    lower.starts_with("wave-")
                        && lower[5..].chars().next().is_some_and(|c| c.is_ascii_digit())
                })
                .collect()
        })
        .unwrap_or_default();
    folders.sort_by_key(|f| wave_number_of(f).unwrap_or(0));
    if folders.is_empty() {
        return None;
    }
    Some(folders.into_iter().map(|folder| WaveFolder { folder }).collect())
}

/// Extract a wave number from a folder name.
fn wave_number_of(name: &str) -> Option<u32> {
    let start = name.find(|c: char| c.is_ascii_digit())?;
    let end = name[start..]
        .find(|c: char| !c.is_ascii_digit())
        .map_or(name.len(), |e| start + e);
    name[start..end].parse().ok()
}

/// Try to extract a wave's file list from `wave-plan.md` (for stub waves).
fn files_from_wave_plan(spec_dir: &Path, wave_num: Option<u32>) -> Option<Vec<String>> {
    let wave_num = wave_num?;
    let text = fs::read_to_string(spec_dir.join("wave-plan.md")).ok()?;
    let lines: Vec<&str> = text.split('\n').map(|l| l.trim_end_matches('\r')).collect();

    // 1. `### Wave N` section → `Files (N): a, b, c`.
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim();
        if !is_wave_header(t, wave_num) {
            continue;
        }
        for next in lines.iter().skip(i + 1) {
            let l = next.trim();
            if l.starts_with("## ") || l.starts_with("### ") || l.starts_with("#### ") {
                break;
            }
            if let Some(rest) = strip_files_prefix(l) {
                let parts: Vec<String> = rest
                    .split(',')
                    .map(|s| s.trim().trim_matches('`').to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !parts.is_empty() {
                    return Some(parts);
                }
            }
        }
    }

    // 2. table row `| W3 | ... |` with a file-list cell.
    for line in &lines {
        let t = line.trim();
        if !is_table_row_for_wave(t, wave_num) {
            continue;
        }
        let cells: Vec<&str> = t.split('|').map(str::trim).filter(|c| !c.is_empty()).collect();
        for c in cells {
            if (c.contains('/') || c.contains('\\')) && c.contains(',') {
                let parts: Vec<String> = c
                    .split(',')
                    .map(|s| s.trim().trim_matches('`').to_string())
                    .filter(|s| s.contains('/') || s.contains('\\'))
                    .collect();
                if !parts.is_empty() {
                    return Some(parts);
                }
            }
        }
    }
    None
}

/// `^#{2,4}\s*Wave\s*N\b`
fn is_wave_header(line: &str, wave_num: u32) -> bool {
    let hashes = line.chars().take_while(|c| *c == '#').count();
    if !(2..=4).contains(&hashes) {
        return false;
    }
    let rest = line[hashes..].trim_start();
    let lower = rest.to_lowercase();
    let Some(after) = lower.strip_prefix("wave") else {
        return false;
    };
    let after = after.trim_start();
    let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok() == Some(wave_num)
        && after[digits.len()..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
}

/// `^Files\s*\(\d+\)\s*:\s*(.+)$`
fn strip_files_prefix(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("Files").or_else(|| line.strip_prefix("files"))?;
    let rest = rest.trim_start();
    let rest = rest.strip_prefix('(')?;
    let close = rest.find(')')?;
    if rest[..close].is_empty() || !rest[..close].chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let rest = rest[close + 1..].trim_start();
    let rest = rest.strip_prefix(':')?;
    let body = rest.trim_start();
    if body.is_empty() {
        None
    } else {
        Some(body)
    }
}

/// `^\|\s*W?N\b`
fn is_table_row_for_wave(line: &str, wave_num: u32) -> bool {
    let Some(rest) = line.strip_prefix('|') else {
        return false;
    };
    let rest = rest.trim_start();
    let rest = rest.strip_prefix(['W', 'w']).unwrap_or(rest);
    let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok() == Some(wave_num)
        && rest[digits.len()..]
            .chars()
            .next()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_'))
}

/// Audit a single wave.
fn audit_wave(
    wave: &WaveFolder,
    spec_dir: &Path,
    limit: usize,
    task_limit: usize,
    role_patterns: &[RolePattern],
    model_path: &Path,
    project_root: &Path,
) -> Value {
    let folder = &wave.folder;
    let wave_num = wave_number_of(folder);

    // Prefer the wave's own spec.md `## Files` section.
    let mut files: Option<Vec<String>> = None;
    let mut source: Option<&str> = None;
    let mut task_count = 0usize;
    let wave_spec_path = spec_dir.join(folder).join("spec.md");
    if wave_spec_path.exists() {
        if let Ok(text) = fs::read_to_string(&wave_spec_path) {
            task_count = count_task_items(&text);
            if let Some(parsed) = parse_files_section(&text) {
                if !parsed.is_empty() {
                    files = Some(parsed);
                    source = Some("wave-spec");
                }
            }
        }
    }
    if files.is_none() {
        if let Some(plan_files) = files_from_wave_plan(spec_dir, wave_num) {
            if !plan_files.is_empty() {
                files = Some(plan_files);
                source = Some("wave-plan");
            }
        }
    }

    let Some(files) = files else {
        let status = if wave_spec_path.exists() {
            "unknown"
        } else {
            "stub"
        };
        return json!({ "wave": wave_num, "folder": folder, "status": status });
    };

    let file_count = files.len();
    let roles: BTreeSet<String> = files.iter().map(|f| detect_role_with(f, role_patterns)).collect();
    let layer_count = if roles.len() == 1 && roles.contains("lib") {
        1
    } else {
        roles.len()
    };

    // Stack-awareness: the role→layer `multi-layer` signal is only trustworthy
    // where the gate was tuned (JS/TS). On a foreign-language wave (C#, Python,
    // …) it fires on every intrinsically cross-layer backend feature — a
    // guaranteed false alarm. Loosen there: keep only the language-agnostic
    // file-count reason. Resolved from the wave's `## Files` extensions plus the
    // repo model's detected stacks (both fail-open, so a JS/TS or undetected
    // wave keeps the historical layer signal).
    let langs = mustard_core::resolve_target_languages(&files, model_path, project_root);
    let understood = mustard_core::target_understood(&langs);

    let mut reasons: Vec<String> = Vec::new();
    if understood {
        let decision = decide(&json!({
            "fileCount": file_count,
            "layerCount": layer_count,
            "newEntityCount": 0,
            "knowledgeMatches": [],
        }));
        if decision.get("decompose").and_then(Value::as_bool) == Some(true) {
            if let Some(reason) = decision.get("reason").and_then(Value::as_str) {
                reasons.push(reason.to_string());
            }
        }
    }
    if file_count > limit {
        reasons.push(format!("file-count:{file_count}>{limit}"));
    }
    if task_count > task_limit {
        reasons.push(format!("task-count:{task_count}>{task_limit}"));
    }
    let oversized = !reasons.is_empty();

    json!({
        "wave": wave_num,
        "folder": folder,
        "fileCount": file_count,
        "taskCount": task_count,
        "layerCount": layer_count,
        "languages": langs.into_iter().collect::<Vec<_>>(),
        "oversized": oversized,
        "reason": reasons.join("; "),
        "source": source,
    })
}

/// Count the checklist items under a wave spec's `## Tasks` / `## Tarefas`
/// heading — top-level `- ` bullets only, so a sub-bullet elaborating one task
/// is not counted as another task.
fn count_task_items(text: &str) -> usize {
    let lines: Vec<&str> = text.lines().collect();
    let Some(start) = lines
        .iter()
        .position(|l| crate::commands::spec::spec_sections::is_heading(l, "tasks"))
    else {
        return 0;
    };
    lines
        .iter()
        .skip(start + 1)
        .take_while(|l| !l.starts_with("## "))
        .filter(|l| l.starts_with("- "))
        .count()
}

/// Dispatch `mustard-rt run wave-size-check`.
pub fn run(spec_dir_arg: Option<&str>) {
    let emit = |v: Value| println!("{v}");
    let Some(spec_dir_arg) = spec_dir_arg else {
        emit(json!({ "action": "skip", "reason": "no-spec-dir-arg" }));
        return;
    };
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    // Accept the three spec-dir spellings (a directory, a `…/spec.md` path, a
    // bare slug) through the shared normaliser before the cwd join.
    // Root resolution matches the sibling call sites (`wave_tree`,
    // `pipeline_summary`, `plan_materialize`): `project_dir()` honours
    // `CLAUDE_PROJECT_DIR`, so a bare slug resolves identically across all four
    // commands the normaliser exists to unify.
    let resolved = crate::shared::context::normalise_spec_dir(
        Path::new(&crate::shared::context::project_dir()),
        spec_dir_arg,
    );
    let spec_dir = if resolved.is_absolute() {
        resolved
    } else {
        cwd.join(resolved)
    };
    if !spec_dir.exists() {
        emit(json!({
            "action": "skip",
            "reason": "error-fallback",
            "error": "spec-dir-not-found",
        }));
        return;
    }

    emit(audit(&spec_dir));
}

/// Audit every wave of `spec_dir` and return the report — the miolo of [`run`],
/// callable IN-PROCESS.
///
/// It is `pub(crate)` for one reason: this audit computed its numbers and no
/// step of the pipeline ever looked at them. `wave-size-check` shipped as a
/// command nobody called, so a plan was accepted with a 19-file, 13-task wave
/// with no warning at any stage — the plan report even printed `widestWave: 19`
/// and did nothing with it. [`warn_oversized_waves`] is the caller that closes
/// that gap.
pub(crate) fn audit(spec_dir: &Path) -> Value {
    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let Some(waves) = enumerate_waves(spec_dir) else {
        return json!({ "action": "skip", "reason": "not-a-wave-plan" });
    };

    let limit = resolve_limit();
    let task_limit = resolve_task_limit();
    // F0-e: honour `mustard.json#rolePatterns` so non-English / non-JS layers
    // classify correctly. Resolve from the workspace anchor, fail-open to cwd.
    let project_root = crate::shared::context::workspace_root_strict().unwrap_or_else(|_| cwd);
    let role_patterns = load_role_patterns(&project_root);
    let model_path = project_root.join(".claude").join("grain.model.json");
    let audited: Vec<Value> = waves
        .iter()
        .map(|w| {
            audit_wave(
                w,
                spec_dir,
                limit,
                task_limit,
                &role_patterns,
                &model_path,
                &project_root,
            )
        })
        .collect();
    let oversized_count = audited
        .iter()
        .filter(|w| w.get("oversized").and_then(Value::as_bool) == Some(true))
        .count();

    json!({
        "action": "audited",
        "specDir": spec_dir.to_string_lossy(),
        "limit": limit,
        "taskLimit": task_limit,
        "oversizedCount": oversized_count,
        "waves": audited,
    })
}

/// Run [`audit`] over a freshly materialised plan and WARN on stderr for each
/// oversized wave. Advisory — it never blocks, and it never touches stdout (the
/// materialise report is machine-read and must stay byte-stable).
///
/// **Why the warning is worth a line each.** A wave is one agent in one pass.
/// Past the ceiling the last tasks get the worst work, and a failure late in the
/// list throws away everything before it. The operator cannot see that from a
/// plan that materialised successfully — every artefact is there, every file is
/// listed, nothing failed. This is the only moment the shape of the plan is
/// visible and still cheap to change.
pub(crate) fn warn_oversized_waves(spec_dir: &Path) {
    let report = audit(spec_dir);
    if report.get("oversizedCount").and_then(Value::as_u64).unwrap_or(0) == 0 {
        return;
    }
    let Some(waves) = report.get("waves").and_then(Value::as_array) else {
        return;
    };
    for w in waves {
        if w.get("oversized").and_then(Value::as_bool) != Some(true) {
            continue;
        }
        let folder = w.get("folder").and_then(Value::as_str).unwrap_or("?");
        let files = w.get("fileCount").and_then(Value::as_u64).unwrap_or(0);
        let tasks = w.get("taskCount").and_then(Value::as_u64).unwrap_or(0);
        let reason = w.get("reason").and_then(Value::as_str).unwrap_or("");
        eprintln!(
            "[wave-size] WARN: {folder} is oversized ({files} files, {tasks} tasks — {reason}). \
             A wave is ONE agent in ONE pass: split off the tasks that share no file with the \
             rest — they are a wave of their own, and they can run in parallel."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn resolve_limit_floors_at_three() {
        // Default applies when env unset.
        assert!(resolve_limit() >= 3);
    }

    #[test]
    fn wave_number_extraction() {
        assert_eq!(wave_number_of("wave-3-backend"), Some(3));
        assert_eq!(wave_number_of("wave-12"), Some(12));
    }

    #[test]
    fn audits_wave_plan_with_oversized_wave() {
        let dir = tempdir().unwrap();
        let spec_dir = dir.path();
        std::fs::write(spec_dir.join("wave-plan.md"), "# plan\n").unwrap();
        let wave_dir = spec_dir.join("wave-1-backend");
        std::fs::create_dir_all(&wave_dir).unwrap();
        let mut files = String::from("## Files\n");
        for i in 0..14 {
            files.push_str(&format!("- src/api/h{i}.ts\n"));
        }
        std::fs::write(wave_dir.join("spec.md"), files).unwrap();
        let waves = enumerate_waves(spec_dir).unwrap();
        // No grain model on disk → detected-stacks signal is empty; the `.ts`
        // extensions carry the (understood) language, so the file-count reason
        // still fires.
        let no_model = spec_dir.join("no-model.json");
        let audited = audit_wave(&waves[0], spec_dir, 10, 10, &[], &no_model, spec_dir);
        assert_eq!(audited["oversized"], json!(true));
        assert_eq!(audited["fileCount"], json!(14));
        assert_eq!(audited["languages"], json!(["typescript"]));
    }

    #[test]
    fn not_a_wave_plan_skips() {
        let dir = tempdir().unwrap();
        assert!(enumerate_waves(dir.path()).is_none());
    }

    /// A wave is oversized by its TASK count too, not only by its files.
    ///
    /// Measured in the field: a plan was accepted carrying a wave of 19 files
    /// AND 13 tasks, and this audit only ever looked at the files — so a wave
    /// that is narrow but very long slipped through every stage in silence.
    /// A wave is one agent in one pass: past the ceiling the last tasks get the
    /// worst work, and a failure at task 11 wastes the ten before it.
    #[test]
    fn a_long_task_list_is_oversized_even_with_few_files() {
        let dir = tempdir().unwrap();
        let spec_dir = dir.path();
        std::fs::write(spec_dir.join("wave-plan.md"), "# plan\n").unwrap();
        let wave_dir = spec_dir.join("wave-1-backend");
        std::fs::create_dir_all(&wave_dir).unwrap();
        let tasks: String = (1..=13).map(|i| format!("- [ ] task {i}\n")).collect();
        std::fs::write(
            wave_dir.join("spec.md"),
            format!("## Files\n- src/a.rs\n- src/b.rs\n\n## Tasks\n{tasks}"),
        )
        .unwrap();
        let waves = enumerate_waves(spec_dir).unwrap();
        let no_model = spec_dir.join("no-model.json");
        let audited = audit_wave(&waves[0], spec_dir, 10, 10, &[], &no_model, spec_dir);
        assert_eq!(audited["fileCount"], json!(2), "well under the file limit: {audited}");
        assert_eq!(audited["taskCount"], json!(13), "{audited}");
        assert_eq!(audited["oversized"], json!(true), "{audited}");
        assert!(
            audited["reason"].as_str().unwrap_or_default().contains("task-count:13>10"),
            "the reason names WHICH ceiling was crossed: {audited}"
        );
    }

    #[test]
    fn foreign_language_wave_suppresses_multi_layer_but_keeps_file_count() {
        let dir = tempdir().unwrap();
        let spec_dir = dir.path();
        std::fs::write(spec_dir.join("wave-plan.md"), "# plan\n").unwrap();
        let wave_dir = spec_dir.join("wave-1-backend");
        std::fs::create_dir_all(&wave_dir).unwrap();
        // A C# wave spanning DTOs + Services + Controllers — layerCount >= 2, but
        // only three files (under the size limit). The `multi-layer` alarm must
        // be suppressed for a foreign language; nothing flags it oversized.
        let files = "## Files\n\
            - backend/App/DTOs/Payable.cs\n\
            - backend/App/Services/Recurrence.cs\n\
            - backend/App/Controllers/PayableController.cs\n";
        std::fs::write(wave_dir.join("spec.md"), files).unwrap();
        let waves = enumerate_waves(spec_dir).unwrap();
        let no_model = spec_dir.join("no-model.json");
        let audited = audit_wave(&waves[0], spec_dir, 10, 10, &[], &no_model, spec_dir);
        assert!(audited["layerCount"].as_u64().unwrap() >= 2, "C# folders span layers: {audited}");
        assert_eq!(audited["languages"], json!(["csharp"]));
        assert_eq!(audited["oversized"], json!(false), "multi-layer suppressed for foreign lang: {audited}");
        assert_eq!(audited["reason"], json!(""));
    }

    #[test]
    fn js_ts_wave_still_flags_multi_layer() {
        let dir = tempdir().unwrap();
        let spec_dir = dir.path();
        std::fs::write(spec_dir.join("wave-plan.md"), "# plan\n").unwrap();
        let wave_dir = spec_dir.join("wave-1-app");
        std::fs::create_dir_all(&wave_dir).unwrap();
        // The layer signal is preserved for the language the gate was tuned for.
        let files = "## Files\n\
            - src/schema/user.ts\n\
            - src/api/users.ts\n\
            - src/components/UserCard.tsx\n";
        std::fs::write(wave_dir.join("spec.md"), files).unwrap();
        let waves = enumerate_waves(spec_dir).unwrap();
        let no_model = spec_dir.join("no-model.json");
        let audited = audit_wave(&waves[0], spec_dir, 10, 10, &[], &no_model, spec_dir);
        assert!(audited["layerCount"].as_u64().unwrap() >= 2);
        assert_eq!(audited["oversized"], json!(true), "multi-layer preserved for JS/TS: {audited}");
        assert!(audited["reason"].as_str().unwrap().contains("multi-layer"));
    }
}
