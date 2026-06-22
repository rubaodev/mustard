//! `mustard-rt run otel-stop` — stop the local OTEL collector for this project.
//!
//! The counterpart to the `SessionStart` spawn in
//! [`crate::hooks::session::session_start_inject`]. A reinstall (`install.ps1`)
//! drops a fresh `mustard-rt.exe` on disk, but a collector daemon spawned from
//! the *previous* build keeps an exclusive file lock on the binary (Windows),
//! stranding the next `cargo build`. This command reaps that daemon before the
//! build, so a reinstall stops leaving the previous collector holding the
//! binary.
//!
//! ## Why kill by PORT, not by the PID file
//!
//! The per-project `.otel-collector.pid` file DRIFTS from the live listener:
//! measured in the field with the file holding `47228` while the live collector
//! was `42140` (a respawn that failed to rewrite the file, a crash, a takeover).
//! So killing by the PID file is unreliable. The reliable path is to resolve the
//! port the collector binds (`MUSTARD_OTEL_PORT`, default 4318 — never a literal
//! here) and kill whatever is *listening* on it, exactly as the SessionStart
//! port-takeover does. The stale PID file is then deleted so the next spawn does
//! not trip over it.
//!
//! ## Fail-open contract
//!
//! Every step is best-effort: a missing `netstat`/`lsof`/`kill` on PATH, an
//! unresolvable project dir, or an unremovable PID file degrades to the printed
//! status line — never a panic, never a non-zero exit. Telemetry teardown must
//! never abort an install or a session.

use super::collector::resolve_port;
use crate::shared::context::project_dir;
use crate::shared::proc::free_port;
use mustard_core::io::fs;
use mustard_core::ClaudePaths;
use std::path::PathBuf;

/// File where the spawned OTEL collector records its PID, under the project's
/// harness directory. Mirrors `session_start_inject::OTEL_PID_FILE` — the same
/// path that hook writes and `session_cleanup` removes on `SessionEnd`.
const OTEL_PID_FILE: &str = ".otel-collector.pid";

/// Stop the local OTEL collector: kill the listener(s) on the resolved OTLP
/// port and delete the stale PID file. Fully fail-open; prints one status line.
pub fn run() {
    let port = resolve_port();
    let killed = free_port(port);
    let pid_file_removed = remove_pid_file();

    // One concise human line: port, pids killed, pid-file disposition.
    let pids = if killed.is_empty() {
        "none".to_string()
    } else {
        killed
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",")
    };
    let pid_file = if pid_file_removed {
        "removed"
    } else {
        "absent/unchanged"
    };
    println!(
        "otel-stop: port {port} — killed listener pid(s): {pids}; pid-file: {pid_file}"
    );
}

/// Delete `<project>/.claude/.harness/.otel-collector.pid`. Returns `true` only
/// when a file existed and was removed; `false` for absent or unremovable.
/// Fail-open: any IO error degrades to `false`.
fn remove_pid_file() -> bool {
    let Some(pid_path) = pid_file_path() else {
        return false;
    };
    if !pid_path.exists() {
        return false;
    }
    match fs::remove_file(&pid_path) {
        Ok(()) => true,
        Err(e) => {
            eprintln!("otel-stop: remove pid file failed ({e})");
            false
        }
    }
}

/// `<project>/.claude/.harness/.otel-collector.pid`, or `None` when the project
/// `.claude` boundary cannot be resolved (the I1 guard rejected the path).
fn pid_file_path() -> Option<PathBuf> {
    ClaudePaths::for_project(project_dir())
        .ok()
        .map(|p| p.harness_dir().join(OTEL_PID_FILE))
}
