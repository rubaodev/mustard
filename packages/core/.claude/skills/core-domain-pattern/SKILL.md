---
name: core-domain-pattern
description: Use when adding or refactoring a `domain` module that owns a typed view of an on-disk config or registry as the single source of truth
tags: [add, refactor]
appliesTo: [domain]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: domain
---

<!-- mustard:generated -->
# domain pattern

<!-- mustard:enrich hash=58de365ab458 -->
## Purpose

The `domain` cluster holds the top-level domain logic of `mustard-core`: each module is the single typed owner of one on-disk artifact or detection concern. `config.rs` defines `ProjectConfig`, the one read/write handle for `mustard.json`; the repo model now lives in `.claude/grain.model.json`, which is produced by `mustard-rt run scan` and read only via the scan tool (`scan facts` / `scan digest`) or `mustard_core::read_entity_names` / `read_projects` (the old `entity_registry.rs` module and its `entity-registry.json` artifact were removed); `command_detect.rs` probes manifests/lockfiles to infer a stack-agnostic build/test/lint command set. Every accessor is fail-open — a missing or malformed file degrades to defaults rather than blocking a gate.
<!-- /mustard:enrich -->

## Convention

- Folder: `src/domain/`
- Extension: `.rs`
- Files: 6

## How to apply

These files are grouped by location under `src/domain/`. Add another `.rs` file in the same folder.

## Examples

- Ref: packages/core/src/domain/command_detect.rs
- Ref: packages/core/src/domain/config.rs

## Shape

Declares: 11 struct_item.

## References

See `references/examples.md`.
