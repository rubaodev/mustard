<!-- mustard:generated -->
# model — examples in this codebase

<!-- mustard:enrich hash=bd567374e06e -->
## Purpose

These files define the pure data model shared across hooks, scripts, and the CLI. `contract.rs` is the frozen hook contract: `Trigger` (the lifecycle event), `HookInput` (the lenient stdin JSON), `Verdict` (a single decision — Allow/Deny/Warn/Rewrite/Inject, with illegal states unrepresentable), `Outcome` (the consolidated fold where Deny dominates), plus the `Check` (may block) and `Observer` (telemetry-only) traits that enforce interface segregation. `mod.rs` documents the submodule split (event, contract, pipeline, provenance, view) and re-exports the view types consumers import directly.
<!-- /mustard:enrich -->

- Ref: packages/core/src/domain/model/contract.rs
- Ref: packages/core/src/domain/model/event.rs
- Ref: packages/core/src/domain/model/mod.rs

