# Tactical Fix: Campo is_subagent/agent_id no payload PostToolUse para fechar a borda mole do hook advisory de delegacao P5 (hoje usa proxy subagentDepth)

## Contexto

Tactical fix derivado de [[mustard-pipeline-debts]].

## Critérios de Aceitação

- [x] AC-1: o contrato lê `agent_id` do payload do harness e `is_subagent()` distingue main de subagente. Command: `cargo test -p mustard-core is_subagent_reads_harness_agent_id`
- [x] AC-2: o hook advisory não avisa dentro de subagente (por `agent_id` e pelo proxy de profundidade). Command: `cargo test -p mustard-rt delegation_advisory`

## Arquivos

- `packages/core/src/domain/model/contract.rs` — `agent_id`/`agent_type` tipados + `HookInput::is_subagent()`.
- `apps/rt/src/hooks/task/delegation_advisory.rs` — usa `is_subagent`, mantém o proxy `subagentDepth` como reforço.

<!-- wikilinks-footer-start -->
- [mustard-pipeline-debts](?) ⚠ unresolved
<!-- wikilinks-footer-end -->