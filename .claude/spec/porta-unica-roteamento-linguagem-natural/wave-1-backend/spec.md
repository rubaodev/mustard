---
id: wave.porta-unica-roteamento-linguagem-natural.1-backend
---

# wave-1-backend

## Summary

Telemetria de tipo: evento pipeline.kind no event model + KNOWN_KINDS + emissao deterministica (side-effect) incluindo caminhos lean (task, bugfix fast-path)

## Network

- Parent: [[porta-unica-roteamento-linguagem-natural]]

## Tasks

- [ ] Adicionar EVENT_PIPELINE_KIND + PipelineKindPayload {kind, scope} em packages/core/src/domain/model/event.rs
- [ ] Registrar pipeline.kind em KNOWN_KINDS de apps/rt/src/commands/event/emit_pipeline.rs
- [ ] Emitir pipeline.kind como side-effect deterministico nos caminhos lean (task, bugfix fast-path), espelhando skill_usage_observer.rs
- [ ] Teste byte-estavel: um run lean emite pipeline.kind

## Files

- `packages/core/src/domain/model/event.rs`
- `apps/rt/src/commands/event/emit_pipeline.rs`
- `apps/rt/src/hooks/task/skill_usage_observer.rs`
- `apps/rt/src/shared/events/route.rs`
