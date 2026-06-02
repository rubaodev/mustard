---
name: mustard-maint
description: Use when the user runs /maint or asks about dependencies, validate, sync, or doctor (project hygiene, build/type-check, registry sync, installation health check).
source: manual
---
<!-- mustard:generated -->
# /maint - Maintenance Utilities

## Trigger

`/maint <action>`

| Action | Description |
|--------|-------------|
| `deps` | Install dependencies for all subprojects (or root if single repo) |
| `validate` | Build + type-check across subprojects |
| `sync` | `mustard-rt run scan` — refresh `grain.model.json` |
| `doctor` | Installation health check — wiring, drift, state + OTEL diagnostics |

## deps / validate

Read `.claude/pipeline-config.md § Agents` for subproject paths + commands (from `{subproject}/CLAUDE.md § Commands`). Run all in parallel. Single repo: read root `CLAUDE.md § Commands`.

## sync

`mustard-rt run scan`. Use after creating new entities, importing code, or major edits.

## doctor

Consolidated check — never blocks, reports only.

```bash
mustard-rt run doctor              # wiring + drift + pipeline-state health
mustard-rt run doctor --residue    # also scan for dead file/script references
mustard-rt run diagnose-otel       # OTEL telemetry pipeline health
```

Print all three as one consolidated report. Categories: `wiring`, `drift`, `state-health`, `residue` (`--residue`) — each OK / WARN / FAIL.

Run periodically, after `mustard update`, after telemetry looks wrong, or when hooks appear to skip silently.
