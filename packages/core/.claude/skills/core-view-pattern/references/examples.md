<!-- mustard:generated -->
# view — examples in this codebase

<!-- mustard:enrich hash=275c3b08d6d3 -->
## Purpose

These files are the typed view models other crates render against. `filter.rs` holds the query arguments — `TimeWindow`, `SpecStatusFilter`, and the composite `SpecFilter` — kept tiny and serializable so a Tauri command can accept them straight from the React frontend. `quality.rs` defines the acceptance-criteria shapes: `AcStatus` (pass/fail/skip/pending, with synonym-tolerant parsing), `AcceptanceCriterion`, and the `QualityRollup` hero counts. `mod.rs` wires the sub-modules together and owns the cross-cutting `Phase` (analyze → plan → execute → qa → close) and `Scope` (full/light/touch) enums that several views share, each with a fail-open `parse`.
<!-- /mustard:enrich -->

- Ref: packages/core/src/domain/model/view/filter.rs
- Ref: packages/core/src/domain/model/view/mod.rs
- Ref: packages/core/src/domain/model/view/quality.rs

