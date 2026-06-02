<!-- mustard:generated -->
# Stack — `packages/core`

<!-- mustard:enrich hash=94d1ef47a514 -->
## Purpose

`packages/core` (`mustard-core`) is a pure Rust library — the agnostic kernel the CLI, runtime, and dashboard all build on. Its dependency choices reflect that role: `tree-sitter` plus the per-language grammar crates (rust, typescript, python, go, java, c-sharp) and `tree-sitter-loader` drive AST extraction; `aho-corasick` backs the framework/architecture/regression vocabularies; `tiktoken-rs` estimates token counts for the economy domain; `serde`/`serde_json`/`toml` handle the on-disk config, registry, and vocabulary documents; `sha2`, `rayon`, `similar`, and `ureq` support hashing, parallel scanning, diffing, and optional grammar acquisition; `thiserror` defines the crate error type; `insta` and `tempfile` are test-only. The crate is overwhelmingly `.rs` with embedded tree-sitter query files (`.scm`) and TOML vocabulary data.
<!-- /mustard:enrich -->

## Manifests
- `Cargo.toml`

## Dependencies
- `serde` (from `Cargo.toml`)
- `serde_json` (from `Cargo.toml`)
- `thiserror` (from `Cargo.toml`)
- `sha2` (from `Cargo.toml`)
- `rayon` (from `Cargo.toml`)
- `tiktoken-rs` (from `Cargo.toml`)
- `aho-corasick` (from `Cargo.toml`)
- `toml` (from `Cargo.toml`)
- `tree-sitter` (from `Cargo.toml`)
- `tree-sitter-loader` (from `Cargo.toml`)
- `tree-sitter-rust` (from `Cargo.toml`)
- `tree-sitter-typescript` (from `Cargo.toml`)
- `tree-sitter-python` (from `Cargo.toml`)
- `tree-sitter-go` (from `Cargo.toml`)
- `tree-sitter-java` (from `Cargo.toml`)
- `tree-sitter-c-sharp` (from `Cargo.toml`)
- `similar` (from `Cargo.toml`)
- `ureq` (from `Cargo.toml`)
- `insta` (from `Cargo.toml`)
- `tempfile` (from `Cargo.toml`)

## Source extensions
- `.rs` — 87
- `.scm` — 14
- `.toml` — 4
- `.md` — 1
- `.pending-snap` — 1

## Clusters
- 17 clusters across 107 source files
- `domain`
- `ast`
- `economy`
- `sources`
- `model`
- `view`
- `regression_check`
- `skill`