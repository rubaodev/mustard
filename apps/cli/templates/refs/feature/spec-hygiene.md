# Spec Hygiene Reference

> Detail for `/feature` — automatic spec audit before ANALYZE.

### Spec Hygiene (automatic, before ANALYZE)

Before starting a new pipeline, audit specs in `.claude/spec/` (flat layout — no `active/`/`completed/` buckets; status is the source of truth):

1. **Scan** all specs in `.claude/spec/*/spec.md`
2. **For each spec**, read the full header and checklist to extract `### Stage:`, `### Outcome:`, and checkbox completion (`[x]` vs `[ ]`). Filter by `### Outcome:` (or the SQLite `pipeline_state_for_spec` projection) — `Completed`/`Abandoned` specs are skipped in step 4.
3. **Verify Completed/Abandoned specs:**
   - If `### Outcome: Completed` or `### Outcome: Abandoned`:
     - **Analyze first**: check that ALL checklist items are `[x]`, no `## Concerns` with unresolved `BLOCKED` items, and build/type-check references are satisfied
     - If analysis confirms done → flip outcome via `mustard-rt run complete-spec {name} --archive` (also emits `pipeline.outcome` and removes the `.diff.md` if present; the spec dir stays at `.claude/spec/{name}/` — no filesystem move), log: `[HYGIENE] Verified and archived {name}`
     - If analysis finds incomplete items → update `### Stage: Execute` + `### Outcome: Active`, log: `[HYGIENE] {name} marked Completed but has {N} unchecked items — reverted to Execute`, then treat as in-progress (step 4)
4. **In-progress specs** (`### Outcome: Active` and `### Stage:` ≠ `Close`):
   - Use `AskUserQuestion`: _"Found spec in progress: **{name}** (Stage: {stage}, Outcome: {outcome}, {done}/{total} tasks done). Do you want to continue this spec before starting a new one?"_
   - If **yes** → stop, suggest `/resume` to continue the existing spec
   - If **no** → proceed to ANALYZE for the new pipeline (existing spec stays at `.claude/spec/{name}/`)
5. **No active specs** → proceed to ANALYZE normally

This step is silent when there's nothing to audit — no output if no active specs are found.
