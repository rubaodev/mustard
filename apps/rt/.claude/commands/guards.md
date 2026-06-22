<!-- mustard:generated -->
# Guards — `apps/rt`

<!-- mustard:enrich hash=54b47418248a -->
## Purpose

Guard rails for `apps/rt`, the `mustard-rt` runtime that hosts the harness hooks and run-commands. Layering runs one-directional from `commands`/`hooks` down through `shared` into `mustard-core`, so a lower layer must never import a higher one. Hooks are split by contract: `observer` modules are observe-only and must never block, while `inject`/`Check` modules return a `Verdict` and may inject context — keep every IO step fail-open so a missing file or unreadable directory degrades to a no-op rather than breaking the pipeline.
<!-- /mustard:enrich -->

> Deterministic seed. `/scan --enrich` appends project-specific DO/DON'T inferred from real files.

## Architecture: layered
- DON'T let a lower layer import a higher one — keep layer dependencies one-directional.

## Frameworks detected
- DO follow the di conventions.
- DO follow the framework conventions.
- DO follow the orm conventions.

## Follow the discovered conventions
- DO match the `observer` convention (15 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `inject` convention (7 files) — a generated skill under `.claude/skills/` documents it.

