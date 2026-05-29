//! Operational-spec head parsing: stage / stub / summary / objective.
//!
//! These helpers are pure string slicers over the first lines (or full body)
//! of an operational `spec.md`. They never touch IO except [`read_first_lines`]
//! which fail-opens to `None`. Shared by [`super::run`], [`super::wave_progress`]
//! and [`super::context_loader`].

use super::{PipelineStateView, SUMMARY_CAP};
use mustard_core::io::fs as mfs;
use std::path::Path;

/// Compact spec path relative to the project root (forward-slash separators).
pub(super) fn relativize(project: &Path, abs: &Path) -> String {
    let stripped = abs.strip_prefix(project).unwrap_or(abs);
    stripped.to_string_lossy().replace('\\', "/")
}

/// Read up to the first `n` lines of a file. `None` on IO error.
pub(super) fn read_first_lines(path: &Path, n: usize) -> Option<String> {
    let text = mfs::read_to_string(path).ok()?;
    let mut out = String::new();
    for (i, line) in text.lines().enumerate() {
        if i >= n {
            break;
        }
        out.push_str(line);
        out.push('\n');
    }
    Some(out)
}

/// Parse `### Key: value` from a header block.
pub(super) fn parse_header_value(text: &str, key_lower: &str) -> Option<String> {
    for line in text.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("### ") else {
            continue;
        };
        let Some(colon) = rest.find(':') else {
            continue;
        };
        let k = rest[..colon].trim();
        if k.eq_ignore_ascii_case(key_lower) {
            let v = rest[colon + 1..].trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Detect the canonical stage word from the spec head, falling back to the
/// pipeline state view's `status`.
pub(super) fn detect_stage(head: &str, view: Option<&PipelineStateView>) -> Option<String> {
    if let Some(stage) = parse_header_value(head, "stage") {
        return Some(normalise_stage(&stage));
    }
    if let Some(v) = view {
        if let Some(s) = v.status.as_deref() {
            return Some(normalise_stage(s));
        }
    }
    None
}

/// Map a stage/status spelling to the canonical PascalCase form.
pub(super) fn normalise_stage(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "plan" | "planning" => "Plan".to_string(),
        "execute" | "implementing" => "Execute".to_string(),
        "analyze" | "analysing" | "analyzing" => "Analyze".to_string(),
        "qareview" | "qa-review" | "qa_review" | "reviewing" => "QaReview".to_string(),
        "close" | "closed" | "closed-followup" | "completed" => "Close".to_string(),
        other => {
            // Title-case fallback.
            let mut chars = other.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => c.to_ascii_uppercase().to_string() + chars.as_str(),
            }
        }
    }
}

/// A stub is `Stage: Plan` with no `## Files`/`## Arquivos`/`## Tasks`/`## Tarefas`
/// section in the first ~30 lines.
pub(super) fn detect_stub(head: &str) -> bool {
    let is_plan = parse_header_value(head, "stage")
        .is_some_and(|s| s.eq_ignore_ascii_case("plan"));
    if !is_plan {
        return false;
    }
    let has_files_or_tasks = head.lines().any(|l| {
        let t = l.trim_start();
        if !t.starts_with("## ") {
            return false;
        }
        let after = t.trim_start_matches('#').trim_start();
        let lower = after.to_lowercase();
        lower.starts_with("files")
            || lower.starts_with("arquivos")
            || lower.starts_with("tasks")
            || lower.starts_with("tarefas")
    });
    !has_files_or_tasks
}

/// Extract first non-empty line under `## Resumo` or `## Summary`, capped to
/// [`SUMMARY_CAP`] chars. Empty when neither heading exists.
pub(super) fn extract_summary(body: &str) -> String {
    let mut in_section = false;
    for line in body.lines() {
        let trimmed = line.trim_end();
        if !in_section {
            let t = trimmed.trim_start();
            if t.starts_with("## ") {
                let after = t.trim_start_matches('#').trim();
                let lower = after.to_lowercase();
                if lower == "resumo" || lower == "summary" {
                    in_section = true;
                }
            }
            continue;
        }
        // We are inside the section — first non-empty line wins.
        if trimmed.trim().is_empty() {
            continue;
        }
        if trimmed.trim_start().starts_with("## ") {
            // Section ended before a content line — bail.
            return String::new();
        }
        let snippet: String = trimmed.trim().chars().take(SUMMARY_CAP).collect();
        return snippet;
    }
    String::new()
}

/// Pick the first non-empty body line under `## Contexto` / `## Context`
/// (with a 240-char cap). Empty when neither section exists — the renderer
/// will substitute the i18n `placeholder.fill` string.
pub(super) fn extract_objective(body: &str) -> String {
    let mut in_section = false;
    for line in body.lines() {
        let trimmed = line.trim_end();
        if !in_section {
            let t = trimmed.trim_start();
            if t.starts_with("## ") {
                let after = t.trim_start_matches('#').trim();
                let lower = after.to_lowercase();
                if lower == "contexto" || lower == "context" {
                    in_section = true;
                }
            }
            continue;
        }
        if trimmed.trim().is_empty() {
            continue;
        }
        if trimmed.trim_start().starts_with("## ") {
            return String::new();
        }
        return trimmed.trim().chars().take(240).collect();
    }
    String::new()
}
