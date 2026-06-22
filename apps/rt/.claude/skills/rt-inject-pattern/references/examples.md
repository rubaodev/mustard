<!-- mustard:generated -->
# inject — examples in this codebase

<!-- mustard:enrich hash=629b3129ee7d -->
## Purpose

Worked examples of the `inject` convention. `context_inject.rs` is the shared helper that matches `memory/*.md` principle files against a dispatch intent (Aho-Corasick over morphological name stems) and renders the `## SPEC MEMORY` and regression-vocabulary blocks. `amend_window_inject.rs` is a dual `Observer`/`Check` that forecasts amendment-window drift and injects a post-CLOSE warning when out-of-scope edits cross the threshold. `pre_compact_memory_inject.rs` injects up to three recent active agent-memory summaries on `PreCompact` so they survive the compaction.
<!-- /mustard:enrich -->

- Ref: apps/rt/src/commands/agent/context_inject.rs
- Ref: apps/rt/src/hooks/observe/amend_window_inject.rs
- Ref: apps/rt/src/hooks/observe/pre_compact_memory_inject.rs

