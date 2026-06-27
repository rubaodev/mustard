---
name: mustard-scan
description: Use when the user runs /scan or asks to analyze, discover, or rescan the codebase. Mines the repo into grain.model.json (deterministic, language-agnostic) and then enriches it as standard — subproject Guards prose and the local recall-index vectors — the durable model the feature/spec pipeline consumes.
source: manual
---
<!-- mustard:generated -->
# /scan — Codebase model

`/scan`, `/scan --root <dir>`, `/scan --out <path>`. **Enrichment is STANDARD** — there is no `--full` or `--enrich` flag. One `/scan` always does the deterministic model, the subproject maps, and the two enrichments (Guards prose, recall-index vectors).

## Process

### 1. Deterministic model + maps (no AI, you do NOT read source)

```bash
mustard-rt run scan --full [--root <dir>] [--out <path>]
```

Always writes `.claude/grain.model.json` — the rich, language-agnostic model (modules, declarations, dependency graph, mined roles, recurring vertical slices, shared contracts, touchpoints, projects) — AND deterministically (re)generates a lean `CLAUDE.md` map per subproject from that model: it **preserves** any hand-written `## Guards`, seeds a `pending` `## Guards` placeholder where none exists, and **never touches the workspace root**. The model is the durable product: downstream (`/feature`, `/bugfix`) consumes it through `mustard-rt run feature` (the digest research step) and `mustard-rt run scan spec` — never by reading the model or the repo directly.

Parse the JSON result (`{ ok, model, regenerated?, oversized? }`); report the model path and surface any `oversized` warnings or `regenerated` paths.

### 2. Enrichment (STANDARD — the AI part of `/scan`)

After the model is built, ALWAYS run the two enrichments below. Each is **incremental** (only the delta since the last scan) and **fail-open** (headless / no LLM / empty worklist → skip that one silently; the deterministic model is already complete and correct). The FIRST scan of a repo pays the one-time enrichment cost; every later scan only does what changed.

#### A) Guards (subproject `## Guards` prose)

1. **Worklist.** `mustard-rt run scan-guards-list` emits a JSON array `[{path, subproject, kind, frameworks}]` of every subproject `CLAUDE.md` still `pending` (root already excluded). Fail-open: on error it returns `[]` (exit 0) — nothing to do. Empty → skip A.
2. **Render one prompt per subproject.** For each worklist item: `mustard-rt run agent-prompt-render --role guards --subproject <subproject> --emit ref` — spec-less (no `--spec`): the renderer reads the pending block's facts and derives the project's language/tone from `mustard.json`. Pass its stdout to the Task **verbatim** (with `--emit ref` it is a 2-line stub the PreToolUse hook expands at dispatch — never read the `.dispatch/` file in the parent); never hand-craft the prompt.
3. **Dispatch in parallel + relay.** Dispatch **one agent per subproject**, `subagent_type` `mustard-guards` (read-only — returns the lines as its final message), all in a **single message** so they run in parallel. Relay each agent's authored guards to `mustard-rt run scan-guards-apply --path <path> --guards -` (text on stdin). The apply is non-destructive (only the block's span changes), capped at ~6 lines, idempotent (flips the marker off `pending`).
4. **Root never enters** — `scan-guards-list` already excludes it.

#### B) Recall index (method-meaning vectors — the recall enrichment)

The digest finds code by NAME; a method whose name diverges from the request's domain word (e.g. the user asks to *settle* a payable but the method is `WriteOffAsync`) is invisible to it. The recall index closes that — a LOCAL embedding vector per logic method, built from the body, that `mustard-embed search` matches BY MEANING when the digest judge reports `centralFound=false`. It **replaces the old per-method Sonnet `purpose` summaries**: local, free, constant-cost — no LLM, no per-method token cost, independent of repo size. Validated on medusa (TS) + saleor (Python): name+embedding `combined@5 = 1.0`, matching the Sonnet enrich at ~zero cost (see `docs/OTIMIZACAO-PURPOSE-ENRICH.md`). See [[recall-index]].

1. **Build (ONE command — deterministic, local, no agents, no prompt).** `mustard-embed build --model .claude/grain.model.json --embed-model code` embeds every logic method's body with a local code-specialised model (jina-code) and writes `.claude/grain.vectors` (a compact binary sidecar). No worklist, no batching, no Sonnet, no Workflow, no spend prompt — the cost is local compute. **INCREMENTAL**: a re-scan reuses the stored vector of every unchanged method (matched by body hash) and re-embeds only new/changed ones, so the 2nd scan onward is near-instant; surface the one-line JSON result (`{ok, methods, embedded, reused, dim, out}`). **Progress:** the FIRST build of a large repo is minutes of local CPU — `build` writes `.claude/grain.vectors.progress` (`{done,total,pct}`) after each chunk and removes it on completion. Run it in the background — don't block, and don't poll the file or print progress to the chat. The status line surfaces the build `pct` live (segment `⟳ scan NN%`) while `.claude/grain.vectors.progress` exists and clears it on completion.
2. **Fail-open.** If `mustard-embed` is absent (not installed / headless), skip silently — the digest degrades to name-only (no error). The miss-time lookup (`mustard-embed search --intent "<missed terms>" --vectors .claude/grain.vectors`) is the recall recovery the orchestrator runs on a `centralFound=false` miss — same output shape as the old `purpose-search`.

## INVIOLABLE RULES

- The **deterministic pass** (model + maps) NEVER calls AI and NEVER reads source; it always writes `grain.model.json` and the lean per-subproject `CLAUDE.md` maps (preserving hand-written `## Guards`, root excluded). It is unconditional.
- **Enrichment is a STANDARD and COMPLETE part of every `/scan`** — no opt-in flag, no confirmation prompt: (A) Guards prose — the ONLY LLM step, one cheap agent per pending subproject; (B) the Recall index — `mustard-embed build`, a LOCAL embedding (NO LLM, NO per-method cost). Both are **fail-open**: if a step cannot run (headless, no LLM, `mustard-embed` absent, empty Guards worklist) it skips silently, never blocking and never corrupting the model.
- An LLM in enrichment now WRITES exactly ONE thing: subproject `## Guards` prose (never the workspace root, capped ~6 lines, non-destructive). The recall index is built by a LOCAL model (`mustard-embed`), not an LLM — additive, into a `.claude/grain.vectors` sidecar. Never source, never system prompts.
- **`/scan` IS the approval — there is NO spend prompt, EVER**, and the recall index never even touches a paid model. Do NOT ask whether to (re)build the index, do NOT present "rebuild / partial / skip" options, do NOT cite a dollar cost. If a step can run it runs silently; if it cannot it skips silently.
