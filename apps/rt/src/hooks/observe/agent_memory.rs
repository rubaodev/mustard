//! Shared path helper for the agent-memory observers.
//!
//! [`super::subagent_stop_observer`] and [`super::memory_promote_observer`]
//! both walk `.claude/memory/agent/*.md`. The directory resolution is the one
//! thing they share — it lives here so neither re-implements it.

use mustard_core::ClaudePaths;
use std::path::{Path, PathBuf};

/// `.claude/memory/agent` for a project, or `None` when the path cannot be
/// resolved.
pub(crate) fn agent_dir(cwd: &str) -> Option<PathBuf> {
    ClaudePaths::for_project(Path::new(cwd))
        .ok()
        .map(|p| p.claude_dir().join("memory").join("agent"))
}
