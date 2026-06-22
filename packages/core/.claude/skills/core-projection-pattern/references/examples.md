<!-- mustard:generated -->
# projection — examples in this codebase

<!-- mustard:enrich hash=bafd6eda6877 -->
## Purpose

These files are deterministic event-stream folds. `card.rs` is `project_spec_view` — it folds the events scoped to one spec into a `SpecView`, applying each `pipeline.*` / `qa.result` / `tool.use` / `agent.start` event in chronological order; when no events exist it falls back to the `meta.json` sidecar (single source of truth) and, for un-migrated specs, the legacy `spec.md` header. `quality.rs` is `project_quality` — it folds `qa.result` events into a `QualityRollup`, keeping the most recent entry per AC id and counting the pass/fail/skip/pending buckets. `mod.rs` documents the projection contract (total, deterministic, IO-free) and provides the NDJSON loaders that feed the folds in production.
<!-- /mustard:enrich -->

- Ref: packages/core/src/view/projection/card.rs
- Ref: packages/core/src/view/projection/mod.rs
- Ref: packages/core/src/view/projection/quality.rs

