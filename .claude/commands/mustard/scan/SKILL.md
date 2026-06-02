---
name: mustard-scan
description: Use when the user runs /scan or asks to analyze, discover, or rescan the codebase. Mines the repo into grain.model.json (deterministic, language-agnostic, no AI) — the durable model the feature/spec pipeline consumes.
source: manual
---
<!-- mustard:generated -->
# /scan — Codebase model

`/scan`, `/scan --root <dir>`, `/scan --out <path>`.

## Process

One deterministic step — no AI, and you do NOT read source:

```bash
mustard-rt run scan [--root <dir>] [--out <path>]
```

Writes `.claude/grain.model.json` — the rich, language-agnostic model (modules,
declarations, dependency graph, mined roles, recurring vertical slices, shared
contracts, touchpoints, projects). It is the durable product: run once per repo,
re-run when the code changes materially. NOTHING is written into subprojects'
`.claude/`; no per-project skills, agents, or concept-graph are generated.

Parse the JSON result (`{ ok, model }`) and report the model path. Downstream
(`/feature`, `/bugfix`) consumes it through `mustard-rt run feature` (the digest
research step) and `mustard-rt run scan spec` — never by reading the model or
the repo directly.

## INVIOLABLE RULES

- `/scan` only produces `grain.model.json`; it never writes into subprojects or generates per-project skills/agents.
- No confirmation prompts — `/scan` is the approval.
