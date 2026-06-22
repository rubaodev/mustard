---
name: core-projection-pattern
description: Use when adding or refactoring a `projection` that folds a slice of harness events into a typed view model deterministically
tags: [add, refactor]
appliesTo: [projection]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: projection
---

<!-- mustard:generated -->
# projection pattern

<!-- mustard:enrich hash=af020c6d89af -->
## Purpose

The `projection` cluster holds the pure folds over `&[HarnessEvent]` — one function per view model. Each projection is total (always returns something) and deterministic (same input, same output): it never reads the filesystem, never touches the event store, and never panics, which is what lets the crate be tested by seeding a `Vec` and asserting the result. `card.rs` folds a spec's event stream into a `SpecView` (with a `meta.json`/header fallback when the stream is empty); `quality.rs` rolls `qa.result` events into a `QualityRollup`, keeping the latest entry per Acceptance Criterion.
<!-- /mustard:enrich -->

## Convention

- Folder: `src/view/projection/`
- Extension: `.rs`
- Files: 6

## How to apply

These files are grouped by location under `src/view/projection/`. Add another `.rs` file in the same folder.

## Examples

- Ref: packages/core/src/view/projection/card.rs
- Ref: packages/core/src/view/projection/mod.rs
- Ref: packages/core/src/view/projection/quality.rs

## References

See `references/examples.md`.
