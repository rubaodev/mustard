<!-- mustard:generated -->
# Guards — `packages/core`

<!-- mustard:enrich hash=80d4ddd9de35 -->
## Purpose

These guards encode the conventions `packages/core` already follows so a new change matches the established shape. `mustard-core` is the pure, agnostic kernel: it layers as `domain` (typed owners of on-disk state — `mustard.json`; the repo model lives in `.claude/grain.model.json`, produced by `mustard-rt run scan` and read only via the scan tool or `mustard_core::read_entity_names`/`read_projects`) → `view` (folds + render-facing view models) over `platform` primitives (the `Error` type, fail-open helpers, enforcement config) and `io`. The dominant rules across the codebase are fail-open (a missing or malformed input degrades to a default, never a panic) and stack-agnosticism (detection runs the same code for every language via tree-sitter with a textual floor and Aho-Corasick vocabularies). Honour the layer direction, keep new modules pure where the cluster is pure, and match the per-cluster conventions documented under `.claude/skills/`.
<!-- /mustard:enrich -->

> Deterministic seed. `/scan --enrich` appends project-specific DO/DON'T inferred from real files.

## Architecture: layered
- DON'T let a lower layer import a higher one — keep layer dependencies one-directional.

## Frameworks detected
- DO follow the di conventions.
- DO follow the framework conventions.
- DO follow the orm conventions.

## Follow the discovered conventions
- DO match the `domain` convention (6 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `ast` convention (9 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `economy` convention (7 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `sources` convention (4 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `model` convention (5 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `view` convention (7 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `regression_check` convention (3 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `skill` convention (3 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `spec` convention (3 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `vocabulary` convention (5 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `io` convention (3 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `atomic_md` convention (4 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `events` convention (3 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `fs` convention (3 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `platform` convention (7 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `projection` convention (6 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `tests` convention (3 files) — a generated skill under `.claude/skills/` documents it.

