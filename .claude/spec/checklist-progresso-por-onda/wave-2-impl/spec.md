# wave-2-impl

## Summary

Rt (apps/rt): wave-scaffold semeia o `checklist` no meta.json de cada onda a partir dos arquivos-alvo do plano; auto-mark + mark-checklist-item passam a gravar `done` no meta e emitir `checklist.item.marked`; close-gate (find_unmarked_checklist) consolida lendo os metas das ondas (nao markdown); spec-validate valida a checklist a partir do meta. Subprojeto: apps/rt.

## Network

- Parent: [[checklist-progresso-por-onda]]
- Depends on: [[wave-1-impl]]

## Arquivos

- `apps/rt/src/commands/wave/wave_scaffold.rs` — semear `checklist` no `meta.json` de cada onda (no `write_scaffold_meta`), a partir dos arquivos-alvo do plano.
- `apps/rt/src/commands/checklist/mark_checklist_item.rs` — gravar `done=true` no `meta.json` da onda + emitir o evento.
- hook `checklist-auto-mark` (`apps/rt/src/hooks/write/`) — marcar `done` no meta da onda do arquivo editado + emitir `checklist.item.marked`.
- `apps/rt/src/hooks/write/close_gate.rs` — `find_unmarked_checklist` consolida lendo o `checklist` dos metas das ondas (não a seção markdown).
- `apps/rt/src/commands/spec/spec_validate.rs` — validar a checklist a partir do meta.

## Tarefas

1. **Semear no scaffold:** em `wave_scaffold.rs`, ao escrever o `meta.json` de cada onda, preencher `checklist` com um item por arquivo-alvo (`{label, path, done:false}`), reusando `ChecklistItem` da Onda 1.
2. **Marcar no meta + evento:** `mark_checklist_item.rs` e o hook `checklist-auto-mark` localizam o item pelo `path`/basename no `meta.json` da onda, setam `done=true` (idempotente) e emitem `checklist.item.marked` no NDJSON. Hook fail-open (nunca entra em pânico; degrade no JSON, exit 0).
3. **Close-gate via meta:** `find_unmarked_checklist` lê o `checklist` dos metas das ondas e bloqueia o CLOSE se algum `done==false`; libera quando todos `true`. Preservar a semântica anti-gate-órfão.
4. **spec-validate:** validar a checklist a partir do meta (não do markdown).
5. **Saída determinística:** comandos `run` mantêm JSON byte-estável (há snapshots `insta`); registrar subcomando novo nos DOIS pontos (enum `RunCmd` + braço do `dispatch`) se aplicável.

## Critérios de Aceitação

- **AC-1** — wave-scaffold grava `checklist` no meta de cada onda; auto-mark/mark-item setam `done` + emitem evento; close-gate bloqueia/libera por `done`. Testes verdes.
  Command: `cd apps/rt && cargo test`

<!-- wikilinks-footer-start -->
- [checklist-progresso-por-onda](?) ⚠ unresolved
- [wave-1-impl](?) ⚠ unresolved
<!-- wikilinks-footer-end -->