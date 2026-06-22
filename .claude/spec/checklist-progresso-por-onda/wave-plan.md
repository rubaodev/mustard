# Wave Plan

## Wave Table

| Wave | Spec | Role | Depends on | Summary |
|------|------|------|------------|---------|
| 1 | [[wave-1-impl]] | impl | — | Core (packages/core): ChecklistItem ganha campo `done: bool` (default false); o tipo Meta ganha o campo `checklist: Vec<ChecklistItem>` serde-tolerante (metas antigos sem o campo continuam validos); registrar o evento `checklist.item.marked` no enum de eventos. Base contratual de tudo. Subprojeto: packages/core. |
| 2 | [[wave-2-impl]] | impl | [[wave-1-impl]] | Rt (apps/rt): wave-scaffold semeia o `checklist` no meta.json de cada onda a partir dos arquivos-alvo do plano; auto-mark + mark-checklist-item passam a gravar `done` no meta e emitir `checklist.item.marked`; close-gate (find_unmarked_checklist) consolida lendo os metas das ondas (nao markdown); spec-validate valida a checklist a partir do meta. Subprojeto: apps/rt. |
| 3 | [[wave-3-impl]] | impl | [[wave-1-impl]], [[wave-2-impl]] | Dashboard (apps/dashboard): agregador NDJSON em src-tauri projeta `checklist.item.marked` em progresso por onda; render `N/M itens` por onda em SpecWavesTab/WaveRowLabel/WaveMarkdownDrawer, ocupando a view criteria que esta orfa. Verificar de dentro de src-tauri (crate fora do workspace) + pnpm build. Subprojeto: apps/dashboard. |
| 4 | [[wave-4-impl]] | impl | [[wave-2-impl]] | Templates/SKILL (apps/cli/templates + .claude/skills): instruir o PLAN de onda a popular a checklist por onda; remover os cabecalhos de lifecycle legados que a SKILL pipeline-execution ainda menciona no markdown. Subprojeto: apps/cli. |
