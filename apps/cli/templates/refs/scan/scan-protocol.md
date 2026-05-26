# Scan Protocol Reference

> Execution model, agent dispatch sequence, --force handling, read-before-write rules for `/scan`.

## Execution Model

- Orchestrator does NOT perform analysis directly. ALL analysis delegated to Task agents.
- Orchestrator's role: discover → incremental check → launch agents → collect results → compile.
- NO confirmation prompts (the user already invoked `/scan`).
- NO `run_in_background: true` for Task agents that write files.

### Read-Before-Write Protocol

Claude Code's `Write`/`Edit` fails with *"File has not been read yet"* when targeting an existing file without a prior `Read` in the same context. Whenever the orchestrator (or a Task agent) modifies an existing file: (1) `Read` the target first (even just the first lines); (2) only then `Write` (full overwrite) or `Edit` (patch); (3) if the path genuinely does not exist, `Write` is safe — verify via `Glob`, not by guessing.

Especially applies to: `.claude/CLAUDE.md` regeneration, root `CLAUDE.md` updates, `.claude/docs/*.md` frontmatter injection, subproject `CLAUDE.md` section edits.

## Step 1 — Discover + Incremental Detection

**Read OLD cache first**: `cat .claude/.detect-cache.json` and save `sourceHashes` + `moduleHashes`.

**Run detect**: `mustard-rt run sync-detect` (always a fresh scan). Parse JSON → `{ name, path, role, agent, stackSummary, gitDirty?, gitDirtyCount? }` per subproject. If `/scan <subproject>` was called, filter to that one.

**Compare hashes + git dirty**: hash match AND NOT `gitDirty` → skip agent (reuse existing `.claude/commands/`). Hash mismatch OR `gitDirty` → include in launch list. All skipped → skip to step 4 + 5. No old cache → first run, scan all. `--force` → ignore comparisons, mark all `needs-rescan`.

**Module-level incremental** (when subproject hash changed): compare `moduleHashes[subproject][module]` with cached values. Pass changed modules to the agent in `INCREMENTAL MODE` (reuse cached patterns for unchanged modules; full analysis only for changed; merge cached + new in final output).

## --force Flag Semantics

Ignores incremental skip + `gitDirty`. Bypasses Bootstrap fast-path (always regenerates `.claude/CLAUDE.md`). Passes "FORCE MODE" to Step 3 agents (delete `{subproject}/.claude/skills/*/` with `mustard:generated` header before regenerating). Runs `sync-registry --force` always. **Preserves** user-authored skills (without the `mustard:generated` header).

## Step 2.5 — Cleanup Stale Subprojects

Compare OLD cached `subprojects[].name` with NEW. For each name in OLD but absent in NEW: (1) delete `{name}/` if no non-generated user files (check `.claude/commands/notes.md` content — if user content, warn and skip); (2) remove stale entries from `.claude/.detect-cache.json` (`subprojects`, `sourceHashes`, `moduleHashes`); (3) remove `.claude/agents/{name}-impl.md` if unused; (4) remove stale skill directories referencing the removed subproject; (5) remove stale entity-registry entries; (6) log `CLEANUP: removed {name}`.

**Safety**: only delete directories NOT git-submodules (`git submodule status` empty) and NOT tracked (`git ls-files {name}` empty).

## Step 2.6 — Bootstrap (when needed)

Fast-path: root `CLAUDE.md` + `.claude/entity-registry.json` exist AND `--force` is not active → skip to step 3.

Otherwise create:

- `.claude/CLAUDE.md` — orchestrator entry point (always regenerate). `Read` it first if it exists, then `Write`. Body: Intent Routing table (Feature/Enhancement → Pipeline Feature; Bugfix → Pipeline Bugfix; Analyze → `/task`; Simple → Task) + a pointer to `pipeline-config.md`.
- Root `CLAUDE.md` — project map from detected subprojects (Project Structure table, Entity Registry pointer, Ignore Paths).
- `.claude/entity-registry.json` — `mustard-rt run sync-registry --force`. On failure, write empty skeleton `{ "_meta": { "version": "4.0" }, "_patterns": {}, "_enums": {}, "e": {} }`.
- `{subproject}/CLAUDE.md` — per subproject (skip if exists). Includes Stack, Commands, Key Paths, empty Guards (filled by analysis).

