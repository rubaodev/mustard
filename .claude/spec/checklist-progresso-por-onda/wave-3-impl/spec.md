# wave-3-impl

## Summary

Dashboard (apps/dashboard): agregador NDJSON em src-tauri projeta `checklist.item.marked` em progresso por onda; render `N/M itens` por onda em SpecWavesTab/WaveRowLabel/WaveMarkdownDrawer, ocupando a view criteria que esta orfa. Verificar de dentro de src-tauri (crate fora do workspace) + pnpm build. Subprojeto: apps/dashboard.

## Network

- Parent: [[checklist-progresso-por-onda]]
- Depends on: [[wave-1-impl]], [[wave-2-impl]]

## Arquivos

- `apps/dashboard/src-tauri/src/*` — agregador NDJSON que projeta `checklist.item.marked` em progresso por onda (seguir os readers vivos de `telemetry.rs`).
- `apps/dashboard/src/lib/dashboard.ts` + `src/api/*` — binding `invoke()` tipado.
- `apps/dashboard/src/hooks/useXxx.ts` — hook TanStack Query do progresso.
- `apps/dashboard/src/features/specs/SpecWavesTab/index.tsx`, `src/components/page/WaveRowLabel/index.tsx`, `src/features/specs/WaveMarkdownDrawer/index.tsx` — render `N/M itens` por onda.

## Tarefas

1. **Agregador (backend):** em `src-tauri`, ler `.events/*.ndjson` (via `EventReader`/readers vivos) e projetar `checklist.item.marked` em `{wave -> {done, total}}` por spec. Comando Tauri tolerante a falha (vazio quando faltam dados).
2. **Binding + hook:** expor via `invoke()` em `src/api`/`src/lib/dashboard.ts` (chaves camelCase: `repoPath`, `specName`) + hook `useXxx` (TanStack Query, `queryKey` estável, `enabled: !!repoPath`); registrar a `queryKey` no watcher.
3. **Render por onda:** `SpecWavesTab`/`WaveRowLabel`/`WaveMarkdownDrawer` mostram `N/M itens` por onda; empty-state honesto quando não há eventos; ocupar a view `criteria` órfã se couber.
4. **Verificar de dentro de `src-tauri`** (crate fora do workspace): `cargo check`/`cargo test` rodados de dentro do dir + `pnpm build`.

## Critérios de Aceitação

- **AC-1** — backend do dashboard compila com o agregador novo.
  Command: `cd apps/dashboard/src-tauri && cargo check`
- **AC-2** — React compila com o render de progresso por onda.
  Command: `pnpm -C apps/dashboard build`

<!-- wikilinks-footer-start -->
- [checklist-progresso-por-onda](?) ⚠ unresolved
- [wave-1-impl](?) ⚠ unresolved
- [wave-2-impl](?) ⚠ unresolved
<!-- wikilinks-footer-end -->