---
name: mustard-feature
description: Use when the user runs /feature or asks to add, create, implement, or enhance a new feature. Starts the feature pipeline (ANALYZE → PLAN → optional inline EXECUTE for Light scope).
source: manual
---
<!-- mustard:generated -->
# /feature - Feature Pipeline

`/feature <request>` — understand the client, research the repo through the deterministic `scan` tool (never reading source by hand), then plan + implement.
Heavy lifting delegates to `mustard-rt`; the orchestrator routes phases + emits events. You (the AI) do the reasoning that grain cannot: elicitation, decomposition, lapidation.

## Action

### 1. Understand + RESEARCH (ANALYZE)

→ `../../../refs/feature/spec-hygiene.md`. Emit `pipeline.stage: Analyze`.

- **Note the client's intent** in your own words (what they actually want, plus every concrete critique from the conversation).
- **Ensure the model exists/is fresh**: `mustard-rt run scan` produces`.claude/grain.model.json` (the durable repo model). Run it if absent or the codebase changed materially.
- **Research via the scan digest, not the repo**: `mustard-rt run feature --intent "<request>"` → structured *insumos* (`matchedTerms`, `slices`, `contracts`, `hubs`, `anchors`, `miss`, `note`). Query again with **repo-vocabulary** terms if the first pass is a `miss`
  (the term index has no synonyms and false negatives — e.g. a PT request maps
  to EN code terms). NEVER conclude "absent" from a `miss` alone.
- **Read ONLY the `anchors`** the insumos point to (~12 real files) — never the whole repo, never `grain.model.json` directly. This is the low-consumption contract.

Scope: light (1-2 layers, ≤5 files, mirrors a matched slice) | extended-light (matched slice + modifies existing + ≤8 files) | full (3+ layers, net-new or spans multiple slices). MAX 5 reads beyond the anchors in ANALYZE.

End: `rtk mustard-rt run analyze-validation --spec .claude/spec/{spec}/spec.md` — append `issues[]` to `## Concerns` on `ok: false` (non-blocking).

### 2. DECOMPOSE

From the insumos + anchors, split the request into three natures (this is the judgement grain cannot make):

- **Units with precedent** → each maps to a matched slice; you will ask `scan spec` for each (with `--like <sibling>` when one exists).
- **Cross-cutting invariants to obey** → contracts/hubs the repo already enforces (e.g. an injected `ICurrentTenant`); pass each via `scan spec --invariant <Name>` so the draft anchors the real wiring. NEVER invent the mechanism — mirror the anchored consumers.
- **Net-new gaps** (no precedent; `miss` after a repo-vocabulary re-query) → surface as a design decision; do not let a `scan spec` draft's "deterministic" framing imply a unit that has no precedent is safe to clone.

### 3. PLAN

→ `../../../refs/feature/spec-language.md` (header translation, narrative rules, Component Contract). → `../../../refs/feature/wave-decomposition.md`.

Resolve Lang via cascade (`meta.json#lang` → `mustard.json#specLang` → AskUserQuestion once → persist to `meta.json`).

**Per unit**, compile the deterministic draft: `mustard-rt run scan spec --entity {Unit} [--like {Sibling}] [--invariant {Contract}] [--ops create,...]`
→ a draft carrying the auto-chosen pattern menu, per-project sections, the anchors, and acceptance criteria. **Lapidate** it: read the draft + its anchors + the client request, resolve the bifurcation, prune, add domain rules, mark
assumptions — and write the final spec **in the project's language/tone** (`mustard.json#specLang`/`tone`). The draft's `## Projeto` sections ARE the wave/agent decomposition.

**Concern Coverage Audit**: every concrete user critique must map to covered by wave/task | non-goal justified | surfaced for decision. Orphaned items block the AskUserQuestion. Full scope: wave decomposition when `file_count ≥ 6 OR layer_count ≥ 3 OR independent_subbehaviors ≥ 3` — `mustard-rt run wave-scaffold --spec-dir <dir> --plan <plan.json>`.

Write `.claude/spec/{date}-{name}/spec.md` two-layer: `## PRD` → `## Contexto`, `## Usuários/Stakeholders`, `## Métrica de sucesso`, `## Não-Objetivos`, `## Critérios de Aceitação`; `## Plano` → `## Entidades`, `## Arquivos`, optional `## Component Contract` (UI only), `## Tarefas`, `## Dependências`, `## Limites`.

Emit `pipeline.scope` + `pipeline.stage: Plan`. Print spec verbatim + `wave-tree`. AskUserQuestion: **"Approve and implement?"** / **"Adjust (give feedback)"** / **"Save for later (stop)"**.

### 4. Light/Extended-Light EXECUTE (inline)

User chooses "Approve and implement now": emit `pipeline.stage: Execute` → `exec-rewave-check` (decomposed → use wave-1 spec) → `dependency-precheck` (block on missing externals) → dispatch agents via `agent-prompt-render` (NEVER hand-craft; all agents of a wave → one message; the subagent's context is the spec's project section + its anchors) → per-wave validate + `memory agent` → REVIEW per subproject (sonnet for re-reviews, `review-result` emit, max 2 fix loops) → QA (`qa-run`; pass → CLOSE; fail → return failing AC; skip → warn + allow CLOSE; max 3 QA iterations).

Escalations: `CONCERN` → `## Concerns`, continue. `BLOCKED` → STOP + AskUserQuestion. `PARTIAL` → granular retry (max 2). `DEFERRED` → note + confirm. → `../../../refs/feature/ac-cross-shell.md`.

## INVIOLABLE RULES

- ALWAYS research via `mustard-rt run feature` (the scan digest) — NEVER read the repo or `grain.model.json` to understand it.
- READ ONLY the `anchors` the scan tools point to (~12 files). NEVER bulk-read source.
- NEVER hand-craft agent prompts — always `agent-prompt-render`; the subagent's context is the spec section + anchors (there are no generated skills/agents).
- ALWAYS compile each unit's draft with `mustard-rt run scan spec`, then lapidate in the project's language (`mustard.json#specLang`/`tone`).
- A `miss` is NOT "absent": re-query with repo-vocabulary terms; treat true net-new as DESIGN, not recomposition.
- NEVER skip `analyze-validation` or `dependency-precheck`.
- ALWAYS emit `pipeline.scope` + `pipeline.stage` at each transition.
