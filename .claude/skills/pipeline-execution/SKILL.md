---
name: pipeline-execution
description: Pipeline phases, dispatch rules, wave system, validate, retry. Use when running /feature, /resume, /approve or any pipeline phase requiring dispatch/wave context.
tags: [plan, any]
appliesTo: []
scope: [plan, code-editing]
metadata:
  generated_by: foundation
disable-model-invocation: true
source: manual
---

# Pipeline Execution Detail

> Phases, role rules, dispatch mechanics, validation, bugfix paths. Loaded on-demand.

## Pipeline Feature

### ANALYZE Phase (collapses old SYNC+UNDERSTAND+SCOPE+EXPLORE)

1. **MODEL:** `mustard-rt run scan` (produce/refresh `.claude/grain.model.json`).
2. Research via the scan digest — NEVER read the repo or the model whole. Run `mustard-rt run feature --intent "{request}"`; the insumos carry the matched slices/contracts/hubs + the anchor files. `miss: true` → no repo-vocabulary precedent (re-query with repo terms; treat true net-new as design). Otherwise infer layers from the matched slices.
3. Read ONLY the `anchors` the insumos point to (~12 real files), then ask `mustard-rt run scan spec` per unit.

| Signal                       | Layers                               |
| ---------------------------- | ------------------------------------ |
| New field/column/relation    | DB (+ Backend/FE if visible)         |
| New endpoint, business logic | Backend (+ FE if visible)            |
| New screen/component         | Frontend (+ Backend if new endpoint) |
| New CRUD / sub-entity        | DB + Backend + Frontend              |
| Refactoring, bug fix         | Root cause layer(s)                  |

When in doubt → `AskUserQuestion`: "Which layers?"

**Scope Detection:**

| Signal                                             | → Scope   |
| -------------------------------------------------- | --------- |
| 1-2 layers, ≤5 files, known pattern, no new entity | **Light** |
| 3+ layers, 5+ files, new entity/CRUD, new pattern  | **Full**  |

**Explore (conditional):**

- Entity in registry → SKIP Explore, read 2-3 reference files directly
- Entity NOT in registry → Explore agent ("medium"), then straight to PLAN
- **MAX 5 file reads in ANALYZE** (registry/pipeline-config are free)

### PLAN Phase (collapses old SPEC)

Create `.claude/spec/{date}-{name}/spec.md`:

- **Full scope:** Summary, Entity Info, Files, Tasks (by wave), Dependencies. Each wave's PLAN must declare its target files — `wave-scaffold` seeds that wave's trackable checklist from them into the wave's `meta.json` (one `{label, path, done: false}` item per file). The wave-plan parent is a coordination doc and carries no checklist.
- **Light scope:** Summary (1-2 lines), Checklist (tasks by agent, no waves).

Lifecycle state (stage/phase/scope/checkpoint) lives ONLY in the `meta.json` sidecar — never write `Status:` / `Phase:` / `Scope:` / `Checkpoint:` header lines into the markdown; the spec.md stays pure narrative.
Create `.claude/.pipeline-states/{spec-name}.json`.

**Light Scope → Inline Path:** When `/feature` detects Light scope and user approves inline, EXECUTE runs in same session. No PLAN phase needed.

### EXECUTE Phase (collapses old IMPLEMENT+VALIDATE+REVIEW)

**1. Skills Auto-Loading:**

Agents auto-load relevant skills from `{subproject}/.claude/skills/` based on task description.
The subproject's curated Guards are injected inline by the renderer (`## GUARDS`); there is no `{recommended_skills}` hint block (generated skills were removed).

**2. Plan Waves — routed by Rust, the LLM relays:**

Do NOT read `wave-plan.md` or decide the wave order by hand. Run:

```bash
mustard-rt run wave-advance --spec {specName}
```

It returns the **current dispatch round** as a deterministic JSON array — every wave of the first dependency level whose waves lack `pipeline.wave.complete`; `[]` when all waves are done. Each item is `{wave, role, subproject, subagent_type, prompt}` with the `prompt` **already rendered**:

- Items returned together have no dependency between them → dispatch them together in ONE message (multiple `<invoke>` blocks). Re-run `wave-advance` after the round completes — a higher level starts ONLY after every lower-level wave completes.
- NEVER nest dispatch — nesting breaks parallel execution.
- `resume-bootstrap` decides the **stage**; `wave-advance` decides the **wave routing + render** (`dispatch-plan` remains as an inspection view of the full DAG/levels). The orchestrator is a relay over the array, not a planner.

