# Task Dispatch — Render Invocations Reference

> Detail for `/task`: the concrete `agent-prompt-render` (+ `context-slice`) invocations per action. Prompts are **never hand-assembled** — this is the same inviolable rule `/feature` and `/tactical-fix` follow. The orchestrator runs the render command, then passes its **stdout verbatim** as the Task `prompt`. With `--emit ref` that stdout is a 2-line stub (the full prompt goes to a `.dispatch/` file) — still verbatim; the PreToolUse hook expands it at dispatch, so the full text never transits the orchestrator's context.

`/task` is spec-less: there is no `wave-plan.md` and no `dispatch-plan`. The render is driven directly by the action (`--role`) and the scope (`--spec` / `--subproject`). Every placeholder fail-opens, so a spec-less invocation is safe — empty slots simply render blank.

## Step 1 — slice the subproject CLAUDE.md + glossary (once per scope)

`context-slice` produces the `{context_md}` slice that `agent-prompt-render` injects. It is relevance-filtered — no size cap; only the blocks/sections whose terms match the active spec are kept (every matched one, in full). The subproject `## Guards` ride in separately as `{guards_summary}` — they are *not* a `--context` source here.

```bash
# If a domain glossary exists for the scope, append: --context {subproject}/CONTEXT.md
mustard-rt run context-slice --spec {scope} \
  --context-claude-md {subproject}/CLAUDE.md
```

The slice is cached at `.claude/.pipeline-states/{scope}.context-md.md`; `agent-prompt-render` reads it back as `{context_md}`. No `CONTEXT.md` glossary authored → empty slice → `{context_md}` blank (dispatch never blocks). A `--context` path that is named but missing is reported on stderr (caller misconfiguration), distinct from the blank-by-design case.

## Step 2 — render the dispatch prompt per action

`--mode first` is the dispatch (non-retry) render; swap to `--mode granular` / `--mode fix-loop` on a retry. No size budget — relevance is the only filter on what the renderer injects (the spec-memory gate, the relevance-sliced context); nothing is trimmed by token count.

| Action | `--role` | `subagent_type` | Render invocation |
|--------|----------|-----------------|-------------------|
| `analyze` | `explore` | `Explore` | `mustard-rt run agent-prompt-render --spec {scope} --role explore --subproject {subproject} --mode first --emit ref` |
| `review` | `review` | `mustard-review` | `mustard-rt run agent-prompt-render --spec {scope} --role review --subproject {subproject} --mode first --emit ref` |
| `docs` | `docs` | `general-purpose` | `mustard-rt run agent-prompt-render --spec {scope} --role docs --subproject {subproject} --mode first --emit ref` |
| `audit` | `audit` | `general-purpose` | `mustard-rt run agent-prompt-render --spec {scope} --role audit --subproject {subproject} --mode first --emit ref` |
| `refactor` (plan) | `plan` | `Plan` | `mustard-rt run agent-prompt-render --spec {scope} --role plan --subproject {subproject} --mode first --emit ref` |
| `refactor` (execute) | `implement` | `general-purpose` | `mustard-rt run agent-prompt-render --spec {scope} --role implement --subproject {subproject} --mode first --emit ref` |
| `implement` | `implement` | `general-purpose` | `mustard-rt run agent-prompt-render --spec {scope} --role implement --subproject {subproject} --mode first --emit ref` |

### Dispatch shape

For each rendered prompt:

```text
Task({
  subagent_type: <from table>,   // per role: read-only roles run tool-restricted
  description: `{action}: {scope}`,
  prompt: <stdout of agent-prompt-render, verbatim>
})
```

No `model` field — dispatched agents inherit the session model (`pipeline-config.md § Model`).

Every render also passes `--task-text "<the action's work>"` — `/task` is spec-less, so the action's task rides in via that flag (the renderer folds it into `## TASK`); never hand-append the task after the render.

`subagent_type` is picked per role: `explore`→`Explore`, `review`→`mustard-review` (both read-only — no Edit/Write); writing roles (`audit` / `docs` / `implement`) → `general-purpose`. The render carries the role contract inline.

## Per-action notes

- **audit** — first load the `improve-codebase-architecture` skill and select the domain checklist (`copy` / `design` / `a11y` / `i18n` / `consistency` / `api-contract`; default `consistency`). The checklist is the *task description* the auditor works through; the rendered prompt carries the guards + standardization context.
- **compare** — render one prompt **per subproject** (each with its own `--subproject`, `--role explore`) and dispatch them PARALLEL in a single message. Then render a consolidation prompt (`--role plan`) that merges the explorer results and surfaces discrepancies.
- **refactor** — two-phase: render+dispatch the `plan` role, print the plan verbatim, AskUserQuestion (Approve / Adjust / Cancel), then on approval render+dispatch the `implement` role.
- **implement** — single dispatch, return cap ≤30 lines (Files Changed / Build result / Status). ON CONCERN → surface + offer `/feature` Light.

Persistent tracking is **N/A** — `/task` is spec-less by design. Promote to `/feature` Light or `/tactical-fix` if a tracked spec is needed.
