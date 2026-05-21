//! Integration tests for the W4 economy attribution join.
//!
//! Each test seeds spans and `agent.start` events into a fresh harness
//! database, then asserts the reader's `per_agent_costs`, `per_spec_costs`,
//! and `per_wave_costs` aggregations resolve the right (agent, spec, wave)
//! triple via the spans↔events join (primary `tool_use_id` key, fallback
//! temporal window keyed on `session_id` + `ts`).

use mustard_core::economy::{
    EconomyScope, SpanRecord, per_agent_costs, per_spec_costs, per_wave_costs, record_span,
};
use mustard_core::economy::scope::{ProjectPath, SpecId, WaveId};
use mustard_core::store::sqlite_store::SqliteEventStore;
use rusqlite::Connection;
use serde_json::{Map, Value, json};
use tempfile::tempdir;

fn open_conn(dir: &std::path::Path) -> Connection {
    let _store = SqliteEventStore::new(dir.join(".claude/.harness/mustard.db")).unwrap();
    Connection::open(dir.join(".claude/.harness/mustard.db")).unwrap()
}

/// Seed one `agent.start` event row directly — bypasses the hook path so the
/// test can place arbitrary `(spec, wave, payload)` triples without spinning
/// up the apps/rt observer machinery.
#[allow(clippy::too_many_arguments)]
fn seed_agent_start(
    conn: &Connection,
    ts: &str,
    session_id: Option<&str>,
    spec: Option<&str>,
    wave: i64,
    actor_id: Option<&str>,
    payload: Value,
) {
    conn.execute(
        "INSERT INTO events(ts, session_id, wave, spec, event, actor_kind, actor_id, payload) \
         VALUES(?1, ?2, ?3, ?4, 'agent.start', 'hook', ?5, ?6)",
        rusqlite::params![ts, session_id, wave, spec, actor_id, payload.to_string()],
    )
    .unwrap();
}

/// Span with `tool_use_id` stashed in `extra` — the writer pulls it into the
/// `spans.tool_use_id` column for the primary join leg.
fn span_with_tool_use(
    ts: &str,
    span_id: &str,
    session_id: &str,
    tool_use_id: Option<&str>,
    cost: i64,
    tokens: i64,
) -> SpanRecord {
    let mut extra = Map::new();
    if let Some(t) = tool_use_id {
        extra.insert("tool_use_id".into(), Value::String(t.into()));
    }
    SpanRecord {
        ts: ts.into(),
        session_id: Some(session_id.into()),
        span_id: span_id.into(),
        model: Some("claude-3-5-sonnet".into()),
        spec: None, // attribution comes from the agent.start, not the span
        phase: None,
        input_tokens: Some(tokens),
        output_tokens: Some(0),
        cache_read_input_tokens: None,
        cache_creation_input_tokens: None,
        cost_usd_micros: Some(cost),
        is_error: false,
        extra,
    }
}

#[test]
fn test_tool_use_id_join_primary() {
    let dir = tempdir().unwrap();
    let conn = open_conn(dir.path());

    seed_agent_start(
        &conn,
        "2026-05-21T10:00:00.000Z",
        Some("sess-1"),
        Some("spec-A"),
        1,
        Some("orchestrator"),
        json!({
            "agent_id": "core-impl",
            "spec_id": "spec-A",
            "wave_id": "wave-1",
            "tool_use_id": "toolu_PRIMARY",
        }),
    );
    record_span(
        &conn,
        span_with_tool_use(
            "2026-05-21T10:00:30.000Z",
            "req-1",
            "sess-1",
            Some("toolu_PRIMARY"),
            10_000,
            500,
        ),
    )
    .unwrap();

    let rows =
        per_agent_costs(&conn, EconomyScope::Project(ProjectPath::new(dir.path()))).unwrap();
    assert_eq!(rows.len(), 1, "expected one attributed agent");
    assert_eq!(rows[0].agent_id.as_str(), "core-impl");
    assert_eq!(rows[0].cost_usd_micros, 10_000);
    assert_eq!(rows[0].tokens, 500);
    assert_eq!(rows[0].span_count, 1);
}

