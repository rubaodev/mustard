# Redesenho do stream de eventos + memória unificada (e corte de camadas mortas)

## Contexto

Auditoria de produtor→consumidor das quatro camadas que sustentam orquestração, memória e telemetria. A intenção original do dono: **um único stream de eventos (`<spec>/.events`) que (1) deixa o dashboard acompanhar a evolução da spec evento a evento e (2) alimenta a confecção de contexto entre ondas e agentes.** Historicamente o `mustard.db` (SQLite) era o sink, ficou lento, e `.events`/`.blobs` nasceram para servir o dashboard rápido.

O desenho é correto e é o `core-projection-pattern` que o repo já tem (uma fonte, várias projeções). O problema é execução: a migração foi feita pela metade e dessincronizou.

**Estado verificado em primeira mão (grep + leitura de call-sites):**

- `<spec>/.events/*.ndjson` — **carga real**. Lido por gates (`close_gate`), projeções e o dashboard (`telemetry.rs`, `spec_views.rs`). Fica.
- **Memória entre ondas — MORTA no fio.** `memory_cross_wave` (apps/rt/src/commands/knowledge/memory_cross_wave.rs:189) filtra `event == "agent.memory"` no `.events`. Grep em todo `apps/rt/src`: `agent.memory` só existe no leitor e nos testes. O único writer de resumo de agente (`memory.rs::run_agent`) grava arquivo `.claude/.agent-memory/*.json` e **não emite evento**. Dois sinks dessincronizados → o bloco "Memórias de waves anteriores" vem sempre vazio (e o `collapse_empty_sections` o apaga).
- **Memória entre agentes da mesma onda — inexistente.** Agentes paralelos do mesmo `level` são contextos `Task` isolados. Trade-off físico (paralelismo × visibilidade-mútua), não bug.
- **`mustard.db` — MORTO.** W6A trocou SQLite por NDJSON; o `db.rs` do dashboard é fachada que retorna `None`; ninguém escreve nem lê. ~35 arquivos ainda referenciam (rt + core + dashboard + testes).
- **`.blobs` — write-only.** `writer_ndjson.rs:237` move payload >4KB para `.blobs` e põe só a referência `$blob` na linha; o reader `blob_path()` é `#[allow(dead_code)]` e o dashboard tem zero referências. Payload grande é silenciosamente engolido por todos os consumidores. Era otimização de perf (linha pequena = dashboard rápido) com o lado de leitura nunca construído.
- **`.session/<uuid>/.events` — redundante.** Fallback legado; o dashboard ainda o varre (`walk_ndjson_events`) mas é redundante com o spec-scoped (eventos já carregam `session_id` pós-OTEL). ~25 pastas acumulando.
- **`<spec>/qa/spec.md` + `<spec>/review/spec.md` — scaffolds mortos.** Escritos por `wave_scaffold` mas lidos por ninguém (gates leem os eventos `qa.result`/`review.result`). O que é carga real são `qa/report.md` e `review/verdict.md`, escritos sob demanda por `qa_run`/`review_result` (que já fazem `create_dir_all` da pasta).

## Arquitetura-alvo

Uma fonte (`<spec>/.events`, eventos PEQUENOS) → três projeções (dashboard timeline, memória cross-wave, gates). Artefato grande mora em arquivo (`diff.md`, `verdict.md`) e o evento referencia o **path**, nunca spilla payload. `mustard.db`, `.blobs` e o fallback `.session` saem.

## Non-goals

- Troca de informação entre agentes paralelos da MESMA onda (esbarra no paralelismo; o orquestrador + a próxima onda é o meio realista). Documentar o trade-off, não construir um canal que serializa a onda.
- Reescrever o dashboard (só remover o consumo de `mustard.db`/`.session`/`.blobs`).

## Plano de execução (cada item = uma onda/TF com gate próprio)

