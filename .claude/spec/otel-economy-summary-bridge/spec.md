# otel-economy-summary-bridge

## Contexto

Quando se pergunta "pra onde foram os tokens?", a telemetria OTEL **captura** o dado — `claude_code.token.usage` e `cost.usage` chegam ao collector e são gravados como NDJSON — mas toda consulta de resumo retorna **zero**. Confirmado empiricamente na sessão `fda6d733` (rodada no projeto `c:/atiz/sialia`, modelo `opus-4-8[1m]`): `diagnose-otel` mostra o collector vivo com centenas de registros, enquanto `get_run_summary`, `metrics report` e a view `session-summary` retornam contagem 0.

São dois defeitos independentes, ambos provados em disco:

- **Defeito A — leitura divergente.** O registro existe no NDJSON com `event/kind = "pipeline.telemetry.metric"` e o número real vive em `payload.sum` + `payload.token_type`. O reader do core `economy_summary` **já consome** esse tipo (documentado em `packages/core/src/domain/economy/reader.rs:13`). Mas o MCP `get_run_summary` **não usa o reader do core** — ele reimplementa a agregação com um filtro próprio que só aceita `pipeline.telemetry.run`, descartando todo `pipeline.telemetry.metric`. Por isso a contagem é 0 mesmo com o dado presente. É duplicação de lógica que divergiu da fonte canônica.

- **Defeito B — atribuição errada.** Os eventos de métrica caem em `.claude/.session/otel-unattached/.events/` (nunca no diretório da sessão de origem) e podem cair no **projeto errado**: os ~224 KB de tokens da sessão do sialia foram gravados dentro de `mustard/.claude/.session/otel-unattached/`, e só ~61 KB ficaram em `sialia/.claude/.session/otel-unattached/`. O diretório correto da sessão (`sialia/.claude/.session/fda6d733/.events/`) recebeu só eventos de pipeline minúsculos (711–757 B), nenhum token. Consequência: mesmo com o Defeito A resolvido, uma consulta a partir do sialia perde os tokens que vazaram para o mustard.

O objetivo é tornar o total de tokens consultável e correto por sessão/modelo/tipo-de-token, para que "pra onde foram os tokens?" tenha resposta com dado.

## Critérios de Aceitação

- [ ] AC-1: Repro da leitura — o resumo retorna totais de token diferentes de zero, agrupados por modelo, a partir de registros `pipeline.telemetry.metric` de `claude_code.token.usage` (antes do fix retornava 0). Command: `cargo test -p mustard-rt run_summary_includes_metric_events`
- [ ] AC-2: Consolidação no core (sem facade) — `get_run_summary` delega ao reader canônico do core em vez do filtro próprio, e os totais batem com o reader do core para o mesmo fixture. Command: `cargo test -p mustard-rt run_summary_matches_core_economy`
- [ ] AC-3: Atribuição — métrica com `session_id` resolvível é gravada sob o diretório da sessão de origem, não em `otel-unattached`. Command: `cargo test -p mustard-rt otel_metric_routed_to_origin_session`
- [ ] AC-4: Clareza do env — o caminho dual-emit saudável (`MUSTARD_HARNESS_DUAL_EMIT=1` + collector vivo) reporta `env.ok=true` mesmo com `OTEL_*` unset. Command: `cargo test -p mustard-rt env_ok_under_dual_emit_even_when_exporter_vars_unset`

Verificações fora do gate de QA (não exit-0-friendly, conferidas à mão): o literal `"pipeline.telemetry.run"` sumiu do corpo de `get_run_summary` (`rg` → sem ocorrência) e `mustard-rt run diagnose-otel --json` passa a emitir `env.ok=true` após o binário ser reinstalado.

## Causa raiz

- `apps/rt/src/mcp/mod.rs:525` — `get_run_summary` filtra `e.kind == "pipeline.telemetry.run"` e agrega com `summarize_runs` próprio, ignorando `pipeline.telemetry.metric`.
- `packages/core/src/domain/economy/reader.rs:13,54,63` — `economy_summary` já trata `pipeline.telemetry.metric` (medido) + `pipeline.telemetry.run`/`pipeline.economy.run` (estimado); é a agregação canônica que o MCP deveria reusar.
- Caminho de escrita de métrica do collector grava em `otel-unattached/` em vez de atrelar ao `session_id` (que está presente no payload) → vazamento cross-projeto.
- `diagnose-otel` exige `OTEL_METRICS_EXPORTER` / `OTEL_EXPORTER_OTLP_ENDPOINT` setados para `env.ok`, mas o dual-emit já entrega o dado sem eles — o INCOMPLETE é cosmético.

## Plano

1. **Parte 1 — leitura + env (baixo risco).** Reescrever `get_run_summary` para delegar ao `economy_summary` do core, mapeando o `EconomySummary` para o shape de saída atual (count / totalInputTokens / totalOutputTokens / byModel). Remover o `summarize_runs` redundante se nenhum outro chamador o usar. Ajustar `diagnose-otel` para tratar o dual-emit como caminho válido. Teste de regressão no seam core↔MCP (AC1, AC2, AC4).
2. **Parte 2 — atribuição no collector (arquitetural, risco maior).** No caminho de escrita de métrica do collector, resolver `session_id` → projeto + diretório de sessão e gravar lá; manter `otel-unattached` apenas para `session_id` irresolvível. Exige ler o código do collector antes de fixar o desenho. Teste de roteamento (AC3). Se a Parte 1 precisar ser entregue antes, a Parte 2 vira spec filha ligada a esta (decidir na aprovação).

## Limites

- **Atribuição por fase está fora de escopo.** Os registros de métrica do OTEL têm `spec: null` e nenhum campo de fase — token por ANALYZE/PLAN/EXECUTE não sai só do OTEL; exigiria correlacionar o timestamp da métrica com eventos `pipeline.phase` por janela de tempo, uma feature à parte. Este fix entrega total por **sessão + modelo + tipo-de-token** (input/output/cacheRead/cacheCreation), que já responde o achado central: a maior parte do "108k" é `cacheRead`, barato.
- Não toca em pricing nem no cálculo de custo (`cost.usage` já flui corretamente).
- Não altera o que o dashboard (Tauri) lê do seu próprio SQLite; o alvo é o store NDJSON consultado por MCP/CLI.