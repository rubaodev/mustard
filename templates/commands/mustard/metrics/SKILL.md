---
name: mustard:metrics
description: Manage and inspect enforcement metrics — budget-gate mode control, hook hit rates, gate activity reports.
---

# /mustard:metrics - Enforcement Metrics Management

## Trigger

`/mustard:metrics <subcommand> [args]`

## Subcommands

| Subcommand | Purpose |
|---|---|
| `observe` | Set budget-gate to observe mode (log sizes, never block) |
| `warn` | Set budget-gate to warn mode (log + allow over-budget) |
| `strict` | Reset budget-gate to strict mode (default — hard-block) |
| `status` | Show current mode + summary of .claude/.metrics/ state |
| `report [--since <ISO>] [--event <type>]` | Run metrics-report.js with optional filters |
| `reset` | Clear all metrics .jsonl files (asks for confirmation) |

## Mode persistence

Mode is stored in `.claude/.metrics/.mode` (single-line file).
Precedence for hook mode detection:
1. `CONTEXT_BUDGET_MODE` environment variable (highest)
2. `.claude/.metrics/.mode` file (fallback)
3. Default: `strict`

## Actions

### `observe` / `warn`
1. Ensure `.claude/.metrics/` directory exists (mkdir recursive)
2. Write `observe` or `warn` to `.claude/.metrics/.mode`
3. Report: "Mode set to `<mode>`. Effective on next hook fire. (Env var CONTEXT_BUDGET_MODE takes precedence if set.)"

### `strict`
1. If `.claude/.metrics/.mode` exists → delete it (returns to default behavior)
2. Report: "Mode reset to `strict` (default). `.mode` file removed."

### `status`
1. Detect current mode:
   - If `CONTEXT_BUDGET_MODE` env var is set → report "Mode: <value> (env var)"
   - Else if `.mode` file exists → report "Mode: <file contents> (file)"
   - Else → report "Mode: strict (default)"
2. List `.claude/.metrics/*` files with sizes (use `fs.readdirSync` + `fs.statSync`)
3. Run `rtk node .claude/scripts/metrics-report.js` and display the output

### `report [args]`
1. Pass arguments through to `rtk node .claude/scripts/metrics-report.js <args>`
2. Display output verbatim

### `reset`
1. Use `AskUserQuestion`: "Delete all metrics .jsonl files in .claude/.metrics/? (yes/no)"
2. If yes: list all `.jsonl` files, delete each (preserve `.mode` file), report count
3. If no: abort, report "Reset cancelled"

## Rules
- NEVER delete `.mode` file in reset (only .jsonl files)
- ALWAYS preserve env var precedence over file
- Fail-open if `.claude/.metrics/` doesn't exist in `status` (report as empty)
- Built-ins only (fs, path, process)
