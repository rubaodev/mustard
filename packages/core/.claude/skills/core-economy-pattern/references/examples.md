<!-- mustard:generated -->
# economy — examples in this codebase

<!-- mustard:enrich hash=07b06eef1cc1 -->
## Purpose

These files implement the economy domain. `estimator.rs` wraps the `tiktoken-rs` cl100k singleton for approximate input/output token counts and holds the per-model pricing table; `compute_cost_micros` prices a frame cache-aware across the four Anthropic buckets (fresh input, cache creation at 1.25x, cache read at 0.10x, output) in floatless `i64` micro-USD. `model.rs` defines the pure side-effect-free records — `SpanRecord`, `SavingsRecord`, `ContextCostFrame`, the `SavingsSource` enum, and the aggregate roll-ups the dashboard reads. `mod.rs` documents the model/scope/writer/reader/estimator split and re-exports the names consumers use.
<!-- /mustard:enrich -->

- Ref: packages/core/src/domain/economy/estimator.rs
- Ref: packages/core/src/domain/economy/mod.rs
- Ref: packages/core/src/domain/economy/model.rs

