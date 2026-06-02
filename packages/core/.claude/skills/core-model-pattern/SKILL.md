---
name: core-model-pattern
description: Use when adding or refactoring a `model` module of pure side-effect-free serde types shared across hooks, scripts, and the CLI
tags: [add, refactor]
appliesTo: [model]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: model
---

<!-- mustard:generated -->
# model pattern

<!-- mustard:enrich hash=b54976550b2b -->
## Purpose

The `model` cluster is the crate's pure data layer: every type is a plain `serde` struct or enum with no I/O, no filesystem access, and no logging — side-effecting infrastructure lives elsewhere. It owns the harness event schema, the SDD view models, and the frozen hook contract in `contract.rs` (`HookInput`, `Verdict`, `Outcome`, `Trigger`, and the `Check`/`Observer` traits) that every hook consumer is built on. The contract makes illegal states unrepresentable and is lenient at the harness boundary so new fields never break a parse.
<!-- /mustard:enrich -->

## Convention

- Folder: `src/domain/model/`
- Extension: `.rs`
- Files: 5

## How to apply

These files are grouped by location under `src/domain/model/`. Add another `.rs` file in the same folder.

## Examples

- Ref: packages/core/src/domain/model/contract.rs
- Ref: packages/core/src/domain/model/event.rs
- Ref: packages/core/src/domain/model/mod.rs

## Shape

Declares: 19 struct_item, 3 enum_item, 2 trait_item, 1 type_item.

## References

See `references/examples.md`.
