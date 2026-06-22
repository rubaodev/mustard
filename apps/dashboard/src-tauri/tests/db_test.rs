//! Onda 1 (spec `dashboard-sqlite-out-telemetria-ndjson`): the dead SQLite
//! `db.rs` facade was deleted. The Wave-6B variant of this suite exercised
//! `db::with_db` (which always returned `None`); with the facade gone, only
//! the shape contract of the public summary structs remains to assert.

use mustard_dashboard_lib::{KnowledgeSummary, MetricsSummary};

#[test]
fn metrics_summary_zeroed_shape() {
    let m = MetricsSummary {
        total_events: 0,
        sessions_recent: 0,
        agents_dispatched: 0,
        last_event_at: None,
        tokens_total: 0,
        tokens_today: 0,
    };
    assert_eq!(m.total_events, 0);
    assert_eq!(m.tokens_total, 0);
    assert!(m.last_event_at.is_none());
}

#[test]
fn knowledge_summary_zeroed_shape() {
    let k = KnowledgeSummary {
        patterns_count: 0,
        conventions_count: 0,
        high_confidence_count: 0,
    };
    assert_eq!(k.patterns_count + k.conventions_count + k.high_confidence_count, 0);
}
