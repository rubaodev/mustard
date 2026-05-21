// Wave-3 spec-view types — mirrors spec_views.rs shapes.
// Field names are snake_case to match Rust serde output directly.

export interface SpecCard {
  spec: string;
  status: string;
  phase: string;
  scope: string | null;
  started_at: string | null;
  last_event_at: string | null;
  duration_ms: number | null;
  current_wave: number | null;
  total_waves: number | null;
  ac_passed: number;
  ac_total: number;
  files_touched: number;
  tools_used: number;
  model: string | null;
  /**
   * Number of sub-specs linked to this spec via `spec.link` events. Populated
   * by the `spec_card_v2` adapter so the dashboard's `SpecCard` component can
   * render the `+N sub-specs` badge without fanning out one `useSpecChildren`
   * query per rendered card (spec `2026-05-21-speccard-use-children-count`).
   * Optional for backwards compatibility with payloads emitted before the
   * field existed.
   */
  children_count?: number;
}

/**
 * One sub-spec row linked to a parent. Mirrors `spec_views::SpecChild` on
 * the Rust side. Wave-3 (2026-05-20, spec
 * `2026-05-20-tactical-fix-via-sub-spec`) introduced the shape; Wave-6
 * (2026-05-21, spec `2026-05-21-dashboard-spec-tabs`) added `source` to
 * surface whether the row was discovered via the SQLite `spec.link` event,
 * the filesystem `### Parent:` header, or both.
 */
export interface SpecChild {
  spec: string;
  /** kebab-case lifecycle status (no-events | planning | implementing | …). */
  status: string;
  started_at: string | null;
  completed_at: string | null;
  reason: string | null;
  /**
   * Provenance of this row, surfaced by the Wave-6 union scanner:
   * - `"event"` — found only in the SQLite `spec.link` projection
   * - `"header"` — found only via the on-disk `### Parent:` header scan
   * - `"both"` — present in both sources (the normal case once the event
   *    store has caught up)
   *
   * Optional for backwards compatibility — payloads from the pre-Wave-6
   * Tauri command never populate this field.
   */
  source?: "event" | "header" | "both";
  /**
   * Wave 2 (spec `2026-05-21-dashboard-spec-tabs-polish`): the parent wave
   * whose execution window contains this child's `started_at`. `null` /
   * `undefined` when the child has no `started_at` (header-only) or its
   * start falls outside every wave window. Rendered as a nested row under
   * the matching wave in the Ondas tab; missing-wave children land in the
   * "Sem onda correlacionada" bucket.
   */
  wave?: number | null;
}

/** Wave-3 — sub-spec summary attached to a `SpecSummary` row. Optional for
 *  backwards compatibility with responses produced before this field existed. */
export interface SpecSummary {
  spec: string;
  status: string;
  /** Number of sub-specs linked to this spec via `spec.link` events. */
  children_count?: number;
}

export interface SpecWave {
  wave: number;
  role: string | null;
  /** queued | in_progress | completed | failed */
  status: string;
  started_at: string | null;
  completed_at: string | null;
  agent_type: string | null;
  files_changed: number;
}

export interface SpecQualityItem {
  ac_id: string;
  ac_label: string | null;
  /** pass | fail | skip | unknown */
  status: string;
  wave: number | null;
  command: string | null;
  last_run_at: string | null;
  fail_reason: string | null;
}

export interface SpecTimelineNode {
  ts: string;
  /** phase | wave | qa | review | agent | tool | other */
  kind: string;
  label: string;
  phase: string | null;
  wave: number | null;
  payload_summary: string | null;
}

export interface TimelineEvent {
  id: string;
  ts: string;
  phase: string | null;
  spec: string | null;
  agent: string | null;
  summary: string;
}

export interface EventFilter {
  kinds?: string[];
  wave?: number;
  agent?: string;
  q?: string;
}

/** Matches SpecActionKind enum on Rust side (sent as string). */
export type SpecActionKind = "reopen" | "close" | "remove";

export interface SpecAction {
  action: string;
  spec: string;
  result: string;
  message: string | null;
}

export interface PhaseSegment {
  /** analyze | plan | execute | qa | close */
  phase: string;
  /** completed | active | future */
  state: string;
}

export interface SpecTrack {
  spec: string;
  status: string;
  current_phase: string;
  current_wave: number | null;
  total_waves: number | null;
  agents_active: number;
  last_event_at: string | null;
  blocked_reason: string | null;
  segments: PhaseSegment[];
}

export interface WorkspaceAlert {
  /** wave_failed | qa_fail */
  kind: string;
  spec: string;
  wave: number | null;
  message: string;
  ts: string | null;
}

export interface FileCount {
  path: string;
  count: number;
}

export interface WorkspaceSummary {
  events_per_minute: number;
  /** `null` when token-savings data is unavailable — render "—" not "0". */
  tokens_saved_today: number | null;
  specs_active_count: number;
  spec_tracks: SpecTrack[];
  alerts: WorkspaceAlert[];
  top_files_today: FileCount[];
}

/** ContributionCell — reserved for future heatmap grid (spec §259). */
export interface ContributionCell {
  date: string;
  count: number;
}
