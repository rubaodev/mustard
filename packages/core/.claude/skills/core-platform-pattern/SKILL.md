---
name: core-platform-pattern
description: Use when adding or refactoring a `platform` module that owns enforcement config, the crate error type, or another cross-cutting fail-open primitive
tags: [add, refactor]
appliesTo: [platform]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: platform
---

<!-- mustard:generated -->
# platform pattern

<!-- mustard:enrich hash=9cf658bab8c7 -->
## Purpose

The `platform` cluster holds the crate's cross-cutting primitives that sit beneath the pure domain layer. `error.rs` defines the single `Error` enum and the `fail_open` / `fail_open_with` helpers that make the crate's never-panic contract explicit — a hook degrades safely instead of crashing. `config.rs` defines `EnforcementConfig`, the typed table that replaces the scattered `MUSTARD_*_MODE` environment variables, resolving each check's `Mode` (off/warn/strict) in a last-wins cascade of defaults, `mustard.json`, then env. Parsing is fail-open throughout: an unrecognised value or malformed file falls back to the default rather than blocking.
<!-- /mustard:enrich -->

## Convention

- Folder: `src/platform/`
- Extension: `.rs`
- Files: 7

## How to apply

These files are grouped by location under `src/platform/`. Add another `.rs` file in the same folder.

## Examples

- Ref: packages/core/src/platform/config.rs
- Ref: packages/core/src/platform/env.rs
- Ref: packages/core/src/platform/error.rs

## Shape

Members commonly end in: `Found`, `Input`, `Failed`.

Declares: 3 enum_item, 3 struct_item, 1 trait_item, 1 type_item.

## References

See `references/examples.md`.
