//! Integration tests for spec 2026-05-20-restore-rtk-rewrite — AC-3, AC-4, AC-5,
//! plus dual-coverage sibling tests for spec 2026-05-21-rtk-rewrite-dual-coverage
//! (warn vs strict mode emitted by `bash_guard`).
//!
//! These drive the `mustard-rt` binary via subprocess (no shell quoting) so the
//! AC commands are deterministic across Windows cmd.exe and POSIX shells.

use std::io::Write;
use std::process::{Command, Stdio};

use rusqlite::Connection;
use serde_json::Value;
use tempfile::TempDir;

/// Returns `true` when the `rtk` binary is reachable on `PATH`.
fn rtk_available() -> bool {
    Command::new("rtk")
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Drive `mustard-rt on PreToolUse` under the requested gate mode.
/// `mode` accepts "warn" / "strict" / "off" (the values
/// `MUSTARD_RTK_GATE_MODE` understands — this is the env var the
/// `bash_guard` rtk-rewrite gate actually reads; the original sub-spec
/// referenced `MUSTARD_BASH_REDIRECT_MODE` which does not exist in the
/// codebase, both names are documented here so future greps land).
fn run_hook_with_mode(tmp: &TempDir, command: &str, mode: &str) -> (std::path::PathBuf, String) {
    let db_path = tmp.path().join("mustard.db");
    let hook_input = serde_json::json!({
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "cwd": tmp.path().to_str().expect("tempdir path utf-8"),
        "session_id": "test-rtk-mode",
        "tool_input": { "command": command }
    });
    let payload = serde_json::to_string(&hook_input).expect("serialize hook input");
    let mut child = Command::new(env!("CARGO_BIN_EXE_mustard-rt"))
        .args(["on", "PreToolUse"])
        .env("MUSTARD_DB_PATH", &db_path)
        .env("MUSTARD_RTK_GATE_MODE", mode)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn mustard-rt");
    child
        .stdin
        .take()
        .expect("stdin pipe")
        .write_all(payload.as_bytes())
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait mustard-rt");
    assert!(
        output.status.success(),
        "mustard-rt exited {}: stderr={}",
        output.status,
        String::from_utf8_lossy(&output.stderr),
    );
    (db_path, String::from_utf8_lossy(&output.stdout).into_owned())
}

/// Drive `mustard-rt on PreToolUse` as a subprocess, feeding it a `Bash`
/// hook-input JSON on stdin.  Returns the DB path where events should land.
///
/// Thin wrapper around [`run_hook_with_mode`] pinned to `warn` so the existing
/// AC-3 / AC-4 / AC-5 tests keep observing the rewrite path (the gate default
/// is `strict`, which would otherwise deny instead of rewriting).
fn run_hook(tmp: &TempDir, command: &str) -> std::path::PathBuf {
    let (db_path, _stdout) = run_hook_with_mode(tmp, command, "warn");
    db_path
}

/// Variant of `run_hook` that also returns the hook's stdout, used by AC-3/AC-4
/// to inspect the `updatedInput` rewrite or confirm a silent pass-through.
///
/// Thin wrapper around [`run_hook_with_mode`] pinned to `warn`.
fn run_hook_capture(tmp: &TempDir, command: &str) -> (std::path::PathBuf, String) {
    run_hook_with_mode(tmp, command, "warn")
}

/// AC-3: a raw command without `rtk` prefix that has an RTK equivalent must
/// produce a `Verdict::Rewrite` encoded as `updatedInput` in the hook's stdout.
#[test]
fn rtk_rewrite_e2e_rewrites_unprefixed_command() {
    if !rtk_available() {
        eprintln!("rtk not on PATH — skipping rtk_rewrite_e2e_rewrites_unprefixed_command");
        return;
    }
    let tmp = TempDir::new().expect("create tempdir");
    let (_db, stdout) = run_hook_capture(&tmp, "git status");
    let parsed: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("parse hook stdout JSON: {e}; raw={stdout:?}"));
    let updated = parsed
        .pointer("/hookSpecificOutput/updatedInput/command")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no updatedInput.command in response: {stdout}"));
    assert!(
        updated.starts_with("rtk "),
        "rewritten command must start with 'rtk ', got {updated:?}"
    );
}

/// AC-4: a command already prefixed with `rtk` short-circuits the rewrite path.
/// The hook responds with a silent allow (empty stdout).
#[test]
fn rtk_rewrite_e2e_passes_through_rtk_prefixed_command() {
    let tmp = TempDir::new().expect("create tempdir");
    // No rtk-presence check needed — this path never invokes the subprocess.
    let (_db, stdout) = run_hook_capture(&tmp, "rtk git status");
    assert!(
        stdout.trim().is_empty(),
        "rtk-prefixed command must produce silent allow; got stdout={stdout:?}"
    );
}