#[test]
fn test_temporal_window_fallback() {
    let dir = tempdir().unwrap();
    let conn = open_conn(dir.path());

    // agent.start at t=10:00, span at t=10:05 — no tool_use_id on either side,
    // so the temporal fallback (`session_id` + `ts <=`) must catch it.
    seed_agent_start(
        &conn,
        "2026-05-21T10:00:00.000Z",
        Some("sess-fallback"),
        Some("spec-F"),
        2,
        Some("orchestrator"),
        json!({
            "agent_id": "core-explore",
            "spec_id": "spec-F",
            "wave_id": "wave-2",
        }),
    );
    record_span(
        &conn,
        span_with_tool_use(
            "2026-05-21T10:05:00.000Z",
            "req-fb",
            "sess-fallback",
            None,
            7_500,
            300,
        ),
    )
    .unwrap();

    let rows =
        per_agent_costs(&conn, EconomyScope::Project(ProjectPath::new(dir.path()))).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].agent_id.as_str(), "core-explore");
    assert_eq!(rows[0].cost_usd_micros, 7_500);
    assert_eq!(rows[0].span_count, 1);
}

#[test]
fn test_empty_all_projects() {
    // AC-4: AllProjects scope with zero entries returns Vec empty, no error.
    let scope = EconomyScope::AllProjects(vec![]);
    // No conn needed for the empty-fan-out path — the function bypasses SQL.
    // We still need *a* connection to satisfy the signature; build a fresh
    // throwaway DB so the call is realistic end-to-end.
    let dir = tempdir().unwrap();
    let conn = open_conn(dir.path());
    let rows = per_agent_costs(&conn, scope).unwrap();
    assert!(rows.is_empty(), "empty AllProjects must return Vec::new()");

    let rows = per_spec_costs(
        &conn,
        EconomyScope::AllProjects(vec![]),
    )
    .unwrap();
    assert!(rows.is_empty());

    let rows = per_wave_costs(
        &conn,
        EconomyScope::AllProjects(vec![]),
    )
    .unwrap();
    assert!(rows.is_empty());
}

#[test]
fn test_per_spec_aggregation() {
    let dir = tempdir().unwrap();
    let conn = open_conn(dir.path());

    // Two agent.starts in two different specs.
    seed_agent_start(
        &conn,
        "2026-05-21T09:00:00.000Z",
        Some("sess-a"),
        Some("spec-X"),
        1,
        Some("orchestrator"),
        json!({"agent_id": "agent-x", "spec_id": "spec-X", "wave_id": "w1",
               "tool_use_id": "toolu_X"}),
    );
    seed_agent_start(
        &conn,
        "2026-05-21T09:30:00.000Z",
        Some("sess-b"),
        Some("spec-Y"),
        1,
        Some("orchestrator"),
        json!({"agent_id": "agent-y", "spec_id": "spec-Y", "wave_id": "w1",
               "tool_use_id": "toolu_Y"}),
    );

    record_span(
        &conn,
        span_with_tool_use("2026-05-21T09:01:00.000Z", "r1", "sess-a", Some("toolu_X"), 1_000, 100),
    )
    .unwrap();
    record_span(
        &conn,
        span_with_tool_use("2026-05-21T09:02:00.000Z", "r2", "sess-a", Some("toolu_X"), 2_000, 200),
    )
    .unwrap();
    record_span(
        &conn,
        span_with_tool_use("2026-05-21T09:31:00.000Z", "r3", "sess-b", Some("toolu_Y"), 5_000, 500),
    )
    .unwrap();

    let scope = EconomyScope::Project(ProjectPath::new(dir.path()));
    let by_spec = per_spec_costs(&conn, scope).unwrap();
    assert_eq!(by_spec.len(), 2, "expected one row per spec");
    // Sorted DESC by cost — spec-Y first (5_000), spec-X second (3_000).
    assert_eq!(by_spec[0].spec_id.as_str(), "spec-Y");
    assert_eq!(by_spec[0].cost_usd_micros, 5_000);
    assert_eq!(by_spec[0].span_count, 1);
    assert_eq!(by_spec[1].spec_id.as_str(), "spec-X");
    assert_eq!(by_spec[1].cost_usd_micros, 3_000);
    assert_eq!(by_spec[1].span_count, 2);
}

