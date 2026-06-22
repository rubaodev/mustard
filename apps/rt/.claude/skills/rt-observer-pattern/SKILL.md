---
name: rt-observer-pattern
description: Use when adding or refactoring an observer hook that reacts to a harness lifecycle event with side-effects only.
tags: [add, refactor]
appliesTo: [observer]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: observer
---

<!-- mustard:generated -->
# observer pattern

<!-- mustard:enrich hash=7d949790085f -->
## Purpose

The `observer` modules are pure `Observer` hooks: they react to harness lifecycle triggers (a `Task` subagent returning, `SessionEnd`, `Notification`) by performing side-effects only and never block the pipeline. They persist agent-memory rows, promote high-confidence memories into permanent decision/lesson files, or append events to the per-spec NDJSON log, and every IO step degrades to a no-op (fail-open).
<!-- /mustard:enrich -->

## Convention

- Folder: `**/hooks/`
- Suffix: `observer`
- Extension: `.rs`
- Naming: `suffix-after`
- Files: 15

## How to apply

To add a new `observer`, create a `.rs` file under `**/hooks/` whose name ends with `observer`.

## Examples

- Ref: apps/rt/src/hooks/observe/agent_summary_observer.rs
- Ref: apps/rt/src/hooks/observe/memory_promote_observer.rs
- Ref: apps/rt/src/hooks/observe/notification_observer.rs

## Shape

Declares: 3 struct_item.

## References

See `references/examples.md`.
