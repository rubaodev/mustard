<!-- mustard:generated -->
# Orchestrator Rules

## Role
You do NOT implement code — you delegate via Task tool.

## Intent Routing

| Intent | Signals | Action |
|--------|---------|--------|
| Feature | create, add, new entity, new CRUD, implement | Pipeline Feature (Full scope) |
| Enhancement | improve, adjust, change, add field/column, change behavior, optimize, update | Pipeline Feature (auto-detects Light/Full scope) |
| Bugfix | error, bug, not working, broken, fix, correct | Pipeline Bugfix |
| Analyze | analyze, audit, evaluate, check, compare, inspect, assess | Delegate via /task |
| Simple | config, docs, small refactor, rename, move | Delegate via Task |

Any change that touches production code (schema, API, UI) → Pipeline Feature.
Scope is auto-detected: Light (1-2 layers, ≤5 files, known pattern) vs Full (3+ layers, new entity).

## Pipeline Phases
ANALYZE → PLAN → EXECUTE → QA → CLOSE (Wave 10)
- Light scope: skip PLAN (ANALYZE → EXECUTE → QA → CLOSE)
  - ANALYZE: Grep/Glob direct preferred; ≤1 Task(Explore) with ≤10 tool uses allowed
  - Reclassify to Full if >5 files surface
  - All dispatched agents cap returns at ≤50 lines
- Full scope: ANALYZE → PLAN → /approve → EXECUTE → QA → CLOSE

### QA Phase (Wave 10)
After EXECUTE completes, run QA before CLOSE:
1. Spec PLAN must define `## Acceptance Criteria` (3-8 AC, each with a runnable command)
2. QA agent reads spec, executes each AC, reports pass/fail
3. close-gate blocks CLOSE unless `qa.result` with `overall=pass` exists in events log
4. Control: `MUSTARD_QA_GATE_MODE=strict (default) | warn | off`

## Context Loading
Agents auto-load skills from `{subproject}/.claude/skills/` based on task description.
Guards always loaded via `{subproject}/CLAUDE.md`.

## Stack

Node.js (>=18), CommonJS, no external dependencies. 23 lifecycle hooks, 12 scripts, 17 slash commands, 6 foundation skills.

## Commands

```bash
# Run hook tests
node --test hooks/__tests__/hooks.test.js

# Subproject discovery (outputs JSON)
node scripts/sync-detect.js
node scripts/sync-detect.js --no-cache

# Entity registry generation
node scripts/sync-registry.js
node scripts/sync-registry.js --force

# Skill validation (invoked by /scan §4.7; also callable standalone)
node scripts/skill-validate.js
node scripts/skill-validate.js --json
```

## Guards

- All hooks fail-open (exit 0 on error) — never block due to hook bugs
- All hooks use only Node.js built-ins — no npm dependencies
- PreToolUse hooks use `permissionDecision` response format
- PostToolUse hooks use `decision` response format
- Every new hook must be registered in `settings.json` with a timeout
- Task dispatch failures (API overload, HTTP 5xx, tool result missing) are logged to `pipeline-state.lastDispatchFailure`; `/resume` auto-recovers within 10 min
- Generated files must start with `<!-- mustard:generated -->` header
- Skills must have YAML frontmatter BEFORE the `<!-- mustard:generated -->` line

## Scan References

| File | Description |
|------|-------------|
| `.claude/commands/stack.md` | Technology stack, structure, tooling |
| `.claude/commands/patterns.md` | 12 recurring code patterns with refs |
| `.claude/commands/guards.md` | DO/DON'T rules for hooks, scripts, commands, skills |
| `.claude/commands/recipes.md` | Implementation recipes for new hooks, commands, skills, scripts |
| `.claude/commands/notes.md` | Manual notes (never overwritten) |

## Recommended Skills

- `templates-hook-protocol` — Hook stdin/stdout JSON protocol
- `templates-settings-wiring` — settings.json hook registration
- `templates-sync-detect` — Subproject discovery and role detection
- `templates-command-authoring` — Slash command SKILL.md structure
- `templates-skill-authoring` — Foundation/subproject skill creation

## Token Economy

RTK (Rust Token Killer) is integrated as core infrastructure. A PreToolUse hook automatically rewrites Bash commands through `rtk`, reducing token consumption by 60-90% on CLI outputs.

- **Hook**: `hooks/rtk-rewrite.js` — transparent, fail-open
- **Analytics**: `rtk gain` — view token savings; `metrics-report.js` integrates RTK data
- **Statusline**: Shows real-time savings when RTK is active
- If RTK is not installed, the hook silently passes through (zero impact)

### Cost Optimization Hooks

Three enforcement hooks reduce token waste across all projects:

| Hook | Matcher | Mode | Effect |
|------|---------|------|--------|
| `bash-native-redirect.js` | Bash | strict/warn/off | Blocks grep/ls/cat/head/tail/find → suggests Grep/Glob/Read tools. Warns on piped commands too. |
| `model-routing-gate.js` | Task | strict/warn/off | Blocks model upgrades vs routing table. Advises when no model specified. |
| `tool-use-counter.js` | .* + SubagentStart/Stop | hard | Caps Explore agents at 15 tool uses (warn at 12) |
| `recommended-skills-audit.js` | Task | advisory | Conta skills listados no prompt; warn se >10; não bloqueia |

