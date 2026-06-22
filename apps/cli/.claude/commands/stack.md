<!-- mustard:generated -->
# Stack — `apps/cli`

<!-- mustard:enrich hash=2c4a467fdca7 -->
## Purpose
Records the stack of the `mustard-cli` crate: a Rust (edition 2024) port of the original TypeScript/Bun CLI that ships both the `mustard` binary and a `mustard_cli` library linked natively by the Tauri dashboard. It uses `clap` for argument parsing, `serde`/`serde_json` and `anyhow` for config and errors, `dialoguer` for interactive prompts, and `ureq` + `tar`/`flate2`/`zip` to fetch and unpack templates and skills; `mustard-core` owns the shared `ProjectConfig` schema and command detection. The `.md`/`.py`/`.json`/`.html`/`.sh` counts come from the bundled `templates/` data payloads, which are copied verbatim and never compiled.
<!-- /mustard:enrich -->

## Manifests
- `Cargo.toml`

## Dependencies
- `mustard-core` (from `Cargo.toml`)
- `serde` (from `Cargo.toml`)
- `serde_json` (from `Cargo.toml`)
- `anyhow` (from `Cargo.toml`)
- `clap` (from `Cargo.toml`)
- `dialoguer` (from `Cargo.toml`)
- `ureq` (from `Cargo.toml`)
- `tar` (from `Cargo.toml`)
- `flate2` (from `Cargo.toml`)
- `zip` (from `Cargo.toml`)
- `tempfile` (from `Cargo.toml`)

## Source extensions
- `.md` — 193
- `.rs` — 14
- `.py` — 13
- `.json` — 4
- `.html` — 2
- `.sh` — 1

## Clusters
- 2 clusters across 229 source files
- `src`
- `commands`