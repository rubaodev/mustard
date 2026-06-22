---
name: dashboard-tauri-pattern
description: Use when adding or refactoring a Tauri backend binding under `src/api/` or `src/lib/` that wraps `invoke()` from `@tauri-apps/api/core` into a typed Promise-returning function.
tags: [add, refactor]
appliesTo: [tauri]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: tauri
---

<!-- mustard:generated -->
# tauri pattern

<!-- mustard:enrich hash=5a4fb7c2f771 -->
## Purpose

These modules are the single typed seam between the React frontend and the Rust Tauri backend. Each function wraps a single `invoke<T>("command_name", args)` call from `@tauri-apps/api/core`, declaring the `interface`/`type` shapes the Rust command returns so components import strongly-typed calls instead of touching `invoke()` directly.
<!-- /mustard:enrich -->

## Convention

- Folder: `**/src/`
- Suffix: `tauri`
- Extension: `.ts`
- Files: 3

## How to apply

To add a new `tauri`, create a `.ts` file under `**/src/` whose name ends with `tauri`.

## Examples

- Ref: apps/dashboard/src/api/env.ts
- Ref: apps/dashboard/src/lib/dashboard.ts
- Ref: apps/dashboard/src/lib/projects.ts

## Shape

Common top-of-file (shared by all samples):
- `}`

Declares: 57 interface_declaration, 2 type_alias_declaration.

## References

See `references/examples.md`.
