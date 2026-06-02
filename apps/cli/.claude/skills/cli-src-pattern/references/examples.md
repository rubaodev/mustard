<!-- mustard:generated -->
# src — examples in this codebase

<!-- mustard:enrich hash=8189aaf2f911 -->
## Purpose
Concrete crate-root modules to pattern-match against. `cli.rs` is the parsing/dispatch layer: a derive-based `Cli` struct, a `Commands` enum with one variant per subcommand and its flags, and a `dispatch` function that maps each variant to a `commands::*` entry. `fs_ops.rs` holds the install primitives shared by `init`/`update`/`add` — `copy_dir` (recursive copy honouring an overwrite flag and a top-level skip list) and `read_json_object` (fail-open read that collapses missing/malformed/non-object input to an empty map). `lib.rs` is the library root: `#![forbid(unsafe_code)]`, the `pub mod` declarations, re-exports of `init`/`update`, and the compile-time `VERSION` constant.
<!-- /mustard:enrich -->

- Ref: apps/cli/src/cli.rs
- Ref: apps/cli/src/fs_ops.rs
- Ref: apps/cli/src/lib.rs

