# Spec Hygiene Reference

> Detail for `/feature` — automatic spec audit before ANALYZE.

### Spec Hygiene (automatic, before ANALYZE)

Before starting a new pipeline, audit specs in `.claude/spec/` (flat layout — no `active/`/`completed/` buckets; status is the source of truth):

1. **Scan** all specs in `.claude/spec/*/spec.md`
2. **For each spec**, read the full header and checklist to extract `Status:`, `Phase:`, and checkbox completion (`[x]` vs `[ ]`). Filter by `Status:` (or the SQLite `pipeline_state_for_spec` projection) — `completed`/`cancelled` specs are skipped in step 4.
3. **Verify completed/cancelled specs:**
   - If `Status: completed` or `Status: cancelled`:
     - **Analyze first**: check that ALL checklist items are `[x]`, no `## Concerns` with unresolved `BLOCKED` items, and build/type-check references are satisfied
     - If analysis confirms done → flip status via `mustard-rt run complete-spec {name} --archive` (also emits `pipeline.status` and removes the `.diff.md` if present; the spec dir stays at `.claude/spec/{name}/` — no filesystem move), log: `[HYGIENE] Verified and archived {name}`
     - If analysis finds incomplete items → update `Status: implementing`, log: `[HYGIENE] {name} marked completed but has {N} unchecked items — reverted to implementing`, then treat as in-progress (step 4)
4. **In-progress specs** (`Status: draft` or `Status: implementing`):
   - Use `AskUserQuestion`: _"Found spec in progress: **{name}** (Status: {status}, Phase: {phase}, {done}/{total} tasks done). Do you want to continue this spec before starting a new one?"_
   - If **yes** → stop, suggest `/resume` to continue the existing spec
   - If **no** → proceed to ANALYZE for the new pipeline (existing spec stays at `.claude/spec/{name}/`)
5. **No active specs** → proceed to ANALYZE normally

This step is silent when there's nothing to audit — no output if no active specs are found.
