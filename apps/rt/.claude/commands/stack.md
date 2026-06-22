<!-- mustard:generated -->
# Stack — `apps/rt`

<!-- mustard:enrich hash=a4e17e10531f -->
## Purpose

`apps/rt` is the `mustard-rt` runtime binary: a Rust crate built on `mustard-core` that backs the Mustard harness. It exposes a `clap` command-line surface, serves the dashboard/MCP integration over `tiny_http` and `rmcp` on a `tokio` async runtime, and reaches external services with `ureq`. The hook layer watches the workspace with `notify`, hashes content with `sha2`, and parallelizes scans with `rayon`; harness events and config are (de)serialized through `serde`/`serde_json`. The bulk of the crate is Rust (236 `.rs` files), organised into the `observer` and `inject` hook clusters.
<!-- /mustard:enrich -->

## Manifests
- `Cargo.toml`

## Dependencies
- `mustard-core` (from `Cargo.toml`)
- `serde` (from `Cargo.toml`)
- `serde_json` (from `Cargo.toml`)
- `clap` (from `Cargo.toml`)
- `tiny_http` (from `Cargo.toml`)
- `rmcp` (from `Cargo.toml`)
- `tokio` (from `Cargo.toml`)
- `ureq` (from `Cargo.toml`)
- `tempfile` (from `Cargo.toml`)
- `notify` (from `Cargo.toml`)
- `rayon` (from `Cargo.toml`)
- `sha2` (from `Cargo.toml`)

## Source extensions
- `.rs` — 236
- `.md` — 13
- `.js` — 1
- `.ps1` — 1
- `.toml` — 1

## Clusters
- 2 clusters across 252 source files
- `observer`
- `inject`