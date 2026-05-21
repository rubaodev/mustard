//! `mustard-rt run memory cross-wave` — render a markdown summary of
//! `agent.memory` events captured by the waves that ran before the current one.
//!
//! Part of the wave-network spec (`2026-05-20-mustard-wave-network-standard`):
//! the SKILL `/feature` (and `/resume`) embeds the rendered markdown into the
//! agent prompt of wave N so the agent inherits context from waves 1..N-1
//! without re-reading their spec files.
//!
//! The wave names for the spec come from `<spec-dir>/wave-plan.md`'s
//! `## Tabela de Waves` markdown table — the `Spec` column carries the
//! wikilink `[[wave-N-{role}]]` from which we strip the brackets to get the
//! exact `pipeline` value stored in events.
//!
//! Output: markdown only (stdout). Empty string when there are no prior waves
//! or no captured memory rows for them. Exit 0 always (fail-open).

use crate::run::env::project_dir;
use mustard_core::store::sqlite_store::SqliteEventStore;
use rusqlite::{Connection, params};
use serde_json::Value;
use std::path::{Path, PathBuf};

/// At most this many memory rows per prior wave land in the rendered block —
/// keeps the embedded context bounded.
const MAX_MEMORIES_PER_WAVE: usize = 5;

/// Strip surrounding `[[`/`]]` (and whitespace) from a wikilink token. Returns
/// `None` when the token does not look like a wikilink.
fn strip_wikilink(raw: &str) -> Option<String> {
    let t = raw.trim();
    let inner = t.strip_prefix("[[").and_then(|s| s.strip_suffix("]]"))?;
    let inner = inner.trim();
    if inner.is_empty() {
        return None;
    }
    Some(inner.to_string())
}

/// Parse the wave-plan markdown table and return the ordered wave names (the
/// `Spec` column, wikilinks stripped).
///
/// Recognises rows whose first cell parses as a wave number (`1`, `W1`,
/// `Wave 1`, …) — mirrors `wave_tree::parse_wave_plan` for consistency, but
/// returns the *Spec* column instead of the folder column.
pub(crate) fn parse_wave_names(wave_plan_text: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for raw_line in wave_plan_text.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        let Some(rest) = line.strip_prefix('|') else {
            continue;
        };
        let body = rest;
        let cells: Vec<&str> = body.split('|').map(str::trim).collect();
        // Expect at minimum: label | Spec | ... (separator rows are filtered
        // below by the label-cell shape check).
        if cells.len() < 2 {
            continue;
        }
        let label = cells[0].to_lowercase();
        // Skip header & separator rows.
        let label_body = label
            .strip_prefix('w')
            .map(str::trim_start)
            .unwrap_or(&label);
        let label_body = label_body
            .strip_prefix("ave")
            .map(str::trim_start)
            .unwrap_or(label_body);
        if label_body.is_empty() || !label_body.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        // The Spec column is the next cell (cells[1]). Strip `[[wave-N-...]]`.
        if let Some(name) = strip_wikilink(cells[1]) {
            out.push(name);
        }
    }
    out
}

/// Open a fresh rusqlite [`Connection`] pointing at the project store —
/// reuses the store's resolved DB path. The `SqliteEventStore` itself is
/// dropped immediately; only the path is borrowed.
fn open_conn(project: &Path) -> Option<Connection> {
    let store = SqliteEventStore::for_project(project).ok()?;
    let db_path = store.path().to_path_buf();
    let conn = Connection::open(&db_path).ok()?;
    let _ = conn.busy_timeout(std::time::Duration::from_millis(5_000));
    Some(conn)
}

