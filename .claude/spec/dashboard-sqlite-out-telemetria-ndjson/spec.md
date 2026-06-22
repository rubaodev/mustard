# Dashboard: remover SQLite e reconstruir a telemetria sobre NDJSON

## Contexto

Decisão do dono: **"não se usa mais sqlite no projeto"** + opção **(b) reconstruir no NDJSON** (não só remover). Esta é a sub-spec que o `telemetry_agg.rs` já anunciava: *"Faithful NDJSON-backed reimplementations are tracked in the W6B sub-spec (wave-20-dashboard)"* — deferida de propósito no W6A. Spec-pai do redesenho de eventos/memória: [[redesenho-eventos-memoria-unificada]] (Ondas 1-3 + Estágio 1 do rt/core já DONE+verdes).

A crate `apps/dashboard/src-tauri` é **excluída do workspace cargo** (Cargo.toml raiz, linha 8); verifica-se só com `cargo check`/`cargo test` rodados **de dentro** do dir, e o React com `pnpm build`. Rodar `cargo test --workspace` da raiz NÃO a cobre.

### Estado verificado (primeira mão)

- `db.rs` é fachada no-op pós-W6A: `with_db`/`with_store` sempre `None`, ~29 fns retornam `Default`/vazio. ~25 call-sites `db::with_db(...)` em `lib.rs` + `spec_views.rs` caem sempre no fallback.
- Telemetria **viva (NDJSON)**, fica: `telemetry.rs::{workflow_by_phase, agent_activity, tool_breakdown, measured, live_activity, routing_breakdown, dashboard_prompt_economy, dashboard_economy_*}`.
- Telemetria **morta/vazia (fachada)**: `dashboard_metrics`, `dashboard_consumption(_global)`, `dashboard_knowledge`, `dashboard_recent_events`, `dashboard_search_*`, `dashboard_activity_aggregated`, `dashboard_quality_metrics`, `dashboard_active_pipelines`, `dashboard_spec_events`, `workspace_health`, e os 7 `dashboard_telemetry_*` (phases/timeline/heatmap/history/criteria/effort/agents). Hoje renderizam vazio.
- **Bug pré-existente confirmado**: 3 testes do dashboard falham hoje — `telemetry::tests::{attribution_tier1_matches_by_tool_use_id, attribution_tier2_picks_last_span_before_ts, spec_trace_lists_tool_use_events_under_spec_root}` (`src/telemetry.rs:1643/1661/1783`). A atribuição `tool_use_id → spec/agente` está quebrada — alvo do rebuild.

## Arquitetura-alvo

Fachada `db.rs` deletada. Cada comando do dashboard lê do NDJSON (`.events/*.ndjson` via `mustard_core::io::events::EventReader` + `MarkdownStore`), reusando os readers vivos do `telemetry.rs`/`economy.rs` onde já existem e ganhando agregadores novos onde faltam. Zero `with_db`, zero `Connection`, zero `mustard.db`/`telemetry.db`.

## Non-goals

- Não recriar SQLite sob outro nome. NDJSON é a fonte única.
- Views sem dado real não ganham widget vazio decorativo — ou têm agregador NDJSON, ou somem.

## Plano (ondas, cada uma com gate)

1. **Remover a fachada SQLite (backend).** Deletar `db.rs`; reescrever os ~25 `db::with_db(...)` em `lib.rs` + `spec_views.rs` para chamar direto o fallback/agregador (o closure morto nunca rodava); remover `use crate::db::Connection` em `spec_views.rs`/`telemetry_agg.rs`; remover `pub mod db` do `lib.rs`; simplificar `discovery.rs` (sem `db_path`/`has_db`); deletar/ajustar testes `db_test.rs`, `telemetry_aggregations_test.rs`, `top_files_today_test.rs`, `specs_phase_from_events_test.rs`. **Gate**: `cargo check` (de dentro de src-tauri) verde.
2. **Reconstruir os agregadores no NDJSON.** Re-apontar onde já há reader vivo (`telemetry_phases`→`workflow_by_phase`, `telemetry_agents`→`agent_activity`, consumo→`dashboard_economy_*`); construir os faltantes a partir do NDJSON (`metrics` = contagem de eventos + soma de tokens; `timeline`/`recent_events`/`search_events` = leitura ordenada; `heatmap` = eventos por dia-da-semana × hora; `sessions` = agregação de `.session/*/.events`; `quality_metrics` = derivado de eventos). **Consertar os 3 testes de atribuição.** Views sem definição clara (`effort`/`criteria`/`history`): definir a partir do dado e documentar, ou remover. **Gate**: `cargo test` (de dentro de src-tauri) verde — incl. os 3 antes-vermelhos.
3. **React.** Remover wrappers mortos em `lib/dashboard.ts` (`fetchMetrics`/`fetchKnowledge`/`fetchConsumption*` se a view sumir); ligar os vivos aos dados reais; empty-state honesto onde aplicável; ajustar tipos TS que esperavam `db_path`. **Gate**: `pnpm -C apps/dashboard build` verde.

## Critérios de Aceitação

- AC1 — sem SQLite no dashboard. Command: `rg -n "with_db|Connection|mustard\.db|telemetry\.db|rusqlite" apps/dashboard/src-tauri/src apps/dashboard/src` → zero em código de produção.
- AC2 — backend compila. Command: `cd apps/dashboard/src-tauri && cargo check` → ok.
- AC3 — atribuição consertada. Command: `cd apps/dashboard/src-tauri && cargo test telemetry::tests::attribution_tier1_matches_by_tool_use_id telemetry::tests::attribution_tier2_picks_last_span_before_ts telemetry::tests::spec_trace_lists_tool_use_events_under_spec_root` → 3 verdes.
- AC4 — suite do dashboard verde. Command: `cd apps/dashboard/src-tauri && cargo test` → 0 failed.
- AC5 — React compila. Command: `pnpm -C apps/dashboard build` → ok.
- AC6 — telemetria mostra dado real. Cada comando reescrito retorna não-vazio quando há eventos NDJSON correspondentes (teste por comando).

## Limites

Onda 1 (remover fachada) é mecânica e segura (o closure morto não roda) — não muda comportamento (widgets seguem vazios), só tira o SQLite. Onda 2 é o trabalho de verdade (agregadores + os 3 testes). Onda 3 (React) por último, com `pnpm build`. Verificar SEMPRE de dentro de `src-tauri` (a crate é fora do workspace).

<!-- wikilinks-footer-start -->
- [redesenho-eventos-memoria-unificada](?) ⚠ unresolved
<!-- wikilinks-footer-end -->