**3. Dispatch Agent:**

For each item, pass its `prompt` **verbatim** to the Task `prompt` (it was already rendered by `agent-prompt-render` inside `wave-advance` — never hand-assembled) with the item's **`subagent_type`** (the tool picks it per role: read-only roles run tool-restricted — `explore`→`Explore`, `review`/`qa`→`mustard-review`, `guards`→`mustard-guards`, so they physically cannot write; writing roles → `general-purpose`). Never pick the agent by hand. The rendered template carries the role contract + boundary + return cap inline, plus the spec's project section + its anchors.

**4. Validate:**

- Build passes (backend: `dotnet build`, frontend: `pnpm build`, mobile: `fvm flutter analyze`)
- Zero critical guard violations
- Checklist marking is automatic: the `checklist-auto-mark` hook runs after every Edit/Write (incremental, silent). Wave specs are meta-first: the hook flips the matching item to `done: true` in the wave's `meta.json#checklist` (matched by the item's `path`/basename) and emits a `checklist.item.marked` NDJSON event — the checklist seeded from the wave's target files is auto-markable by construction. Markdown `## Checklist` sections (Light scope, or legacy specs without a meta checklist) are still marked in place — give each item a file hint: include the file basename in the item text (e.g. `- [ ] Validate UserService.cs`) or append a target arrow (e.g. `- [ ] Validate input → src/Services/UserService.cs`). Items without any hint won't auto-mark; close-gate will surface them at CLOSE.
- Any failure → retry (max 2/agent), then STOP + replan

**6. Review (MANDATORY — NEVER skip):**
Dispatch review agent for EACH affected subproject. The review agent reads `{subproject}/CLAUDE.md` — the `## Guards` section carries the subproject's DO/DON'T rules — and runs the full 7-category checklist:

1. **SOLID** — SRP, OCP, LSP, ISP, DIP
2. **Design System** — tokens, typography, spacing, components, icons, theme
3. **Patterns** — project conventions from the subproject's `## Guards`
4. **i18n** — all strings localized, all locale files updated
5. **Integration** — types synced, no orphans, no circular deps
6. **Build** — compiles/analyzes clean
7. **Elegance** — simplest solution, no over-engineering

APPROVED (zero CRITICAL) → CLOSE. REJECTED (any CRITICAL) → fix agent dispatched (max 2 fix loops), then re-review.

### CLOSE Phase (collapses old COMPLETE)

1. `mustard-rt run scan` (refresh `grain.model.json` if the codebase changed)
2. Checklist must already be fully done from EXECUTE — `close-gate` consolidates every wave's `meta.json#checklist` (markdown `## Checklist` is the legacy fallback) and blocks CLOSE while any item is unmarked. Never write lifecycle headers (`Status:` / `Phase:`) into the markdown — `meta.json` is synced by the close itself (step 3).
3. Run `mustard-rt run close-orchestrate --spec {name}`. When `overall == pass` it **auto-chains the finalize in-process** (flips the spec straight to `completed`, emits + verifies `pipeline.complete`, syncs `meta.json` to Close/Completed); the LLM does not call `complete-spec` itself. When `overall == fail` it is report-only — fix the failing gate and re-run. (A close lands straight on `completed` — there is no follow-up grace window; follow-up work goes into a separate linked sub-spec. No filesystem move — spec dir stays at `.claude/spec/{name}/`.)
4. Output with agent colors: `═══ PIPELINE COMPLETE — {name} | Agents: {n} ok | Files: {c} created, {m} modified ═══`

### Replan Protocol

When: agent FAILED structurally, retry exhausted, user reports unexpected behavior, review REJECTED with architectural concern.
Steps: update spec → summarize failure → Explore → rewrite tasks → re-approve → resume EXECUTE.

## Role Rules

> See `.claude/pipeline-config.md § Role Rules` for role boundaries and validation rules.

## Pipeline Bugfix

### Fast Path (1-2 files, clear cause)

ANALYZE → FIX → VALIDATE → CLOSE. No spec needed.

### Full Path (3+ files, unclear impact)

ANALYZE → PLAN → APPROVE → FIX → VALIDATE → CLOSE.

### Decision

Explore returns clear root cause in 1-2 files → Fast Path. Otherwise → Full Path.
