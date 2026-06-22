---
name: dashboard-use-pattern
description: Use when adding or refactoring a React `useXxx` hook under `src/hooks/` that fetches and derives dashboard state via TanStack Query (`useQueries`/`useQuery`) over the Tauri binding layer.
tags: [add, refactor]
appliesTo: [use]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: use
---

<!-- mustard:generated -->
# use pattern

<!-- mustard:enrich hash=a6704cf80993 -->
## Purpose

These are the custom React hooks that turn raw Tauri command results into the shapes pages render. Each hook calls TanStack Query (`useQueries` for a fan-out across registered projects, `useQuery` for a single lookup) against the `invoke()` wrappers in `lib/`, then sorts, filters, and aggregates the results into a typed view object (counters, feed rows, drift maps) so components stay free of fetching and reducing logic.
<!-- /mustard:enrich -->

## Convention

- Folder: `**/src/`
- Suffix: `use`
- Extension: `.ts`
- Naming: `suffix-before`
- Files: 36

## How to apply

To add a new `use`, create a `.ts` file under `**/src/` whose name starts with `use`.

## Examples

- Ref: apps/dashboard/src/hooks/useActivityFeed.ts
- Ref: apps/dashboard/src/hooks/useAggregate.ts
- Ref: apps/dashboard/src/hooks/useArtifactDrift.ts

## Shape

Declares: 7 interface_declaration.

## References

See `references/examples.md`.
