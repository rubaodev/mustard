# Tactical Fix: Token por fase: correlacionar timestamp das metricas OTEL com eventos pipeline.phase para atribuir tokens a ANALYZE/PLAN/EXECUTE (OTEL nao carrega dimensao de fase, spec:null)

## Contexto

Tactical fix derivado de [[otel-economy-summary-bridge]].

## Critérios de Aceitação

- [x] AC-1: atribuição por fase — buckets de métrica entre duas transições caem nas fases corretas (ANALYZE/EXECUTE/unattributed). Command: `cargo test -p mustard-core per_phase_token_summary_attributes_by_timestamp`
- [x] AC-2: sem regressão nos readers de economia. Command: `cargo test -p mustard-core economy`
- [x] AC-3: ambos os crates compilam (MCP `get_run_summary` phase delega ao novo reader). Command: `cargo build -p mustard-core -p mustard-rt`

## Arquivos

<!-- Paths intentionally touched -->

- `packages/core/src/domain/economy/model.rs` — new `PerPhaseTokenSummary` /
  `PhaseTokenBucket` types + `PHASE_UNATTRIBUTED` const.
- `packages/core/src/domain/economy/reader.rs` — `per_phase_token_summary`
  reader + `event_ts_ms` / `event_session_id` / `phase_active_at` helpers + test.
- `packages/core/src/domain/economy/mod.rs` — re-exports.
- `apps/rt/src/mcp/mod.rs` — wire `get_run_summary` `phase` arg to the new
  reader via `run_summary_for_phase`.

<!-- wikilinks-footer-start -->
- [otel-economy-summary-bridge](?) ⚠ unresolved
<!-- wikilinks-footer-end -->