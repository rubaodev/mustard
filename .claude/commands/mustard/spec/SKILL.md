---
name: mustard-spec
description: Use when the user wants to approve a planned spec or continue an in-progress spec. Single picker — delegates to mustard-rt run active-specs and resume-bootstrap.
source: manual
---
<!-- mustard:generated -->
# /mustard:spec — Unified Spec Picker

`/mustard:spec [alvo]` — replaces `/approve` (PLAN) and `/resume` (EXEC). `alvo` is a **picker letter** (`a`-`z` from the table) OR a **spec name** (the slug). No `alvo` → render the table to pick. A spec name jumps **straight to that spec — no table**. Letter + `r` (e.g. `ar`) is a power shortcut = approve + execute inline, skipping the question.

## Action

### 0. Parse `alvo` — letter vs name vs empty

- **Empty** → picker mode: render the table (§1), wait for a letter.
- **Matches `^[a-z]r?$`** → letter mode: render the table (§1), map the row's letter to its spec name, then route (§2). A trailing `r` pre-answers the §2 PLAN question as **approve + implement inline** (no question shown).
- **Anything else** → **focused mode**: `alvo` IS the spec name. **SKIP the table — do NOT run `active-specs`, do NOT print Siglas/Modo.** Route directly (§2). No `r` parsing here (a slug may legitimately end in `r`); the approve-vs-implement choice comes from the §2 PLAN question.

### 1. Picker render (picker + letter modes only — FORBIDDEN in focused mode)

```bash
rtk mustard-rt run active-specs --format table
```

Print stdout verbatim, then the **Siglas + Modo de seleção** block below literally.

**Siglas** — `#` letter (a-z), `Esc` Scope (`lt` light / `fl` full / `-`), `Prog` waves done/total. Stage `PLAN` planejar / `EXEC` executar. Status `TF` tactical-fix, `TF→{alias}` TF parent, `W{N}` wave N, `BLOCK` blocked, `em exec` dispatched, `-` none.

**Modo de seleção** — `a-z` act on row (PLAN approve / EXEC continue). `a-z+r` (e.g. `ar`) approve + execute inline (EXEC ignores `r`). A spec name jumps straight to it (no table). Anything else → error + re-render.

### 2. Resolve + route via `resume-bootstrap`

Letter mode: map the picked letter to its `active-specs` row → `{specName}`. Focused mode: `{specName}` = `alvo` verbatim. Then:

```bash
rtk mustard-rt run resume-bootstrap --spec {specName} --json
```

Parse: `stage`, `mode`, `operationalSpecPath`, `currentWave`, `totalWaves`, `specSummary`, `lastDispatchFailure`, `needsDiff`, `needsContextSlice`.

- **`Plan`** → `../../../refs/spec/approve-only-flow.md`. It owns the focused single-spec render **and** the one approve/implement question (primary = approve + implement inline; secondary = approve only / new session). A letter-mode `r` suffix **pre-answers** that question as approve + implement inline (skip the question).
- **`Execute`/`Analyze`/`QaReview`/`Close`** → `../../../refs/spec/resume-flow.md`. In focused mode, first print a one-line header (`{specName} — retomando (EXEC)`; precise wave numbering comes from `wave-tree`/`dispatch-plan`, not from `currentWave`, which is 0-based) and ask a single **"Implementar agora?"** confirm before dispatch; letter mode (and a letter-mode `r`) skip the confirm. (EXEC ignores `r`.)

#### EXEC branch — `wave-advance` relay

Routing/order is decided by Rust, not the LLM. Get the current dispatch round, prompts already rendered:

```bash
rtk mustard-rt run wave-advance --spec {specName}
```

It returns `[{wave, role, subproject, subagent_type, prompt}]` — every wave of the first dependency level still lacking `pipeline.wave.complete`; once every impl wave is complete, the rendered **review round** (one `role: review` / `mustard-review` item per touched subproject — after each returns, the orchestrator records the verdict via `mustard-rt run review-result --spec {specName} --verdict ... --subproject {sub}`); `[]` only after every touched subproject carries a `review.result`. Each item's `prompt` IS the final Task prompt (already rendered by `agent-prompt-render` — no `prompt_cmd` round-trip). Items returned together are independent → dispatch them all in **one** message. `subagent_type` = each item's `subagent_type` field — the tool picks the agent per role (read-only roles run tool-restricted: `explore`→`Explore`, `review`/`qa`→`mustard-review`, `guards`→`mustard-guards`; writing roles → `general-purpose`). NEVER hand-craft prompts, pick the agent by hand, or interpret `wave-plan.md` by hand. (`dispatch-plan` still exists — an inspection fallback for the full DAG/levels, not the dispatch path.) Post-dispatch → `../../../refs/spec/resume-flow.md`.

### 4. Edge cases

0 specs → *"Nenhuma spec ativa."*. >26 → first 26 + *"(N adicionais)"*. Letter `r` shortcut (`/mustard:spec ar`) → pre-answers the PLAN question, skip re-render. Focused mode (a spec name) → never render the picker table; if `resume-bootstrap` errors (unknown slug), say *"Spec '{alvo}' não encontrada."* and render the table (§1) as a fallback.

## INVIOLABLE RULES

- Picker table + Siglas + Modo blocks are mandatory + literal in **picker/letter mode**; in **focused mode** (a spec name was passed) they are **FORBIDDEN** — render only that one spec.
- A bare spec name routes **directly** to that spec — NEVER list all specs first to "find" it. `resume-bootstrap`/`approve-spec` are name-addressable; `active-specs` exists only for letter picking.
- A PLAN-stage spec gets **one** question (approve + implement now / approve only / …); NEVER approve-then-tell-the-user-to-re-run as the default — that round-trip is the `approve only — new session` secondary option, not the primary path.
- NEVER hand-craft agent prompts — `wave-advance` delivers each item's `prompt` already rendered by `agent-prompt-render`.
- NEVER read `wave-plan.md` or decide wave order by hand — `wave-advance` owns routing (`dispatch-plan` = inspection fallback); the LLM only relays.
- NEVER reimplement `continued` vs `reanalyzed` — `resume-bootstrap` is the source of truth.