/// AC-5: when `bash_guard` rewrites a `Bash` command via `rtk`, an `rtk-rewrite`
/// event must be persisted in the SQLite event store.
///
/// The test is skipped gracefully when `rtk` is absent from PATH — printing a
/// message and returning early still counts as 1 passed for `cargo test`.
/// When `rtk` is present the assertion is hard: a missing event is a regression.
#[test]
fn rtk_rewrite_emission() {
    if !rtk_available() {
        eprintln!("rtk not on PATH — skipping rtk_rewrite_emission");
        return;
    }

    let tmp = TempDir::new().expect("create tempdir");

    // `git status` is a canonical command that `rtk` rewrites to `rtk git status`.
    let db_path = run_hook(&tmp, "git status");

    // The store file must exist — the hook creates it on first write.
    assert!(
        db_path.exists(),
        "SQLite DB was not created at {}",
        db_path.display()
    );

    let conn = Connection::open(&db_path)
        .unwrap_or_else(|e| panic!("open DB at {}: {e}", db_path.display()));

    let count: i64 = conn
        .query_row(
            "SELECT count(*) FROM events WHERE event = 'rtk-rewrite'",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|e| panic!("query events: {e}"));

    assert!(
        count >= 1,
        "expected at least 1 rtk-rewrite event in SQLite, found {count}"
    );

    // Bonus: confirm the stored payload references the original command head.
    let command_head: String = conn
        .query_row(
            "SELECT json_extract(payload, '$.command_head') \
             FROM events WHERE event = 'rtk-rewrite' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or_else(|e| panic!("query command_head: {e}"));

    assert!(
        command_head.contains("git"),
        "command_head should reference 'git', got: {command_head:?}"
    );
}

// -----------------------------------------------------------------------------
// Dual-coverage sibling tests (spec 2026-05-21-rtk-rewrite-dual-coverage)
//
// The default gate mode is `strict`: instead of rewriting the command via
// `updatedInput`, the gate denies and surfaces the rewrite suggestion through
// `permissionDecisionReason`.  These tests pin the strict path so a regression
// in either mode is caught.
// -----------------------------------------------------------------------------

/// Strict mode must deny an unprefixed command and surface the rewrite rule in
/// `permissionDecisionReason`.
#[test]
fn rtk_rewrite_strict_denies_unprefixed_command() {
    let tmp = TempDir::new().expect("create tempdir");
    let (_db, stdout) = run_hook_with_mode(&tmp, "git status", "strict");
    let parsed: Value = serde_json::from_str(stdout.trim())
        .unwrap_or_else(|e| panic!("parse hook stdout JSON: {e}; raw={stdout:?}"));
    let reason = parsed
        .pointer("/hookSpecificOutput/permissionDecisionReason")
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("no permissionDecisionReason in response: {stdout}"));
    assert!(
        reason.contains("Reenvie como: rtk"),
        "strict mode must surface the rule; got {reason:?}"
    );
}

/// Strict mode must still pass `rtk`-prefixed commands through as a silent
/// allow — the gate only fires on unprefixed candidates.
#[test]
fn rtk_rewrite_strict_passes_through_rtk_prefixed() {
    let tmp = TempDir::new().expect("create tempdir");
    let (_db, stdout) = run_hook_with_mode(&tmp, "rtk git status", "strict");
    assert!(
        stdout.trim().is_empty(),
        "rtk-prefixed must produce silent allow even in strict; got {stdout:?}"
    );
}

/// Strict mode denies before the rewrite path runs, so no `rtk-rewrite`
/// telemetry event should ever land in the store.
#[test]
fn rtk_rewrite_strict_emits_no_rewrite_event() {
    if !rtk_available() {
        eprintln!("rtk not on PATH — skipping rtk_rewrite_strict_emits_no_rewrite_event");
        return;
    }
    let tmp = TempDir::new().expect("create tempdir");
    let (db_path, _stdout) = run_hook_with_mode(&tmp, "git status", "strict");
    // In strict mode, the gate denies before any rewrite — the SQLite event
    // store should not carry an `rtk-rewrite` event for this run.
    if db_path.exists() {
        let conn = Connection::open(&db_path).expect("open sqlite");
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM events WHERE event = 'rtk-rewrite'",
                [],
                |row| row.get(0),
            )
            .unwrap_or(0);
        assert_eq!(count, 0, "strict mode must NOT emit rtk-rewrite events");
    }
}
