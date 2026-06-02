---
name: mustard-scan
description: Use when the user runs /scan or asks to analyze, discover, or rescan the codebase. Mines the repo into grain.model.json (deterministic, language-agnostic, no AI) — the durable model the feature/spec pipeline consumes.
source: manual
---
<!-- mustard:generated -->
# /scan — Codebase model

`/scan`, `/scan --root <dir>`, `/scan --out <path>`, `/scan --full`.

## Process

One deterministic step — no AI, and you do NOT read source:

```bash
mustard-rt run scan [--root <dir>] [--out <path>] [--full]
```

Always writes `.claude/grain.model.json` — the rich, language-agnostic model (modules,
declarations, dependency graph, mined roles, recurring vertical slices, shared
contracts, touchpoints, projects). It is the durable product: run once per repo,
re-run when the code changes materially. Downstream (`/feature`, `/bugfix`) consumes it
through `mustard-rt run feature` (the digest research step) and `mustard-rt run scan spec`
— never by reading the model or the repo directly.

### Per-subproject CLAUDE.md

- **Default (no `--full`)**: writes NOTHING into subprojects. It only *measures* each
  subproject's `CLAUDE.md`; if one is large enough to weigh on token usage when
  auto-injected, the JSON `oversized[]` lists it and a warning suggests `--full`.
- **`--full`**: deterministically (re)generates a lean `CLAUDE.md` per subproject from the
  grain model (a small orientation map), creating `{subproject}/.claude/` if absent. It
  **preserves** any hand-written `## Guards` section — only the generated map block is
  replaced; human guards are never clobbered. Still no AI, still no source reading.

Parse the JSON result (`{ ok, model, regenerated?, oversized? }`); report the model path
and surface any `oversized` warnings or `regenerated` paths.

## INVIOLABLE RULES

- Default `/scan` produces only `grain.model.json` and never writes into subprojects; it may *warn* about oversized subproject `CLAUDE.md` files.
- `--full` only (re)writes a deterministic, lean `CLAUDE.md` map per subproject and preserves hand-written `## Guards`. It never generates skills/agents and never invokes AI.
- No confirmation prompts — `/scan` is the approval.