## Step 2.7 — Scan Product Docs

If `.claude/docs/` contains `.md` files: extract or infer name (H1) / description (first paragraph/blockquote) / topics (H2 keywords). **Read first**, then generate/update YAML frontmatter with `name`, `description`, `topics`, `scanned-at`. Existing frontmatter WITH `scanned-at` → overwrite; WITHOUT → preserve, skip; no frontmatter → prepend. No Task agent needed; orchestrator inline. Always `Read` before `Edit`/`Write`.

## Step 3 — Launch Agents

Launch ALL agents in a SINGLE message with parallel tool calls. NEVER `run_in_background: true` — agents MUST write files. For each subproject, launch one Task agent with `subagent_type: "general-purpose"`. See `evidence-rules.md` for the agent prompt template + EVIDENCE RULE.

**Execution rule**: NEVER ask the user to confirm file writes, overwrites, deletes, or directory creations. The user already invoked `/scan`.

## Step 4 — Update CLAUDE.md files

Regenerate `.claude/CLAUDE.md` from step 2.6 template (always overwrite). Update root `CLAUDE.md`: `Read` before any `Edit`; update `## Project Structure` if subprojects changed, project-specific commands detected, `## Ignore Paths`.

## Step 4.5 — Generate Agents

For each subproject, generate `.claude/agents/{subproject.name}-impl.md` (frontmatter `name`, `description`, `model: sonnet`, `tools: [Read, Write, Edit, Bash, Grep, Glob]`, `memory: project`; body with Mandatory Reads, Boundary from Role Rules, Validation command, Return Format).

Also `.claude/agents/{subproject.name}-explorer.md` (`tools: [Read, Grep, Glob]`, body with Mandatory Reads, Skill References, Read-Only Boundary capped at ≤20 tool uses + ≤3 full file reads, Return Format with Findings table).

Mark all with `<!-- mustard:generated -->`. Overwrite on next scan.

## Step 4.6 — Generate Granular Skills

For each detected pattern, generate a granular skill following skill-creator methodology. → See `scan-format.md § 10` for decomposition rules, SKILL.md format, description guidelines.

Key rules: one conceptual pattern = one skill (not one file = one skill); skill name `{subproject-short}-{pattern-name}` (kebab-case concept used by the codebase); "pushy" description with casual trigger phrases; extract real code examples into `references/examples.md`; max 500 lines per SKILL.md body (ideally <200). Output: `{subproject}/.claude/skills/{skill-name}/SKILL.md` + `references/examples.md`. NEVER generated in root `.claude/skills/`. Mark all `<!-- mustard:generated -->`.

## Step 4.7 — Refresh Registry

`mustard-rt run sync-registry --force`. Skill generation is **entirely the responsibility of Step 3 agents** (see `scan-format.md § 10`).

## Step 5 — Refresh Detect

`mustard-rt run sync-detect` (recomputes discovery + source hashes every run; no separate cache file).

## Step 6 — Validate Skills

```bash
mustard-rt run skills validate --factual
```

Checks per skill: header, cluster backing (`fileCount ≥ 3`), sample existence, no fenced code in body, reference paths exist. Control: `MUSTARD_SKILL_VALIDATE_MODE=strict (default)|warn|off`. Strict mode + validator exit 1 → abort scan return (skills kept on disk; user alerted to fix).

## Security Scan Phase

Run after step 3 or independently via `/scan --security`:

```bash
mustard-rt run security-scan "$PROJECT_DIR"
mustard-rt run security-scan "$PROJECT_DIR" --json
```

Include findings under a `## Security` section. Severities: CRITICAL (secrets — flag, do not commit) / WARNING (env file not in .gitignore — add before push) / ADVISORY (dangerous permission rule — review). Exit code 0 = clean; 1 = findings. Secret previews truncated to 8 chars. Skip if `$PROJECT_DIR` unset; fall back to `process.cwd()`.
