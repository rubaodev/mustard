# wave-1-impl

## Summary

Core (packages/core): ChecklistItem ganha campo `done: bool` (default false); o tipo Meta ganha o campo `checklist: Vec<ChecklistItem>` serde-tolerante (metas antigos sem o campo continuam validos); registrar o evento `checklist.item.marked` no enum de eventos. Base contratual de tudo. Subprojeto: packages/core.

## Network

- Parent: [[checklist-progresso-por-onda]]

## Arquivos

- `packages/core/src/domain/spec/contract.rs` — `ChecklistItem` ganha `done: bool` com `#[serde(default)]`; `render_checklist_item` reflete `[x]` quando `done`; rever a regra `ChecklistEmpty` do validador.
- `packages/core/src/domain/meta/*` (tipo `Meta`) — novo campo `checklist: Vec<ChecklistItem>` com `#[serde(default)]` (round-trip de metas antigos sem o campo continua válido).
- `packages/core/src/domain/model/event/*` — registrar o evento `checklist.item.marked` (payload `{spec, wave, item, path}`).

## Tarefas

1. **`ChecklistItem.done`:** adicionar `done: bool` com `#[serde(default)]` em `contract.rs`; `render_checklist_item` emite `- [x]` quando `done==true`, senão `- [ ]`. Campo aditivo, serde-compatível.
2. **`Meta.checklist`:** adicionar `checklist: Vec<ChecklistItem>` ao tipo `Meta` com `#[serde(default)]`, preservando o shape (rt e dashboard renderizam sobre `Meta` — não quebrar). Teste de round-trip: um `meta.json` antigo, sem o campo, desserializa para `checklist: vec![]`.
3. **Evento `checklist.item.marked`:** registrar a variante/serialização no `domain/model/event` seguindo o padrão dos eventos existentes (ex.: `qa.result`), payload `{spec, wave, item, path}`. Só o tipo + serialização — a emissão fica na Onda 2.
4. **Pureza:** `domain/model` permanece sem IO (guard do core); sem `unwrap`/`expect` fora de teste; escrita só via `io::fs::write_atomic` onde aplicável.

## Critérios de Aceitação

- **AC-1** — `Meta` ganha `checklist` sem quebrar o shape serde; round-trip de um meta antigo (sem o campo) continua válido; `ChecklistItem.done` serializa/desserializa. Testes verdes.
  Command: `cargo test -p mustard-core`

<!-- wikilinks-footer-start -->
- [checklist-progresso-por-onda](?) ⚠ unresolved
<!-- wikilinks-footer-end -->