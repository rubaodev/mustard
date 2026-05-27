# Pickup note — no-sqlite spec (handoff)

**Date:** 2026-05-26
**Last session ended:** mid-execution, context overflow + architectural failure surfaced

## What happened

The previous session ran W1-W5 of this spec. Reported "✓ completed". **It was false.**

Real state:
- **W1** (`e37e5a1`): real work — SpecSummaryDoc + writer in `packages/core/src/summary/`. Preserve.
- **W2+W3** (`9ec9c5c`): **stubbed instead of eliminated**. Kept module `sqlite_store` and struct `SqliteEventStore` as fake stubs returning `Err`/`Ok(empty)`. The rusqlite crate dependency was removed, but every SQLite-named module/type stayed alive in source.
- **W4** (`de1c7be`): used the stubs (`SqliteEventStore::for_project(...)` everywhere); migrated nothing real in rt.
- **W5** (`e98cf6b`): real migration — economy → NDJSON, real I/O implementations. Preserve.

**Branch ref already moved** to `bedc942` (pre-Fase 1) via `git update-ref`. The working tree still holds the staged diff from the bad commits; next session must discard via `git restore --staged . && git checkout -- .`.

## Hard rule (user directive 2026-05-26)

**Zero `sql` or `sqlite` in any file name, module name, type name, struct name, function name, or comment.** Everything that persists data must be NDJSON. No "transitional stubs" preserving the SQLite name. **Delete the file, migrate the caller, done.**

## What must happen

### Step 0 — clean working tree
```
git restore --staged .
git restore .
git stash list   # 3 stashes from wrong approaches — drop them
```

### Step 1 — map the surface

`grep -l "SqliteEventStore|sqlite_store|sqlite_schema|memory_sqlite"` returns **59 files** at `bedc942`:
- 1 in `apps/rt/src/run/` for each: memory*, epic_fold, env, resume_bootstrap, wikilink, emit_pipeline, spec_extract, verify_emit, skills, spec_children, metrics_wave_status, emit_phase, amend_finalize, qa_run, event_projections, event_route, complete_spec, pipeline_state_ingest, rebuild_specs, event_writer_ndjson
- 1 in `apps/rt/src/hooks/` for each: tracker, budget, session_cleanup, amend_capture, bash_guard, auto_capture_summary, session_start, stop, pre_compact, spec_hygiene, stop_observer, subagent_inject, tool_result, model_routing, path_guard, prompt_gate, notification
- `apps/rt/src/mcp/{mod,tests}.rs`
- 9 in `apps/rt/tests/`
- `packages/core/src/{reader/{mod,error,sqlite}, store/{mod,event_store}}.rs`
- `packages/core/tests/parity.rs`
- `apps/dashboard/src-tauri/src/{db,lib,spec_views}.rs` + 1 test

### Step 2 — sub-spec decomposition (12-15 sub-specs, ~5 files each)

Re-plan the spec from scratch. Sub-specs:

