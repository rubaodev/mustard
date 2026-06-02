---
name: cli-commands-pattern
description: Use when adding or refactoring a `mustard` subcommand under `src/commands/` that exposes an Options struct and an entry function dispatched from `cli.rs`.
tags: [add, refactor]
appliesTo: [commands]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: commands
---

<!-- mustard:generated -->
# commands pattern

<!-- mustard:enrich hash=6a3ff5ccfa2d -->## Purpose
Captures the shape of each `mustard` subcommand module under `src/commands/`: a public `*Options` struct holding the command's flags plus a public entry function (`add`, `config`, `configure`, ...) that takes the project path and those options and returns `anyhow::Result<()>`, invoked from the `cli.rs` dispatch table. Modules stay thin — they delegate shared logic (git-flow/locale collection, JSON merging, recursive copy) and follow the crate guards: fail-open external-tool probes, name validation, and no `unwrap`/`expect` outside tests.
<!-- /mustard:enrich -->

## Convention

- Folder: `src/commands/`
- Extension: `.rs`
- Files: 9

## How to apply

These files are grouped by location under `src/commands/`. Add another `.rs` file in the same folder.

## Examples

- Ref: apps/cli/src/commands/add.rs
- Ref: apps/cli/src/commands/config.rs
- Ref: apps/cli/src/commands/git_flow.rs

## Shape

Declares: 7 struct_item.

## References

See `references/examples.md`.
