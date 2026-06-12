//! Local git inspection for the dashboard project overview card.
//!
//! [`dashboard_git_info`] shells out to the local `git` binary inside the
//! selected repository and projects a few read-only facts the overview card
//! renders: the `origin` remote URL, the current branch, the ahead/behind
//! counts against its upstream, and the last commit (hash, message, author,
//! ISO date).
//!
//! FAIL-OPEN CONTRACT (mirrors every dashboard command): a missing repository,
//! a missing `git` binary, a detached HEAD, or a missing remote/upstream never
//! surfaces as an `Err` toast — each sub-probe degrades to an empty field so
//! the card shows an empty state instead. The command only returns `Ok`.
//!
//! WINDOWS-INVISIBLE INVOCATION: every spawn goes through
//! [`crate::process_util::no_window_command`], which sets `CREATE_NO_WINDOW`
//! on Windows so packaged users never see a console flash.

use crate::process_util::no_window_command;
use serde::Serialize;
use std::path::Path;

/// Read-only snapshot of a repository's git state. Every field defaults to its
/// empty form so a non-repo / no-remote path renders as an empty-state card.
#[derive(Serialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct GitInfo {
    /// `true` when `repo_path` is inside a git work tree.
    pub is_repo: bool,
    /// URL of the `origin` remote, empty when there is no remote.
    pub remote_url: String,
    /// Current branch name, empty on a detached HEAD or non-repo.
    pub branch: String,
    /// Commits ahead of the upstream (0 when no upstream is configured).
    pub ahead: u32,
    /// Commits behind the upstream (0 when no upstream is configured).
    pub behind: u32,
    /// Abbreviated hash of the last commit, empty when the repo has no commits.
    pub last_commit_hash: String,
    /// Subject line of the last commit.
    pub last_commit_message: String,
    /// Author name of the last commit.
    pub last_commit_author: String,
    /// Author date of the last commit, ISO-8601 (`%cI`), empty when absent.
    pub last_commit_date: String,
}

/// Run `git <args>` in `repo_path` and return trimmed stdout, or `None` when
/// the spawn fails or git exits non-zero. The fail-open primitive every probe
/// below is built on — an error is indistinguishable from "no data here".
fn git_capture(repo_path: &Path, args: &[&str]) -> Option<String> {
    let output = no_window_command("git")
        .args(args)
        .current_dir(repo_path)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Inspect the local git state of `repo_path`. Always returns `Ok`; absent data
/// (no repo, no remote, no upstream, no commits) yields empty fields, never an
/// error — the overview card renders the empty state instead of a toast.
#[tauri::command]
pub async fn dashboard_git_info(repo_path: String) -> Result<GitInfo, String> {
    // A join error (panic in the closure) degrades to an empty overview, never
    // an Err toast — the failure-tolerant contract.
    let info = tauri::async_runtime::spawn_blocking(move || git_info_impl(&repo_path))
        .await
        .unwrap_or_default();
    Ok(info)
}

/// Synchronous body of [`dashboard_git_info`], kept separate so unit tests call
/// it directly without a Tauri runtime.
fn git_info_impl(repo_path: &str) -> GitInfo {
    let base = Path::new(repo_path);
    let mut info = GitInfo::default();

    // Gate every other probe on being inside a work tree. `rev-parse
    // --is-inside-work-tree` prints `true` on success; anything else (not a
    // repo, git missing) leaves the empty default in place.
    let is_repo = git_capture(base, &["rev-parse", "--is-inside-work-tree"])
        .map(|s| s == "true")
        .unwrap_or(false);
    if !is_repo {
        return info;
    }
    info.is_repo = true;

    // Remote URL — `origin` only; absent remote leaves the field empty.
    if let Some(url) = git_capture(base, &["remote", "get-url", "origin"]) {
        info.remote_url = url;
    }

    // Current branch. `HEAD` on a detached checkout is treated as no branch.
    if let Some(branch) = git_capture(base, &["rev-parse", "--abbrev-ref", "HEAD"]) {
        if branch != "HEAD" {
            info.branch = branch;
        }
    }

    // Ahead/behind vs the upstream. `@{upstream}` resolves only when one is
    // configured; the whole probe is skipped (counts stay 0) otherwise. Output
    // is "<behind>\t<ahead>" with --left-right against `@{u}...HEAD`.
    if let Some(counts) = git_capture(
        base,
        &["rev-list", "--left-right", "--count", "@{upstream}...HEAD"],
    ) {
        let mut parts = counts.split_whitespace();
        if let Some(behind) = parts.next().and_then(|s| s.parse::<u32>().ok()) {
            info.behind = behind;
        }
        if let Some(ahead) = parts.next().and_then(|s| s.parse::<u32>().ok()) {
            info.ahead = ahead;
        }
    }

    // Last commit, one field per format token so values that contain the
    // separator (commit subjects do) never split wrong.
    if let Some(hash) = git_capture(base, &["log", "-1", "--format=%h"]) {
        info.last_commit_hash = hash;
    }
    if let Some(message) = git_capture(base, &["log", "-1", "--format=%s"]) {
        info.last_commit_message = message;
    }
    if let Some(author) = git_capture(base, &["log", "-1", "--format=%an"]) {
        info.last_commit_author = author;
    }
    if let Some(date) = git_capture(base, &["log", "-1", "--format=%cI"]) {
        info.last_commit_date = date;
    }

    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_repo_path_returns_empty_state() {
        let dir = tempfile::tempdir().unwrap();
        let info = git_info_impl(&dir.path().to_string_lossy());
        assert!(!info.is_repo);
        assert!(info.remote_url.is_empty());
        assert!(info.branch.is_empty());
        assert_eq!(info.ahead, 0);
        assert_eq!(info.behind, 0);
        assert!(info.last_commit_hash.is_empty());
    }

    #[test]
    fn repo_without_remote_reports_branch_and_commit() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path();
        let run = |args: &[&str]| {
            no_window_command("git")
                .args(args)
                .current_dir(base)
                .output()
        };
        // Skip when git is unavailable on the host — fail-open contract.
        if run(&["init", "-b", "trunk"]).is_err() {
            return;
        }
        let _ = run(&["config", "user.email", "qa@example.com"]);
        let _ = run(&["config", "user.name", "QA Bot"]);
        std::fs::write(base.join("a.txt"), b"hello").unwrap();
        let _ = run(&["add", "."]);
        let _ = run(&["commit", "-m", "initial commit"]);

        let info = git_info_impl(&base.to_string_lossy());
        assert!(info.is_repo);
        assert!(info.remote_url.is_empty(), "no remote configured");
        assert_eq!(info.branch, "trunk");
        assert!(!info.last_commit_hash.is_empty());
        assert_eq!(info.last_commit_message, "initial commit");
        assert_eq!(info.last_commit_author, "QA Bot");
        assert!(!info.last_commit_date.is_empty());
    }
}
