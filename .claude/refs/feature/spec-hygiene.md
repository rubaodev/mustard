# Spec Hygiene Reference

> Detail for `/feature` — automatic spec audit before ANALYZE.

### Spec Hygiene (automatic, before ANALYZE)

Before starting a new pipeline, audit specs under the flat `spec/` layout (status comes from the header, not from a bucket directory):

1. **Scan** all specs in `.claude/spec/*/spec.md`
2. **For each spec**, read the full header and checklist to extract `Status:`, `Phase:`, and checkbox completion (`[x]` vs `[ ]`)
3. **Verify completed/cancelled specs:**
   - If `Status: completed` or `Status: cancelled`:
     - **Analyze first**: check that ALL checklist items are `[x]`, no `## Concerns` with unresolved `BLOCKED` items, and build/type-check references are satisfied
     - If analysis confirms done → emit `mustard-rt run emit-pipeline --kind pipeline.status --spec {name} --payload '{"status":"completed"}'` and delete `.diff.md` if it exists (the spec directory stays at `.claude/spec/{name}/` — there is no longer a `completed/` bucket to move into; archival is semantic-only via the `pipeline.status` event), log: `[HYGIENE] Verified {name} as completed`
     - If analysis finds incomplete items → update `Status: implementing`, log: `[HYGIENE] {name} marked completed but has {N} unchecked items — reverted to implementing`, then treat as in-progress (step 4)
4. **In-progress specs** (`Status: draft` or `Status: implementing`):
   - Use `AskUserQuestion`: _"Found spec in progress: **{name}** (Status: {status}, Phase: {phase}, {done}/{total} tasks done). Do you want to continue this spec before starting a new one?"_
   - If **yes** → stop, suggest `/resume` to continue the existing spec
   - If **no** → proceed to ANALYZE for the new pipeline (existing spec stays at `.claude/spec/{name}/`)
5. **No in-progress specs** → proceed to ANALYZE normally

This step is silent when there's nothing to audit — no output if no spec is in progress.