/// Fetch up to [`MAX_MEMORIES_PER_WAVE`] `agent.memory` payloads for a single
/// wave name, newest first. A wave name is the value persisted in
/// `payload.pipeline` by `memory agent` writers.
pub(crate) fn memories_for_wave(conn: &Connection, wave_name: &str) -> Vec<Value> {
    let mut stmt = match conn.prepare(
        "SELECT payload FROM events \
         WHERE event = 'agent.memory' \
           AND json_extract(payload, '$.pipeline') = ?1 \
         ORDER BY ts DESC \
         LIMIT ?2",
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    let rows = match stmt.query_map(
        params![wave_name, MAX_MEMORIES_PER_WAVE as i64],
        |row| {
            let text: Option<String> = row.get(0)?;
            Ok(text
                .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                .unwrap_or(Value::Null))
        },
    ) {
        Ok(it) => it,
        Err(_) => return Vec::new(),
    };
    rows.filter_map(std::result::Result::ok)
        .filter(|v| !v.is_null())
        .collect()
}

/// Render the prior-wave memories block. Returns the empty string when there
/// are no prior waves or no memory rows for any of them.
pub(crate) fn render(wave_names: &[String], conn: Option<&Connection>) -> String {
    if wave_names.is_empty() {
        return String::new();
    }
    let Some(conn) = conn else {
        return String::new();
    };
    let mut sections: Vec<String> = Vec::new();
    for name in wave_names {
        let mems = memories_for_wave(conn, name);
        if mems.is_empty() {
            continue;
        }
        let mut block = String::new();
        block.push_str(&format!("### [[{name}]]\n"));
        for m in mems {
            // Prefer `summary`, fall back to a compact JSON line.
            if let Some(s) = m.get("summary").and_then(Value::as_str) {
                if !s.is_empty() {
                    block.push_str(&format!("- {s}\n"));
                    continue;
                }
            }
            let compact = serde_json::to_string(&m).unwrap_or_default();
            if !compact.is_empty() {
                block.push_str(&format!("- {compact}\n"));
            }
        }
        sections.push(block);
    }
    if sections.is_empty() {
        return String::new();
    }
    let mut out = String::from("## Memórias de waves anteriores\n\n");
    out.push_str(&sections.join("\n"));
    out
}

/// Run `mustard-rt run memory cross-wave --spec <name> --wave <N>`.
///
/// Fail-open: a missing wave-plan, missing DB, or unparseable `--wave` all
/// degrade to an empty stdout body.
pub fn run(spec: Option<&str>, wave: Option<u32>) {
    let Some(spec) = spec else {
        eprintln!("Usage: memory cross-wave --spec <name> --wave <N>");
        return;
    };
    let Some(wave) = wave else {
        eprintln!("Usage: memory cross-wave --spec <name> --wave <N>");
        return;
    };
    if wave <= 1 {
        // Wave 1 has no prior waves — empty block.
        return;
    }

    let project = PathBuf::from(project_dir());
    let plan_path = project
        .join(".claude")
        .join("spec")
        .join(spec)
        .join("wave-plan.md");

    let plan_text = std::fs::read_to_string(&plan_path).unwrap_or_default();
    let all_names = parse_wave_names(&plan_text);
    // Keep waves 1..N-1 (the first N-1 entries).
    let n_prior = (wave as usize).saturating_sub(1).min(all_names.len());
    let prior: Vec<String> = all_names.into_iter().take(n_prior).collect();

    let conn = open_conn(&project);
    let rendered = render(&prior, conn.as_ref());
    if !rendered.is_empty() {
        print!("{rendered}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mustard_core::model::event::{Actor, ActorKind, HarnessEvent, SCHEMA_VERSION};
    use mustard_core::store::event_store::EventSink;
    use serde_json::json;
    use tempfile::tempdir;

    fn mem_event(pipeline: &str, summary: &str) -> HarnessEvent {
        HarnessEvent {
            v: SCHEMA_VERSION,
            ts: "2026-05-20T10:00:00.000Z".to_string(),
            session_id: "s-test".to_string(),
            wave: 0,
            actor: Actor {
                kind: ActorKind::Agent,
                id: Some("test".to_string()),
                actor_type: None,
            },
            event: "agent.memory".to_string(),
            payload: json!({ "pipeline": pipeline, "summary": summary }),
            spec: None,
        }
    }

    #[test]
    fn parses_wave_names_from_table() {
        let plan = "\
| Wave | Spec                          | Role    |
|------|-------------------------------|---------|
| 1    | [[wave-1-rt-infra]]           | general |
| 2    | [[wave-2-skill-template]]     | general |
| 3    | [[wave-3-dashboard-graph]]    | frontend|
";
        let names = parse_wave_names(plan);
        assert_eq!(
            names,
            vec![
                "wave-1-rt-infra".to_string(),
                "wave-2-skill-template".to_string(),
                "wave-3-dashboard-graph".to_string(),
            ]
        );
    }

    #[test]
    fn strip_wikilink_rejects_non_wikilinks() {
        assert!(strip_wikilink("plain").is_none());
        assert!(strip_wikilink("[[]]").is_none());
        assert_eq!(strip_wikilink("  [[abc]] ").as_deref(), Some("abc"));
    }

    #[test]
    fn reads_prior_waves() {
        let dir = tempdir().unwrap();
        let store = SqliteEventStore::new(dir.path().join("mustard.db")).unwrap();
        // Two memories for wave-1, one for wave-2.
        store
            .append(&mem_event("wave-1-rt-infra", "rt infra delivered four subcommands"))
            .unwrap();
        store
            .append(&mem_event("wave-1-rt-infra", "wikilinks table created"))
            .unwrap();
        store
            .append(&mem_event("wave-2-skill-template", "SKILLs updated"))
            .unwrap();

        let conn = Connection::open(store.path()).unwrap();
        let prior = vec![
            "wave-1-rt-infra".to_string(),
            "wave-2-skill-template".to_string(),
        ];
        let md = render(&prior, Some(&conn));
        assert!(md.starts_with("## Memórias de waves anteriores"));
        assert!(md.contains("### [[wave-1-rt-infra]]"));
        assert!(md.contains("### [[wave-2-skill-template]]"));
        assert!(md.contains("rt infra delivered four subcommands"));
        assert!(md.contains("wikilinks table created"));
        assert!(md.contains("SKILLs updated"));
    }

    #[test]
    fn render_empty_when_no_prior_waves() {
        let dir = tempdir().unwrap();
        let store = SqliteEventStore::new(dir.path().join("mustard.db")).unwrap();
        let conn = Connection::open(store.path()).unwrap();
        let md = render(&[], Some(&conn));
        assert!(md.is_empty());
    }

    #[test]
    fn render_empty_when_no_memories_match() {
        let dir = tempdir().unwrap();
        let store = SqliteEventStore::new(dir.path().join("mustard.db")).unwrap();
        let conn = Connection::open(store.path()).unwrap();
        let md = render(&["wave-1-rt-infra".to_string()], Some(&conn));
        assert!(md.is_empty());
    }
}
