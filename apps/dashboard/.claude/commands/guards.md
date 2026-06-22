<!-- mustard:generated -->
# Guards — `apps/dashboard`

<!-- mustard:enrich hash=62c46b8ceadb -->
## Purpose

Project-specific DO/DON'T guards for the `dashboard` subproject — a React 19.1 + Tailwind 4.3 + TypeScript 5.8 Tauri app. The dominant invariants are layering ones: components must not call `invoke()` directly but go through the typed Tauri wrappers in `src/api/` and `src/lib/`, and data fetching/aggregation belongs in the `useXxx` hooks under `src/hooks/` (TanStack Query) rather than in views. The two strongest discovered conventions are the `use` hook cluster (36 files) and the `tauri` binding cluster, each documented by a generated skill under `.claude/skills/`.
<!-- /mustard:enrich -->

> Deterministic seed. `/scan --enrich` appends project-specific DO/DON'T inferred from real files.

## Architecture: layered
- DON'T let a lower layer import a higher one — keep layer dependencies one-directional.

## Frameworks detected
- DO follow the framework conventions.

## Follow the discovered conventions
- DO match the `use` convention (36 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `tauri` convention (3 files) — a generated skill under `.claude/skills/` documents it.

