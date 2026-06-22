---
name: cli-src-pattern
description: Use when adding or refactoring a crate-root `src/*.rs` module of mustard-cli, such as argument parsing, filesystem primitives, or the library facade.
tags: [add, refactor]
appliesTo: [src]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: src
---

<!-- mustard:generated -->
# src pattern

<!-- mustard:enrich hash=d3ea979760d2 -->## Purpose
Captures the top-level `src/` layout of the `mustard-cli` crate: `lib.rs` is the library face the Tauri dashboard links against (declaring `cli`, `commands`, `fs_ops` and the `VERSION` constant), `cli.rs` defines the `clap` `Cli`/`Commands` types and the `dispatch` table that forwards each subcommand to a `commands` module, and `fs_ops.rs` holds the shared filesystem primitives (`copy_dir` recursive copy with overwrite + top-level skip, `read_json_object` fail-open JSON read). New root modules should keep `main.rs` thin and put reusable logic in the library.
<!-- /mustard:enrich -->

## Convention

- Folder: `src/`
- Extension: `.rs`
- Files: 4

## How to apply

These files are grouped by location under `src/`. Add another `.rs` file in the same folder.

## Examples

- Ref: apps/cli/src/cli.rs
- Ref: apps/cli/src/fs_ops.rs
- Ref: apps/cli/src/lib.rs

## Shape

Declares: 1 enum_item, 1 struct_item.

## References

See `references/examples.md`.
