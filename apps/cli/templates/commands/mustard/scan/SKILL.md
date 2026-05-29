---
name: mustard-scan
description: Use when the user runs /scan or asks to analyze, discover, or rescan the codebase. Agnostic and DETERMINISTIC by default (zero AI) — discovers subprojects, populates the entity-registry, renders per-cluster skills + stack.md in Rust, validates, runs a security scan. `/scan --enrich` adds one parallel AI prose-enrichment agent per subproject.
source: manual
---
<!-- mustard:generated -->
# /scan - Agnostic Code Analyzer

`/scan`, `/scan <subproject>`, `/scan --force` (bypass incremental skip), `/scan --enrich` (add AI prose on top of the deterministic pass).

## Process

### 1. Pre-dispatch

```bash
mustard-rt run scan-orchestrate [<subproject>] [--force] [--enrich]
```

Parse the JSON. The binary does ALL the mechanical work deterministically (no AI): discovery, hash comparison, stale cleanup, bootstrap files, **registry population**, **per-cluster `SKILL.md` + `references/examples.md` + `stack.md` generation** (and a `_no-patterns.md` marker where no cluster qualifies), rich `.claude/agents/*` files, and the dispatch plan.

**Default mode is ZERO AI:** `dispatch[]` comes back **empty** — there is nothing to dispatch. Skip straight to step 3, then print the summary. The full scan is done by Rust.

`dispatch[]` is non-empty **only** when `--enrich` was passed.

### 2. Dispatch — only when `dispatch[]` is non-empty

If `dispatch[]` is empty, skip to step 3 — the deterministic scan is complete with zero AI.

Otherwise emit **N `Task` tool-calls in ONE single assistant message** (one per `dispatch[]` item). Do **not** send one Task and wait for its result before sending the next — all N go out together. Example for 3 items: your single message contains `Task(...)`, `Task(...)`, `Task(...)` — three tool-use blocks, no text between them and the turn end. Never `run_in_background: true`. Pass each item's `agentPrompt` **verbatim** — it already carries the EVIDENCE RULE and the "enrich only, never re-discover" contract.

**`subagent_type` selection (token economy):** when `.claude/agents/{name}-impl.md` exists (listed in `generated[]`), dispatch with `subagent_type: "{name}-impl"` so Claude Code applies that agent's system prompt natively (guards/skills/clusters not re-sent). Otherwise fall back to `subagent_type: "general-purpose"`. The `agentPrompt` is passed verbatim either way.

### 3. Post-dispatch + verification

```bash
mustard-rt run scan-finalize
```

Refreshes the detect cache, validates skills, runs the security scan, verifies each subproject has a `SKILL.md` or `_no-patterns.md` (now Rust-owned, so this passes in default mode). Surface `errors[]`/`warnings[]`.

In default mode `dispatchVerify.ran` is `false` (no agents) **or** `ok:true` (Rust wrote the artifacts) — both are expected, not errors. The `dispatchVerify.ok === false` re-dispatch loop applies **only** in `--enrich` mode: one follow-up Task per `status === "empty"|"missing-dir"` subproject (single message, parallel), then re-run `scan-finalize`.

## Return Format

```json
{ "scanned": [...], "skipped": [...], "generated": [], "cleanup": [],
  "skills_generated": { "sub": [...] }, "security": { "findings": 0 }, "errors": [] }
```

**Sourcing — do not invent counts:** `scanned`/`skipped`/`generated`/`cleanup` from `orchestrate.json.*`. `skills_generated` counts from `orchestrate.json.generated` (the Rust render report — `skills: N SKILL.md`), NOT from `dispatchVerify`. `security.findings` from `finalize.steps.security.findings`. `errors` = concat of both error arrays.

After a default-mode run, tell the user: **"Deterministic scan complete (0 AI tokens). Run `/scan --enrich` to add AI-written guard rationale and pushier skill descriptions."**

## Fallback

`scan-orchestrate` fails: `mustard-rt run sync-detect` → `mustard-rt run sync-registry --force` → `mustard-rt run scan-skill-render` + `mustard-rt run scan-structural` per subproject (all deterministic, no AI) → report which step failed.

## INVIOLABLE RULES

- Default `/scan` is zero-AI — **never dispatch a Task unless `dispatch[]` is non-empty.**
- No confirmation prompts — `/scan` is the approval.
- Never `run_in_background: true` for Task agents.
