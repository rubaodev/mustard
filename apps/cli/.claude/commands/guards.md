<!-- mustard:generated -->
# Guards — `apps/cli`

<!-- mustard:enrich hash=eaf3ef0bdf70 -->
## Purpose
DO/DON'T rules for the `mustard-cli` crate, grounded in how its commands actually behave: write the single config to `<root>/mustard.json` (never `.claude/mustard.json`), keep `.claude` in the `copy_dir` top-level skip list to avoid `.claude/.claude/` nesting, probe external tools fail-open (`git`/`npm`/`mustard-rt` failures degrade with hints rather than abort), validate template and skill names against `[A-Za-z0-9_-]` and reject `..` before any filesystem or network use, merge JSON surgically (fail-open read, `entry().or_insert_with()`, atomic write) so user keys survive, and keep all logic in the library with no `unwrap`/`expect` outside `#[cfg(test)]`.
<!-- /mustard:enrich -->

> Deterministic seed. `/scan --enrich` appends project-specific DO/DON'T inferred from real files.

## Architecture: layered
- DON'T let a lower layer import a higher one — keep layer dependencies one-directional.

## Frameworks detected
- DO follow the di conventions.
- DO follow the framework conventions.
- DO follow the orm conventions.

## Follow the discovered conventions
- DO match the `src` convention (4 files) — a generated skill under `.claude/skills/` documents it.
- DO match the `commands` convention (9 files) — a generated skill under `.claude/skills/` documents it.