**Environment overrides:**
- `MUSTARD_BASH_REDIRECT_MODE=warn|strict|off` (default: strict)
- `MUSTARD_MODEL_GATE_MODE=warn|strict|off` (default: strict)
- All hooks can be disabled via `MUSTARD_DISABLED_HOOKS=bash-native-redirect,model-routing-gate,tool-use-counter`

### Enforcement Hooks

**Strict gates** (Wave 9-10): block on real failure

| Hook | Matcher | Mode env | Blocks on |
|------|---------|----------|-----------|
| `close-gate.js` | Write/Edit `.pipeline-states/*.json` with phase=CLOSE | `MUSTARD_CLOSE_GATE_MODE` (default strict) | build/type/lint/test fail |
| `close-gate.js` (Wave 10 QA) | same trigger | `MUSTARD_QA_GATE_MODE` (default strict) | no qa.result or qa.result=fail |
| `review-gate.js` | Bash `git commit` | `MUSTARD_COMMIT_GATE_MODE` (default warn) | secrets staged or build broken |

Bug in the hook itself (I/O error, timeout outside child process) still fails open — only real sensor failures block.

**Anti-slope hooks** (Wave 11): default warn, opt-in strict

| Hook | Mode env | Heuristic |
|---|---|---|
| `duplication-check.js` | `MUSTARD_DUPLICATION_MODE` (default warn) | Levenshtein ≥0.85 vs entity-registry |
| `convention-check.js` | `MUSTARD_CONVENTION_MODE` (default warn) | Rules derived from knowledge.json conf≥0.8 |
| `regression-guard.js` | `MUSTARD_REGRESSION_MODE` (default **off**) | File-to-test heuristic + re-run |

All anti-slope hooks fail-open on bug. Only real signal triggers warn/block.

**Downgrade in emergencies:**
- `MUSTARD_CLOSE_GATE_MODE=warn` — prints warning, allows
- `MUSTARD_CLOSE_GATE_MODE=off` — skips check
- `MUSTARD_QA_GATE_MODE=warn` — prints warning when QA absent/fail, allows
- `MUSTARD_QA_GATE_MODE=off` — skips QA check entirely
- `MUSTARD_COMMIT_GATE_MODE=strict` — upgrades commit gate to blocking
- `MUSTARD_COMMIT_GATE_MODE=off` — skips commit gate entirely

## Shared Memory Architecture (Wave 4)

### Truth source
`.claude/.harness/events.jsonl` — append-only NDJSON log. All hooks emit events here.

### Persistent projections
| File | Writer | Purpose |
|------|--------|---------|
| `knowledge.json` | `session-knowledge.js` + `knowledge-update.js` | Confidence-ranked patterns across sessions |
| `memory/decisions.json` | `memory-persist.js` | Architectural decisions |
| `memory/lessons.json` | `memory-persist.js` | Operational lessons |
| `.pipeline-states/{spec}.json` | Pipeline commands | Current phase (ANALYZE/PLAN/EXECUTE/CLOSE) |

### How agents read context
Via **views** in `scripts/harness-views.js`:
- `buildAgentVisibility(events, opts)` — parallel agents in current wave + prior findings
- `buildPipelineState(events, { spec })` — phase + metrics (tool counts, retries, agents) for a spec
- `buildCrossSessionTimeline(sessionsDir, opts)` — episodic memory across sessions
- `buildSessionSummary(events)` — roll-up for SessionEnd fold

### Removed (Wave 4 — no longer written)
- `.agent-memory/` (was: per-agent summaries → now: `agent.stop` events in log)
- `.agent-state/_queue.json` (was: description relay → now: `agent.start` event in log)
- `.agent-state/{id}.json` (was: active agent tracking → now: `agent.start`/`agent.stop` in log)
- `.pipeline-states/*.metrics.json` (was: cumulative counters → now: `tool.use` events folded by `buildPipelineState`)

### Log rotation
`harness-init.js` (SessionStart) rotates `.harness/events.jsonl` → `.harness/sessions/{sessionId}.jsonl` and prunes sessions >30 days.

### On-Demand Memory Queries (Escape Hatch)

The automatic injection in SessionStart/SubagentStart is capped (400-800 chars). If you need more historical context, query the harness directly:

```bash
# Find specific topic in session summary
node .claude/scripts/harness-views.js --view session-summary --query "JWT" --compact

# Get full state of a spec
node .claude/scripts/harness-views.js --view pipeline-state --spec auth-login --compact

# See last N sessions timeline
node .claude/scripts/harness-views.js --view cross-session-timeline --limit 5 --compact

# Active parallel agents in current wave
node .claude/scripts/harness-views.js --view agent-visibility --compact
```

**When to use:**
- Exploring a feature area you have partial context on
- Before making a decision, check if a similar one was made
- Resuming work on a spec after session gap

**When NOT to use:**
- For patterns already in `knowledge.json` (that's auto-injected)
- As first action of every task (injection already gives you the top)

## Full Reference
Rules, pipeline, naming: `.claude/pipeline-config.md`