1. **Corte qa/review scaffold** (apps/rt/wave_scaffold.rs). Remover `render_qa`/`render_review` + o emit de `qa/spec.md`/`review/spec.md` + os campos `qa_*`/`review_*` do `Headings` + ajustar o teste de contagem. `qa_run`/`review_result` já criam a pasta sob demanda. Confinado, sem mudança de comportamento de gate.
2. **Religar memória cross-wave (a feature do dono).** No ponto de captura do resumo do agente (`agent_summary_observer` / `run_agent`), emitir um evento `agent.memory` em `<spec>/.events` com `{wave, role, summary, decisões, arquivos}` — a mesma fonte que o dashboard já lê. Subir o cap (`MAX_MEMORIES_PER_WAVE` 5 → ≥20). Manter UM canal (SRP: parar de gravar o `.agent-memory/*.json` paralelo OU torná-lo derivado do evento).
3. **Corte `.blobs`.** Remover `maybe_spill`/`blob_path`/`blob_spill.rs` e o ramo `Spilled` do `writer_ndjson.rs`; payload sempre inline; capar payload no emit (eventos pequenos por contrato). Ajustar testes do writer.
4. **Corte `mustard.db`.** Remover o accessor (`claude_paths.rs`), a sondagem de mtime (`session_stop_observer`), a fachada `db.rs` + `watcher` no dashboard, o reader legado em `packages/core/economy`, e os testes mortos. Multi-crate — coordenar com o build do dashboard (cargo + pnpm).
5. **Consolidar `.session`.** Remover o fallback de roteamento sem-spec (`writer_ndjson` `event_dir`) e a varredura redundante no dashboard (`walk_ndjson_events`).

## Critérios de Aceitação

- AC1 — qa/review scaffold removido. Command: `rg -n "render_qa|render_review" apps/rt/src/commands/wave/wave_scaffold.rs` → sem matches; `cargo test --workspace` verde.
- AC2 — `qa_run`/`review_result` criam a pasta sem o scaffold prévio. Command: teste que roda `qa_run` num spec-dir sem `qa/` e afirma que `qa/report.md` é escrito.
- AC3 — memória cross-wave viva (E2E). Command: teste que captura um resumo de agente da onda 1, dispara o render da onda 2 e afirma que o summary aparece no bloco injetado.
- AC4 — `.blobs` removido. Command: `rg -n "blob_spill|maybe_spill|\.blobs" apps/rt apps/dashboard packages/core` → sem matches de produção; `cargo test --workspace` verde.
- AC5 — `mustard.db` removido. Command: `rg -n "mustard\.db|mustard_db|with_db|telemetry_store_for" apps packages` → sem matches de produção; `cargo test --workspace` + build do dashboard verdes.
- AC6 — `.session` fallback removido. Command: `rg -n "\.session.*events|session_slug" apps/rt/src/shared/events` → sem o ramo de fallback; dashboard ainda hidrata a economia a partir do spec-scoped.

## Limites

Ordem importa: 1 e 2 primeiro (baixo risco + a feature que o dono pediu). 4 (mustard.db, 35 arquivos) e 5 (.session) tocam o dashboard — fazer por último, cada um com build rt+pnpm verde antes de seguir.

## Progresso

- ✓ **Onda 1 (qa/review scaffold)** — DONE, 3137 testes verdes. `render_qa`/`render_review` + campos `Headings` removidos; `qa_run`/`review_result` criam a pasta sob demanda.
- ✓ **Onda 2 (religar memória)** — DONE. `run_agent` emite `agent.memory` no `<spec>/.events`; cap 5→20; teste E2E `agent_emits_cross_wave_memory_event` (AC3) verde.
- ✓ **Onda 3 (corte .blobs)** — DONE. `blob_spill.rs` deletado; writer inline sempre; teste prova que `.blobs` não é criado (AC4). 3131 verdes.
- ☐ **Onda 4 (mustard.db, 35 arq, dashboard)** — pendente.
- ☐ **Onda 5 (.session, dashboard)** — pendente.

Binário NÃO reinstalado desde Onda 1 — verde mas inerte até reinstalar.