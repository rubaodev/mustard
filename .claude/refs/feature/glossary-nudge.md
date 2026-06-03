# Glossary Nudge (ANALYZE, optional, non-blocking)

> Detail for the `/feature` ANALYZE "glossary nudge" (Selo 1). The pipeline can
> *point at* grilling without grilling inline: a deterministic, zero-token
> coverage check decides whether to offer ONE dismissible suggestion to enrich
> the domain glossary via the `grill-with-docs` skill **before** planning. It
> never blocks, never interrogates the user inline, and writes nothing itself.

## When

Right after the `mustard-rt run feature` digest (which produces the matched
repo-vocabulary terms), once per request. Skip entirely on Light requests where
you already have full precedent — the nudge is most valuable on net-new / wide
Full features that touch domain terms the glossary doesn't define.

## Run

```bash
mustard-rt run glossary-coverage --intent "<the request>" \
  --context {root}/CONTEXT.md
  # repeat --context for each subproject CONTEXT.md / a CONTEXT-MAP.md
```

It is **deterministic and zero-token**: pure Rust over `grain.model.json` +
`CONTEXT.md`, reusing the exact term matcher `context-slice` uses. Output is
byte-stable JSON:

```json
{ "verdict": "missing|weak|ok|na", "present": false, "termsTotal": 3,
  "termsCovered": 0, "coveragePct": 0, "uncovered": ["spec","wave","pipeline"] }
```

- **N (`termsTotal`)** = the digest's MATCHED terms (repo vocabulary the intent
  maps to), not raw intent tokens — stopwords never inflate it.
- **`verdict`**: `missing` (no `CONTEXT.md` authored) · `weak` (authored but
  coverage `< 50%` OR `≥ 3` uncovered matched terms) · `ok` (covered, or no
  domain terms touched) · `na` (scan model unavailable — fail-open).

## React

- `verdict ∈ {missing, weak}` → surface **ONE** AskUserQuestion, then continue
  on any answer (default/Enter = continue). Name the uncovered terms so the offer
  is concrete:
  > "This feature touches domain terms your glossary doesn't define yet
  > (`{uncovered}`). Want to sharpen `CONTEXT.md` with `grill-with-docs` first so
  > the spec and every dispatched agent share that language?"
  > — Options: **Grill first** (hand off to the `grill-with-docs` skill) /
  > **Skip, plan now** (continue) / **Don't ask for this repo** (note + continue).
- `verdict ∈ {ok, na}`, or the command is missing/errors → **stay silent and
  continue**. The lean path is byte-identical to a run without this step.

## Hard rules

- **Never block** and **never grill inline.** The actual grilling lives in the
  dedicated `grill-with-docs` skill; `/feature` only points at it.
- **Writes nothing.** This step has zero side effects — no `CONTEXT.md`, no
  events. All glossary/ADR writes belong to `grill-with-docs`, which owns the
  **English-only** `CONTEXT.md`/ADR contract (`spec-language.md`). Only the live
  AskUserQuestion text localises to the user's language.
- **Fail-open to OFF.** If `glossary-coverage` is absent (binary not rebuilt) or
  errors, treat it as `na` and continue — the nudge is an enhancement, never a
  gate.

## Why it pays

The glossary the user authors here is not wasted: it flows downstream for FREE
through the already-wired `context-slice → {context_md}` cache, so a sharpened
`CONTEXT.md` reaches every wave-1 subagent with zero new per-dispatch wiring.
Deliberate friction lands only where it amortises (wide/Full features with
uncovered domain terms); everywhere else the step is silent and free.