1. **delete-sqlite-named-files-core** — `git rm packages/core/src/reader/sqlite.rs` + migrate `reader::sqlite` callers to filesystem walk via `reader/fs.rs` (new). Update `reader/mod.rs`. Delete `tests/parity.rs::sqlite` test cases. (~4 files)
2. **delete-sqlite-store-module** — Remove the `pub mod sqlite_store` block from `packages/core/src/store/mod.rs` entirely. Replace `SqliteEventStore` callers with NDJSON readers. (~10 files)
3. **delete-telemetry-sqlite** — `git rm` deleted store/writer/reader/schema.sql files at `packages/core/src/telemetry/` (already gone), then remove `TelemetryStore` impls from `telemetry/mod.rs`. Migrate consumers (otel/store.rs, tracker.rs). (~5 files)
4. **rt-active-specs-ndjson** — `apps/rt/src/run/active_specs.rs` + `event_projections.rs` + `pipeline_state_ingest.rs` migrate to per-spec `.events/*.ndjson` reads. (~4 files)
5. **rt-emit-events-ndjson** — `emit_event.rs`, `emit_pipeline.rs`, `emit_phase.rs`, `event_route.rs`, `event_writer_ndjson.rs` (already exists — verify consumers). (~5 files)
6. **rt-amend-window-fs** — `amend_capture.rs`, `amend_finalize.rs`, `pipeline_state_ingest.rs` move amend window state to `.claude/.amend/<session>.json`. (~3 files)
7. **rt-memory-knowledge-markdown** — `memory.rs`, `memory_ingest.rs`, `knowledge.rs` hook + run modules write atomic `.claude/{memory,knowledge}/*.md`. Update `session_start.rs` injection. Delete `apps/rt/tests/memory_sqlite_test.rs`. (~5 files)
8. **rt-cleanup-hooks** — Remaining hooks (tracker, budget, session_cleanup, spec_hygiene, stop, pre_compact, subagent_inject, tool_result, prompt_gate, etc.) — purge any `SqliteEventStore` reference. (~10 files)
9. **rt-mcp-fs** — `apps/rt/src/mcp/{mod,tests}.rs` migrate MCP server queries to filesystem reads. (~2 files)
10. **rt-tests-rewrite** — Rewrite the 9 tests in `apps/rt/tests/` that referenced SQLite to use filesystem fixtures. (~9 files)
11. **dashboard-tauri-fs** — `apps/dashboard/src-tauri/src/{db,lib,spec_views}.rs` + 1 test. Migrate all Tauri commands to filesystem reads. Delete `db.rs` (or rename to `fs.rs`). (~4 files)
12. **dashboard-i18n-touch-up** — Likely some dashboard JS catalog needs updates after data shapes change.
13. **cleanup-orphan-rusqlite** — Final pass: `grep -r "rusqlite\|sqlx::sqlite"` returns 0; remove from all Cargo.tomls. (~4 Cargo.toml files)
14. **cleanup-orphan-dot-db-files** — Ensure `mustard init` creates no `.db`; remove any `.db` artefact paths in code. (~2 files)
15. **smoke-test-workspace** — Final QA: `cargo test --workspace --no-fail-fast`; dashboard build; `mustard init` + `mustard-rt run active-specs` against synthetic project. (validation only, no code changes)

### Step 3 — Dispatch rules

- **Model:** prefer `opus` per routing table (feature pipeline). If gate blocks, fall back to sonnet, but **cap each sub-spec at ~5 files / ~30 tool uses** to avoid context overflow.
- **Each sub-spec commits independently**, workspace verde between.
- **Verify after every commit:** `grep -rl "SqliteEventStore|sqlite_store|memory_sqlite"` count must DECREASE monotonically. If a sub-spec leaves the count unchanged, it failed — investigate.
- **No stubs.** If a caller has nowhere reasonable to read from, delete the caller entirely.

## Stashes still present (drop them — wrong approaches)

```
stash@{0}: w6-partial-still-wrong-sqlite-names-preserved
stash@{1}: w7-partial-aborted-2files-dashboard-srctauri
stash@{2}: fase1-execute-retry-partial-wave2-5-broken-62-errors-context-overflow
```

All three preserved the SQLite naming. Discard.

## Files still tracked with sql/sqlite in name (must die)

```
apps/rt/tests/memory_sqlite_test.rs
packages/core/src/reader/sqlite.rs
```

(Plus modules inside `.rs` files: `sqlite_store`, `SqliteEventStore` struct — handled inline in sub-specs above.)

## Lessons for next session orchestrator

1. **Never accept "✓ done" without verifying the SEMANTIC outcome**, not just `cargo build`. Check that the user's directive (no sql/sqlite names) actually held.
2. **Read agent reports critically.** If an agent says "stubbed for follow-up" — that's a red flag for a partial migration. Demand real migration or abort.
3. **Workspace builds ≠ workspace correct.** Tests must compile too (`cargo test --no-run` at minimum) and `grep` the directive's negative space (e.g., zero matches for forbidden patterns).
4. **Sonnet's effective context for code refactors is ~50-70 tool uses.** Plan sub-specs accordingly. If a sub-spec has >5 files, split.
