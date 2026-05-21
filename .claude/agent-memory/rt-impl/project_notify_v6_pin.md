---
name: notify-v6-workspace-pin
description: Why mustard workspace pins notify to "6" (not latest 8.x) and where the dep graph constraint lives
metadata:
  type: project
---

`notify = "6"` no `[workspace.dependencies]` (root `Cargo.toml`) e em `apps/rt/Cargo.toml` desde 2026-05-21 (W3b da economia-moat-unification).

**Why:** `apps/dashboard/src-tauri/Cargo.toml` já pinava `notify = "6"` (via `notify-debouncer-mini = "0.4"`). Cargo lockfile mostrava `notify 6.1.1` como transitive dep do dashboard. Subir o workspace pin para `notify = "8"` (max_stable na crates.io em maio 2026) duplicaria o grafo de dep — dashboard ficaria em 6.x e rt em 8.x. Manter ambos no mesmo major preserva uma única resolução. API `notify::recommended_watcher`, `RecursiveMode::Recursive` e `EventKind::{Modify,Create}` são idênticos em 6.x e 8.x para o uso atual.

**How to apply:** Quando um bump do `notify` for proposto, verificar primeiro se o dashboard ainda usa `notify-debouncer-mini` ou pinou direto. Bump global só faz sentido junto com o dashboard (mesma forma como `rusqlite 0.31 → 0.39` foi planejada na eliminate-bun Wave 4).
