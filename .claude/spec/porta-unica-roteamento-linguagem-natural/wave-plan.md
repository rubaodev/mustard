---
id: wave.porta-unica-roteamento-linguagem-natural.plan
---

# Wave Plan

## Wave Table

| Wave | Spec | Role | Depends on | Summary |
|------|------|------|------------|---------|
| 1 | [[wave-1-backend]] | backend | — | Telemetria de tipo: evento pipeline.kind no event model + KNOWN_KINDS + emissao deterministica (side-effect) incluindo caminhos lean (task, bugfix fast-path) |
| 2 | [[wave-2-orchestrator]] | orchestrator | [[wave-1-backend]] | Porta unica: CLAUDE.md Intent Routing vira roteador (classifica, narra, confirma na duvida, despacha, emite kind); neutraliza descricoes dos 4 fluxos; ajuda /mustard; doc em duas audiencias |

## Acceptance Criteria
- **AC-3** — pipeline.kind reconhecido pelo emissor. Command: `grep -q "pipeline.kind" apps/rt/src/commands/event/emit_pipeline.rs`
- **AC-4** — constante no event model. Command: `grep -q "EVENT_PIPELINE_KIND" packages/core/src/domain/model/event.rs`
- **AC-5** — caminho lean emite o tipo. Command: `cargo test pipeline_kind`
- **AC-6** — descricao do fluxo neutralizada. Command: `grep -q "internal flow" apps/cli/templates/commands/mustard/feature/SKILL.md`
- **AC-1** — build verde. Command: `cargo build`
- **AC-2** — testes verdes. Command: `cargo test`
