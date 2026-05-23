# Spec Hygiene Reference

> Detail for `/feature` — automatic spec audit before ANALYZE.

### Spec Hygiene (automatic, before ANALYZE)

Before starting a new pipeline, audit specs under the flat `spec/` layout (status comes from the header, not from a bucket directory):

1. **Scan** all specs in `.claude/spec/*/spec.md`
2. **For each spec**, read the full header and checklist to extract `### Stage:`, `### Outcome:`, and checkbox completion (`[x]` vs `[ ]`)
3. **Verify Completed/Abandoned specs:**
   - If `### Outcome: Completed` or `### Outcome: Abandoned`:
     - **Analyze first**: check that ALL checklist items are `[x]`, no `## Concerns` with unresolved `BLOCKED` items, and build/type-check references are satisfied
     - If analysis confirms done → emit `mustard-rt run emit-pipeline --kind pipeline.outcome --spec {name} --payload '{"outcome":"Completed"}'` and delete `.diff.md` if it exists (the spec directory stays at `.claude/spec/{name}/` — there is no longer a `completed/` bucket to move into; archival is semantic-only via the `pipeline.outcome` event), log: `[HYGIENE] Verified {name} as Completed`
     - If analysis finds incomplete items → update `### Stage: Execute` + `### Outcome: Active`, log: `[HYGIENE] {name} marked Completed but has {N} unchecked items — reverted to Execute`, then treat as in-progress (step 4)
4. **In-progress specs** (`### Outcome: Active` and `### Stage:` ≠ `Close`):
   - Use `AskUserQuestion`: _"Found spec in progress: **{name}** (Stage: {stage}, Outcome: {outcome}, {done}/{total} tasks done). Do you want to continue this spec before starting a new one?"_
   - If **yes** → stop, suggest `/resume` to continue the existing spec
   - If **no** → proceed to ANALYZE for the new pipeline (existing spec stays at `.claude/spec/{name}/`)
5. **No in-progress specs** → proceed to ANALYZE normally

This step is silent when there's nothing to audit — no output if no spec is in progress.
