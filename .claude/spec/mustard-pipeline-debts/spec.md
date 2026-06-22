# mustard-pipeline-debts

## Contexto

Cluster de 5 dívidas de plumbing do pipeline, reveladas ao **usar** o próprio pipeline durante a sessão da spec `otel-economy-summary-bridge` (investigação de consumo de token de uma feature rodada em `c:/atiz/sialia`). Cada dívida já foi implementada e verificada via agentes delegados; esta spec **back-filla a trilha** — a correção anterior tinha sido feita sem spec/TF (desvio de processo) e registrada num doc em `docs/` (instrumento errado, removido).

## Checklist

- [x] P1 — `spec-draft` materializa `## Checklist` na lapidação (light+full), formato do auto-mark → `packages/core/src/domain/spec/contract.rs`
- [x] P2 — `emit-pipeline` patcha `meta.json` em stage/outcome/complete + tolera `pipeline.complete` com payload null → `apps/rt/src/commands/event/emit_pipeline.rs`
- [x] P3 — `agent-prompt-render` emite `## TASK` não-vazio para spec lean (fallback Causa raiz + Plano) → `apps/rt/src/commands/agent/agent_prompt_render.rs`
- [x] P4 — teste `check_collector_missing_pid_file` determinístico (tempdir) → `apps/rt/src/commands/economy/otel/diagnose.rs`
- [x] P5 — hook advisory (warn) de delegação → `apps/rt/src/hooks/task/delegation_advisory.rs`

## Critérios de Aceitação

- [x] AC-1: P1 — `spec-draft` gera `## Checklist` parseável com ≥1 item em light e full. Command: `cargo test -p mustard-rt spec_draft`
- [x] AC-2: P2 — `emit-pipeline` patcha `meta.json` e a projeção aceita `pipeline.complete` com payload null. Command: `cargo test -p mustard-rt emit_pipeline`
- [x] AC-3: P3 — render produz bloco `## TASK` não-vazio para spec sem `## Tasks`. Command: `cargo test -p mustard-rt agent_prompt_render`
- [x] AC-4: P4 — teste do collector é determinístico independente do host. Command: `cargo test -p mustard-rt diagnose`
- [x] AC-5: P5 — hook advisory de delegação decide warn por limiar/pipeline-ativo/depth. Command: `cargo test -p mustard-rt delegation_advisory`

## Causa raiz

- P1: o contrato (`PRD_SECTIONS`/`PLAN_SECTIONS`) não emitia `## Checklist`, mas `MUSTARD_CHECKLIST_GATE_MODE=strict` + close-gate bloqueavam o CLOSE com `[ ]` pendente → gate órfão.
- P2: `emit_pipeline.rs` só sincronizava o sidecar em `pipeline.status`/`wave.complete`; stage/outcome/complete iam só pro event log. `pipeline.complete` sem payload virava `null`, rejeitado por `PipelineCompletePayload`.
- P3: `read_task_steps` retornava `""` quando não havia `## Tasks`.
- P4: o teste lia o CWD e assumia ausência de pid file.
- P5: a regra L0 de delegação não tinha enforcement nenhum.

## Plano

Resolvido por 5 fixes cirúrgicos, cada um reusando o tipo/leitor canônico (sem facade): contrato + spec-draft (P1); `meta::write_meta` + projeção lenient (P2); fallback de seção no render (P3); `check_collector_in(root)` no teste (P4); Observer novo no padrão `rt-observer-pattern` (P5). Build+test de integração (`mustard-core` + `mustard-rt`): 2915 passaram, 0 falhas.

## Limites

- **Binário não reinstalado**: a lógica nova (D1 do MCP `get_run_summary`, meta-sync de P2) só fica live após `cargo install --path apps/rt` (no Windows exige o collector liberar o `.exe`).
- **Borda mole de P5**: `subagentDepth` é proxy, não flag por-invocação — fechamento pleno depende de um campo `is_subagent`/`agent_id` no payload `PostToolUse` (follow-up em tactical-fix linkada).