//! Deterministic model FACTS — the small, stable projection the ORCHESTRATOR
//! (Mustard) consumes instead of parsing `grain.model.json` itself.
//!
//! Two facts an orchestrator needs without reading source or the (large) model:
//! the subproject list (one per build manifest) and the known declaration names
//! (entities/types/functions). Keeping this here makes `scan` the single owner
//! of the model schema — consumers depend only on this tiny JSON shape, never on
//! the model's internals. A pure projection of the deterministic model, so it is
//! deterministic too. Nothing here is language- or framework-specific.

use crate::model::{ProjectModel, ProjectUnit};
use serde::Serialize;

#[derive(Serialize)]
pub struct ModelFacts {
    /// Subprojects (one per build manifest) — the deterministic discovery the
    /// orchestrator splits work by. Kept in the model's stable order.
    pub projects: Vec<ProjectUnit>,
    /// Distinct declaration names (entities/types/functions), sorted + deduped —
    /// the "known entities" set (answers "is X new or already in the repo?").
    pub entities: Vec<String>,
}

/// Project the model down to its orchestrator FACTS. Deterministic: `projects`
/// keep the model's stable order; `entities` are sorted + deduped.
#[must_use]
pub fn build(model: &ProjectModel) -> ModelFacts {
    let mut entities: Vec<String> = model
        .modules
        .iter()
        .flat_map(|m| m.declarations.iter().map(|d| d.name.clone()))
        .filter(|n| !n.is_empty())
        .collect();
    entities.sort();
    entities.dedup();
    ModelFacts { projects: model.projects.clone(), entities }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Decl, Module};

    fn model_with(decls: &[&str], projects: &[&str]) -> ProjectModel {
        ProjectModel {
            modules: vec![Module {
                declarations: decls.iter().map(|n| Decl { name: (*n).to_string(), ..Default::default() }).collect(),
                ..Default::default()
            }],
            projects: projects.iter().map(|n| ProjectUnit { name: (*n).to_string(), ..Default::default() }).collect(),
            ..Default::default()
        }
    }

    #[test]
    fn entities_are_sorted_deduped_and_nonempty() {
        let f = build(&model_with(&["User", "Invoice", "User", ""], &[]));
        assert_eq!(f.entities, vec!["Invoice", "User"]);
    }

    #[test]
    fn projects_preserve_model_order() {
        let f = build(&model_with(&[], &["api", "web"]));
        let names: Vec<String> = f.projects.iter().map(|p| p.name.clone()).collect();
        assert_eq!(names, vec!["api", "web"]);
    }
}
