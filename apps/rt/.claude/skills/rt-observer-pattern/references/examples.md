<!-- mustard:generated -->
# observer — examples in this codebase

<!-- mustard:enrich hash=9379945204fb -->
## Purpose

Worked examples of the `observer` convention. `agent_summary_observer.rs` fires on `PostToolUse(Task)`, scans the subagent output for a `<MEMORY>` block or `Resumo:`/`Summary:` line, and persists it as a markdown agent-memory row. `memory_promote_observer.rs` runs at `SessionEnd`, promoting memory rows with confidence `>= 0.85` into permanent decision/lesson files and flipping the source to `promoted`. `notification_observer.rs` reacts to `Notification` by appending a `notification.received` event to the per-spec NDJSON log. All three are observe-only and fail-open.
<!-- /mustard:enrich -->

- Ref: apps/rt/src/hooks/observe/agent_summary_observer.rs
- Ref: apps/rt/src/hooks/observe/memory_promote_observer.rs
- Ref: apps/rt/src/hooks/observe/notification_observer.rs

