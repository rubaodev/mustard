---
name: core-view-pattern
description: Use when adding or refactoring a `view` model — a typed serde shape other crates render against, plus its parsing of free-form strings
tags: [add, refactor]
appliesTo: [view]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: view
---

<!-- mustard:generated -->
# view pattern

<!-- mustard:enrich hash=5ce256a970fc -->
## Purpose

The `view` cluster holds the typed `ViewModels` that the rt run-face and the dashboard render against. Each sub-module owns one cohesive shape so the Single Responsibility Principle holds — surfacing acceptance criteria lives in `quality.rs` alone, query arguments in `filter.rs` — while cross-cutting enums like `Phase` and `Scope` live in `mod.rs` because multiple views reference them. The enums carry lenient `parse` constructors (case-insensitive, fail-open to `None`) so they round-trip with the on-disk spec header and harness event payloads.
<!-- /mustard:enrich -->

## Convention

- Folder: `src/domain/model/view/`
- Extension: `.rs`
- Files: 7

## How to apply

These files are grouped by location under `src/domain/model/view/`. Add another `.rs` file in the same folder.

## Examples

- Ref: packages/core/src/domain/model/view/filter.rs
- Ref: packages/core/src/domain/model/view/mod.rs
- Ref: packages/core/src/domain/model/view/quality.rs

## Shape

Declares: 5 enum_item, 3 struct_item.

## References

See `references/examples.md`.
