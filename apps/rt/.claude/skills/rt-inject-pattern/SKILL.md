---
name: rt-inject-pattern
description: Use when adding or refactoring an inject module that computes context text to splice into an agent's window.
tags: [add, refactor]
appliesTo: [inject]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: inject
---

<!-- mustard:generated -->
# inject pattern

<!-- mustard:enrich hash=ef861f8931d6 -->
## Purpose

The `inject` modules build the context text that gets spliced into an agent's prompt window. Each one gathers project signal — spec-memory files matched against the dispatch intent, the regression vocabulary, amendment-window drift forecasts, or recent agent-memory summaries before a compaction — and returns it either as a rendered block or as a `Verdict::Inject { context }`, always fail-open so a missing source yields no injection.
<!-- /mustard:enrich -->

## Convention

- Folder: `**/src/`
- Suffix: `inject`
- Extension: `.rs`
- Naming: `suffix-after`
- Files: 7

## How to apply

To add a new `inject`, create a `.rs` file under `**/src/` whose name ends with `inject`.

## Examples

- Ref: apps/rt/src/commands/agent/context_inject.rs
- Ref: apps/rt/src/hooks/observe/amend_window_inject.rs
- Ref: apps/rt/src/hooks/observe/pre_compact_memory_inject.rs

## Shape

Declares: 5 struct_item.

## References

See `references/examples.md`.