#[test]
fn test_per_wave_aggregation() {
    let dir = tempdir().unwrap();
    let conn = open_conn(dir.path());

    // Same spec, two waves — verifies (spec, wave) grouping splits correctly.
    seed_agent_start(
        &conn,
        "2026-05-21T08:00:00.000Z",
        Some("sess-w"),
        Some("spec-W"),
        1,
        Some("orchestrator"),
        json!({"agent_id": "core-impl", "spec_id": "spec-W", "wave_id": "wave-alpha",
               "tool_use_id": "toolu_A"}),
    );
    seed_agent_start(
        &conn,
        "2026-05-21T08:30:00.000Z",
        Some("sess-w"),
        Some("spec-W"),
        2,
        Some("orchestrator"),
        json!({"agent_id": "core-impl", "spec_id": "spec-W", "wave_id": "wave-beta",
               "tool_use_id": "toolu_B"}),
    );

    record_span(
        &conn,
        span_with_tool_use("2026-05-21T08:01:00.000Z", "r1", "sess-w", Some("toolu_A"), 1_200, 120),
    )
    .unwrap();
    record_span(
        &conn,
        span_with_tool_use("2026-05-21T08:31:00.000Z", "r2", "sess-w", Some("toolu_B"), 3_400, 340),
    )
    .unwrap();

    let scope = EconomyScope::Project(ProjectPath::new(dir.path()));
    let by_wave = per_wave_costs(&conn, scope).unwrap();
    assert_eq!(by_wave.len(), 2, "expected one row per (spec, wave) pair");

    // Sorted DESC by cost: wave-beta (3_400) first, wave-alpha (1_200) second.
    assert_eq!(by_wave[0].spec_id.as_str(), "spec-W");
    assert_eq!(by_wave[0].wave_id.as_str(), "wave-beta");
    assert_eq!(by_wave[0].cost_usd_micros, 3_400);
    assert_eq!(by_wave[1].wave_id.as_str(), "wave-alpha");
    assert_eq!(by_wave[1].cost_usd_micros, 1_200);

    // Wave-scoped filter narrows to a single wave's roll-up.
    let wave_scope = EconomyScope::Wave {
        project: ProjectPath::new(dir.path()),
        spec: SpecId::new("spec-W"),
        wave: WaveId::new("wave-alpha"),
    };
    let scoped = per_wave_costs(&conn, wave_scope).unwrap();
    assert_eq!(scoped.len(), 1);
    assert_eq!(scoped[0].wave_id.as_str(), "wave-alpha");
    assert_eq!(scoped[0].cost_usd_micros, 1_200);
}

/// Regression test for AC-6 — the literal name is grepped by the AC binary.
///
/// Models the scenario the superseded `metrics-writers-pipeline-key` spec
/// described: an `agent.start` is emitted on a parent spec (because the
/// orchestrator session is rooted there) but it carries a `wave_id` for a
/// *child* wave that the parent dispatched into. The W4 join must follow the
/// payload's `wave_id`, not the event row's `spec`, so the attributed roll-up
/// surfaces the child wave correctly.
#[test]
fn test_parent_spec_child_wave_attribution() {
    let dir = tempdir().unwrap();
    let conn = open_conn(dir.path());

    // The event row's top-level `spec` column is the parent; the payload's
    // `spec_id` + `wave_id` point at the child wave the agent was dispatched
    // against. The attribution CTE prefers the payload over the row column.
    seed_agent_start(
        &conn,
        "2026-05-21T12:00:00.000Z",
        Some("sess-parent"),
        Some("parent-spec"),
        0,
        Some("orchestrator"),
        json!({
            "agent_id": "core-impl",
            "spec_id": "parent-spec",
            "wave_id": "child-wave",
            "tool_use_id": "toolu_PARENT_CHILD",
        }),
    );

    record_span(
        &conn,
        span_with_tool_use(
            "2026-05-21T12:00:15.000Z",
            "req-pc",
            "sess-parent",
            Some("toolu_PARENT_CHILD"),
            8_888,
            444,
        ),
    )
    .unwrap();

    let scope = EconomyScope::Project(ProjectPath::new(dir.path()));
    let by_wave = per_wave_costs(&conn, scope.clone()).unwrap();
    assert_eq!(by_wave.len(), 1, "one (spec, wave) attribution expected");
    assert_eq!(by_wave[0].spec_id.as_str(), "parent-spec");
    assert_eq!(
        by_wave[0].wave_id.as_str(),
        "child-wave",
        "agent.start payload wave_id must drive the wave attribution"
    );

    let by_spec = per_spec_costs(&conn, scope.clone()).unwrap();
    assert_eq!(by_spec.len(), 1);
    assert_eq!(by_spec[0].spec_id.as_str(), "parent-spec");
    assert_eq!(by_spec[0].cost_usd_micros, 8_888);

    let by_agent = per_agent_costs(&conn, scope).unwrap();
    assert_eq!(by_agent.len(), 1);
    assert_eq!(by_agent[0].agent_id.as_str(), "core-impl");
    assert_eq!(by_agent[0].cost_usd_micros, 8_888);
}
