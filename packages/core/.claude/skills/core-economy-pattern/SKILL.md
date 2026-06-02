---
name: core-economy-pattern
description: Use when adding or refactoring an `economy` module that records token cost or savings signals as pure serde records over the NDJSON event channel
tags: [add, refactor]
appliesTo: [economy]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: economy
---

<!-- mustard:generated -->
# economy pattern

<!-- mustard:enrich hash=eec2a63a88f4 -->
## Purpose

The `economy` cluster is the single source of truth for every cost and savings signal in `mustard-core`, split by responsibility: `model.rs` holds the pure `serde` records (spans, savings, context-cost frames) and aggregate roll-ups with money carried as drift-free `i64` micro-USD; `estimator.rs` wraps `tiktoken-rs` for approximate token counts plus the cache-aware pricing table that converts the four Anthropic billing buckets into a cost; `mod.rs` re-exports the submodule surface so consumers import from one place. The records carry a flattened `extra` map so external adapters can add fields without reshaping the core types.
<!-- /mustard:enrich -->

## Convention

- Folder: `src/domain/economy/`
- Extension: `.rs`
- Files: 7

## How to apply

These files are grouped by location under `src/domain/economy/`. Add another `.rs` file in the same folder.

## Examples

- Ref: packages/core/src/domain/economy/estimator.rs
- Ref: packages/core/src/domain/economy/mod.rs
- Ref: packages/core/src/domain/economy/model.rs

## Shape

Declares: 11 struct_item, 1 enum_item, 1 type_item.

## References

See `references/examples.md`.
