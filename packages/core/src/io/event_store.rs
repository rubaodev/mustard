//! The harness event log — append and replay of `.claude/.harness/events.jsonl`.
//!
//! The log is an append-only NDJSON stream: one JSON-encoded [`HarnessEvent`]
//! per line, newline-terminated. The JS emitter (`_lib/harness-event.js`)
//! writes it with `fs.appendFileSync(file, JSON.stringify(line) + '\n')`;
//! [`JsonlEventStore::append`] reproduces exactly that — one event, one line,
//! a trailing `\n`.
//!
//! Consumers depend on the [`EventSink`] **trait**, not on the concrete store,
//! so a test (or a hook running with telemetry disabled) can inject a fake
//! sink. [`JsonlEventStore`] is the filesystem-backed implementation a binary
//! instantiates.
//!
//! Replay is fail-open: a missing log replays as an empty `Vec`, and a
//! corrupt line is skipped rather than aborting the whole replay. A single
//! truncated trailing line — common when a process is killed mid-append —
//! must never lose the events written before it.

use crate::error::Result;
use crate::io::fs;
use crate::model::event::HarnessEvent;
use std::path::{Path, PathBuf};

/// Directory name of the harness event store, under `.claude/`.
const HARNESS_DIR: &str = ".harness";

/// File name of the NDJSON event log.
const EVENTS_FILE: &str = "events.jsonl";

/// A destination that accepts harness events.
///
/// The trait is the API consumers and the B3 dispatcher program against.
/// Implementations must fail open: an [`EventSink::append`] that fails
/// returns [`Err`] rather than panicking, and a caller is free to ignore it
/// (telemetry is never load-bearing).
pub trait EventSink {
    /// Append one event to the sink.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`](crate::error::Error) if the event could not be
    /// persisted (serialization or I/O failure).
    fn append(&self, event: &HarnessEvent) -> Result<()>;
}

/// Filesystem-backed [`EventSink`] over a single `events.jsonl` file.
///
/// Construct it with [`JsonlEventStore::new`] from an explicit path, or with
/// [`JsonlEventStore::for_project`] from a project root (which resolves
/// `.claude/.harness/events.jsonl`). Cloning is cheap — it only holds a path.
#[derive(Debug, Clone)]
pub struct JsonlEventStore {
    path: PathBuf,
}

impl JsonlEventStore {
    /// Create a store backed by the file at `path`.
    ///
    /// The file and its parent directory are created lazily on the first
    /// [`append`](EventSink::append); construction never touches the disk.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Create a store for the standard event log of a project.
    ///
    /// Resolves `{project_dir}/.claude/.harness/events.jsonl`, matching the
    /// path the JS emitter uses.
    #[must_use]
    pub fn for_project(project_dir: impl AsRef<Path>) -> Self {
        let path = project_dir
            .as_ref()
            .join(".claude")
            .join(HARNESS_DIR)
            .join(EVENTS_FILE);
        Self { path }
    }

    /// The absolute path of the backing event log.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Replay the whole log into a `Vec` of events.
    ///
    /// Fail-open by design:
    ///
    /// - a log file that does not exist replays as an empty `Vec`, not an
    ///   error — an unstarted project simply has no events;
    /// - a line that fails to parse (corrupt or truncated) is skipped, so a
    ///   single bad trailing line never discards the valid events before it.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`](crate::error::Error) only for a genuine I/O
    /// failure while reading the file (permissions, etc.) — never for absence
    /// or for malformed content.
    pub fn replay(&self) -> Result<Vec<HarnessEvent>> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            // A missing log is not an error: there is simply nothing to replay.
            Err(crate::error::Error::NotFound(_)) => return Ok(Vec::new()),
            Err(err) => return Err(err),
        };
        Ok(parse_lines(&text))
    }
}

/// Parse NDJSON text into events, skipping blank and unparsable lines.
fn parse_lines(text: &str) -> Vec<HarnessEvent> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .filter_map(|line| serde_json::from_str::<HarnessEvent>(line).ok())
        .collect()
}

impl EventSink for JsonlEventStore {
    fn append(&self, event: &HarnessEvent) -> Result<()> {
        // One event → one compact JSON line. `append_line` adds the `\n`.
        let line = serde_json::to_string(event)?;
        fs::append_line(&self.path, &line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::event::{Actor, ActorKind, SCHEMA_VERSION};
    use serde_json::json;
    use std::cell::RefCell;
    use tempfile::tempdir;

    fn sample_event(name: &str) -> HarnessEvent {
        HarnessEvent {
            v: SCHEMA_VERSION,
            ts: "2026-05-19T00:00:00.000Z".to_string(),
            session_id: "s-test".to_string(),
            wave: 0,
            actor: Actor {
                kind: ActorKind::Hook,
                id: Some("event-store-test".to_string()),
                actor_type: None,
            },
            event: name.to_string(),
            payload: json!({"k": "v"}),
            spec: None,
        }
    }

    #[test]
    fn append_then_replay_round_trips() {
        let dir = tempdir().unwrap();
        let store = JsonlEventStore::new(dir.path().join("events.jsonl"));
        store.append(&sample_event("session.start")).unwrap();
        store.append(&sample_event("tool.use")).unwrap();

        let events = store.replay().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event, "session.start");
        assert_eq!(events[1].event, "tool.use");
        assert_eq!(events[1].payload, json!({"k": "v"}));
    }

    #[test]
    fn replay_missing_file_is_empty_not_error() {
        let dir = tempdir().unwrap();
        let store = JsonlEventStore::new(dir.path().join("never-written.jsonl"));
        let events = store.replay().unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn replay_skips_corrupt_lines() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let store = JsonlEventStore::new(&path);
        store.append(&sample_event("good.one")).unwrap();
        // A garbage line and a truncated line between two valid events.
        fs::append_line(&path, "{not json at all").unwrap();
        fs::append_line(&path, r#"{"v":1,"ts":"x"  "#).unwrap();
        store.append(&sample_event("good.two")).unwrap();

        let events = store.replay().unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event, "good.one");
        assert_eq!(events[1].event, "good.two");
    }

    #[test]
    fn for_project_resolves_standard_path() {
        let store = JsonlEventStore::for_project("/proj");
        assert!(store.path().ends_with("events.jsonl"));
        assert!(
            store
                .path()
                .components()
                .any(|c| c.as_os_str() == ".harness")
        );
    }

    #[test]
    fn each_appended_line_is_newline_terminated() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("events.jsonl");
        let store = JsonlEventStore::new(&path);
        store.append(&sample_event("a")).unwrap();
        store.append(&sample_event("b")).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        // Two events → two lines, each ending in `\n`, no torn lines.
        assert_eq!(text.lines().count(), 2);
        assert!(text.ends_with('\n'));
    }

    /// A fake [`EventSink`] proves the trait is what consumers depend on:
    /// a test can collect events in memory with no filesystem at all.
    #[test]
    fn trait_supports_an_in_memory_fake() {
        struct FakeSink {
            collected: RefCell<Vec<String>>,
        }
        impl EventSink for FakeSink {
            fn append(&self, event: &HarnessEvent) -> Result<()> {
                self.collected.borrow_mut().push(event.event.clone());
                Ok(())
            }
        }

        let fake = FakeSink {
            collected: RefCell::new(Vec::new()),
        };
        fake.append(&sample_event("decision")).unwrap();
        assert_eq!(fake.collected.borrow().as_slice(), ["decision"]);
    }
}
