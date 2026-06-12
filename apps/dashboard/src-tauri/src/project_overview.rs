//! Project-overview projection for the dashboard overview card.
//!
//! [`dashboard_project_overview`] reads the grain model
//! (`.claude/grain.model.json`) via [`mustard_core::read_projects`] — never
//! parsing the model's own schema directly — and projects the small,
//! card-ready shape: whether the workspace is a monorepo, how many compilation
//! units it has, and the distinct languages, frameworks, and detected stacks
//! mined across them.
//!
//! FAIL-OPEN CONTRACT (mirrors every dashboard command): a missing model (no
//! scan yet) yields an all-empty overview — `read_projects` already returns an
//! empty vec on a missing/malformed model — so the card shows an empty state
//! rather than an error toast.

use mustard_core::read_projects;
use serde::Serialize;
use std::collections::BTreeSet;
use std::path::PathBuf;

/// One inferred stack, flattened for the frontend (the model's
/// `StackDetection` carries auditable `signals` the card does not render).
#[derive(Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct StackSummary {
    pub name: String,
    pub confidence: f32,
}

/// Per-unit projection so the card can list each subproject of a monorepo
/// rather than only the workspace-wide aggregates. One per `projects[]` entry.
#[derive(Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ProjectUnitSummary {
    /// Unit name from the model.
    pub name: String,
    /// Unit directory relative to the repo root; falls back to `name` when the
    /// model carries no `dir`.
    pub dir: String,
    /// The unit's `kind` (e.g. `cargo`, `npm`, `go`) — the only per-unit
    /// language signal the model carries.
    pub language: String,
    /// Frameworks/deps mined for this unit (frequency-ranked by the model).
    pub frameworks: Vec<String>,
    /// Stacks inferred for this unit, flattened to name + confidence.
    pub stacks: Vec<StackSummary>,
}

/// Card-ready projection of the workspace's grain model.
#[derive(Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ProjectOverview {
    /// `true` when the model declares more than one compilation unit.
    pub is_monorepo: bool,
    /// Number of compilation units (subprojects) in the model.
    pub project_count: usize,
    /// Distinct languages across units, derived from each unit's `kind`
    /// (e.g. `cargo`, `npm`, `go`) — the only language signal the model
    /// carries per unit. Sorted, deduped.
    pub languages: Vec<String>,
    /// Distinct frameworks/deps mined across units. Sorted, deduped.
    pub frameworks: Vec<String>,
    /// Distinct inferred stacks across units, keeping the highest confidence
    /// seen for each stack name.
    pub detected_stacks: Vec<StackSummary>,
    /// One entry per compilation unit, so the card can render the monorepo's
    /// members instead of only the workspace-wide aggregates. Stably ordered
    /// by `dir` then `name`.
    pub units: Vec<ProjectUnitSummary>,
}

/// Project the grain model at `repo_path` into a [`ProjectOverview`]. Always
/// returns `Ok`; a missing/unscanned model degrades to an empty overview.
#[tauri::command]
pub fn dashboard_project_overview(repo_path: String) -> Result<ProjectOverview, String> {
    let model = PathBuf::from(&repo_path)
        .join(".claude")
        .join("grain.model.json");
    let projects = read_projects(&model);

    let project_count = projects.len();
    let mut languages: BTreeSet<String> = BTreeSet::new();
    let mut frameworks: BTreeSet<String> = BTreeSet::new();
    // Highest confidence wins per stack name.
    let mut stacks: std::collections::BTreeMap<String, f32> = std::collections::BTreeMap::new();
    let mut units: Vec<ProjectUnitSummary> = Vec::with_capacity(project_count);

    for project in &projects {
        if !project.kind.is_empty() {
            languages.insert(project.kind.clone());
        }
        for framework in &project.frameworks {
            frameworks.insert(framework.clone());
        }
        for stack in &project.detected_stacks {
            stacks
                .entry(stack.name.clone())
                .and_modify(|c| {
                    if stack.confidence > *c {
                        *c = stack.confidence;
                    }
                })
                .or_insert(stack.confidence);
        }

        // The model carries `dir` per unit, but older models / a root unit may
        // leave it empty — fall back to the name so the card always has a key.
        let dir = if project.dir.is_empty() {
            project.name.clone()
        } else {
            project.dir.clone()
        };
        units.push(ProjectUnitSummary {
            name: project.name.clone(),
            dir,
            language: project.kind.clone(),
            frameworks: project.frameworks.clone(),
            stacks: project
                .detected_stacks
                .iter()
                .map(|s| StackSummary {
                    name: s.name.clone(),
                    confidence: s.confidence,
                })
                .collect(),
        });
    }

    // Stable order: by directory, then name, so the card list never jitters
    // between scans.
    units.sort_by(|a, b| a.dir.cmp(&b.dir).then_with(|| a.name.cmp(&b.name)));

    Ok(ProjectOverview {
        is_monorepo: project_count > 1,
        project_count,
        languages: languages.into_iter().collect(),
        frameworks: frameworks.into_iter().collect(),
        detected_stacks: stacks
            .into_iter()
            .map(|(name, confidence)| StackSummary { name, confidence })
            .collect(),
        units,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_yields_empty_overview() {
        let dir = tempfile::tempdir().unwrap();
        let overview =
            dashboard_project_overview(dir.path().to_string_lossy().into_owned()).unwrap();
        assert!(!overview.is_monorepo);
        assert_eq!(overview.project_count, 0);
        assert!(overview.languages.is_empty());
        assert!(overview.frameworks.is_empty());
        assert!(overview.detected_stacks.is_empty());
        assert!(overview.units.is_empty());
    }
}
