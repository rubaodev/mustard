//! Telemetry aggregation shapes.
//!
//! Onda 1 (spec `dashboard-sqlite-out-telemetria-ndjson`): the dead SQLite
//! `db.rs` facade was deleted. The aggregator functions that took a
//! `&db::Connection` were reachable only through the retired `with_db` gate,
//! so they are gone; the `dashboard_telemetry_*` commands in `lib.rs` now
//! resolve to their fail-open empty/default payloads directly. Only the JSON
//! shapes consumed by those commands' return types remain here. Faithful
//! NDJSON-backed reimplementations land in Onda 2 (the dedicated telemetry
//! rewrite).

use serde::{Deserialize, Serialize};

// ── Shapes ──────────────────────────────────────────────────────────────────

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct PhaseSummary {
    pub phase: String,
    pub events_count: i64,
    pub last_event_at: Option<String>,
    /// Event counts per day, last 7 days (oldest first, 7 slots).
    pub sparkline: Vec<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct TimelineEvent {
    pub id: String,
    pub ts: String,
    pub phase: Option<String>,
    pub spec: Option<String>,
    pub agent: Option<String>,
    pub summary: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct HeatmapCell {
    /// 0 = Sunday … 6 = Saturday.
    pub day_of_week: i64,
    /// 0–23
    pub hour: i64,
    pub event_count: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct HistoryEntry {
    pub spec: String,
    pub status: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    /// phase label → cumulative event count for that phase
    pub duration_per_phase: std::collections::HashMap<String, i64>,
    pub ac_passed: i64,
    pub ac_total: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AcceptanceCriterion {
    pub spec: String,
    pub id: String,
    pub status: String,
    pub last_run_at: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct FileCount {
    pub path: String,
    pub count: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct ToolUseCount {
    pub name: String,
    pub count: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct PhaseEventCount {
    pub phase: String,
    pub duration_ms: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AgentTypeCount {
    pub agent_type: String,
    pub count: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "snake_case")]
pub struct EffortBreakdown {
    pub top_files: Vec<FileCount>,
    pub top_tools: Vec<ToolUseCount>,
    pub top_phases: Vec<PhaseEventCount>,
    pub top_agents: Vec<AgentTypeCount>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub struct AgentDispatch {
    pub subagent_type: String,
    pub count: i64,
    pub error_count: i64,
    pub avg_duration_ms: i64,
    pub last_dispatched_at: Option<String>,
}

// Onda 1: the `telemetry_*(&Connection, ...)` aggregators were removed — they
// were dead behind the retired SQLite `with_db` gate. The `dashboard_telemetry_*`
// Tauri commands in `lib.rs` return the empty/default payloads directly.
