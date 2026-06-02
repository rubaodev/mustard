<!-- mustard:generated -->
# platform — examples in this codebase

<!-- mustard:enrich hash=034bee5e9f16 -->
## Purpose

These files are the platform-level primitives every higher layer leans on. `config.rs` is `EnforcementConfig`: a per-check `Mode` table (off/warn/strict) plus a disabled set, resolved last-wins from defaults → `mustard.json` `enforcement` block → `MUSTARD_<CHECK>_MODE` env vars, with fail-open parsing so a config typo never blocks a hook. `error.rs` is the crate `Error` enum (`#[non_exhaustive]`, keeping `NotFound` distinct from `Io` so callers can treat absence as empty) and the `fail_open` / `fail_open_with` helpers that collapse a `Result` to a safe fallback — the explicit form of the JS hooks' swallow-and-continue pattern. `env.rs` is the hook runtime environment — a behavioural port of `_lib/hook-env.js` that answers "should this hook run?", guards against a hook recursing into itself, and abstracts process-env reads/writes behind an `Env` trait (`ProcessEnv` in production, `MapEnv` in tests).
<!-- /mustard:enrich -->

- Ref: packages/core/src/platform/config.rs
- Ref: packages/core/src/platform/env.rs
- Ref: packages/core/src/platform/error.rs

