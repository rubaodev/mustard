---
name: mustard-close
description: Use when the user runs /close or asks to finalize, complete, or cancel the active pipeline. Verifies build/review/QA, archives the spec, and emits the completion banner.
source: manual
---
<!-- mustard:generated -->
# /close - Finalize Pipeline

## Trigger

`/close`

## Verification Gate (MANDATORY)

Each step blocks CLOSE on failure (fail-open only on script missing): (1) **Review** ran + APPROVED, zero CRITICAL findings; (2) **Build + tests** — `mustard-rt run verify-pipeline` (exit 1 → stop + report); (3) **QA** — `mustard-rt run qa-run --spec {spec}` (fail → list failing AC + STOP; skip → warn + continue); (4) **Docs audit** — `mustard-rt run docs-stale-check` (hits > 0 + `MUSTARD_DOCS_AUDIT_MODE=strict` → block; default warns + continues); (5) **Concerns** — unresolved `BLOCKED` → block; `CONCERN`/`DEFERRED` → surface + proceed; (6) **Checklist** — any `- [ ]` remaining → ABORT + report unmarked items.

## Action

1. Locate spec at `.claude/spec/{name}/`. Status from header + SQLite projection.
2. Update header: Stage `Close`, Outcome `Completed`, Checkpoint `{ISO now}`.
3. `mustard-rt run sync-registry` if `## Files` touched schemas.
4. Two-stage close (events only):

```bash
mustard-rt run complete-spec <spec-name>
mustard-rt run emit-pipeline --kind pipeline.stage --spec {spec} --payload "{\"stage\":\"Close\"}"
mustard-rt run emit-pipeline --kind pipeline.outcome --spec {spec} --payload "{\"outcome\":\"Completed\"}"
mustard-rt run emit-pipeline --kind pipeline.flag.set --spec {spec} --payload "{\"flag\":\"followup_open\"}"
```

5. Knowledge: one `mustard-rt run memory knowledge` per significant pattern; one `mustard-rt run memory decision` per lesson (max 3 each, skip trivial).
6. Metrics archive: read pipeline-state projection → save to `.claude/metrics/{spec}.json` (omit missing fields).
7. Print: `pipeline-summary` → `wave-tree` → banner `PIPELINE COMPLETE — {spec}` with agents/files/registry + optional `rtk gain` token line. All fail-open.
8. Epic auto-fold (Wave 8): `epic-fold --detect` → if non-empty, `epic-fold --epic <name>` per entry.

## Cancellation

Stage `Close`, Outcome `Cancelled`. Emit `pipeline.stage: Close` + `pipeline.outcome: Cancelled`. No filesystem move.

## INVIOLABLE RULES

- NEVER bypass the verification gate.
- NEVER move the spec directory — archival is event-only.
- NEVER batch-mark Checklist items on behalf of agents.
- Re-reviews always dispatch with `model: "sonnet"`.
