---
name: mustard-skill
description: Use when the user runs /skill or asks about installing, creating, listing, removing, optimizing, or evaluating skills. Handles install/create/list/remove/optimize/eval/update actions.
source: manual
---
<!-- mustard:generated -->
# /skill - Skill Manager

## Trigger

`/skill <action> [args]`

| Action | Usage | Backend |
|--------|-------|---------|
| `install` | `/skill install <source>` | `mustard-rt run skill-fetch <source>` |
| `create` | `/skill create <name>` | `skill-creator` (NOT bundled — see note) |
| `list` | `/skill list` | `mustard-rt run skills list --format table` |
| `remove` | `/skill remove <name>` | Delete `.claude/skills/{name}/` (warn if `source: scan`) |
| `optimize` | `/skill optimize <name>` | `skill-creator` description-optimization (NOT bundled — see note) |
| `eval` | `/skill eval <name>` | `skill-creator` eval methodology (NOT bundled — see note) |
| `update` | `/skill update skill-creator` | re-install `skill-creator` MANUALLY from `anthropics/skills` (see note) |

> **`skill-creator` is NOT bundled** (a ~250 KB Python authoring tool; removed — the project is shell-native, no Python). `create`/`optimize`/`eval`/`update` depend on it and are **inert until you install it manually**: clone the `skills/skill-creator` subdir of `github.com/anthropics/skills` into `.claude/skills/skill-creator/` (needs Python 3 + `claude` CLI). There is **no built-in fetch** — `mustard-rt run skill-fetch` is not implemented.

## install — source formats

| Format | Example |
|--------|---------|
| Local path | `/skill install ./my-skills/api-caching/` |
| GitHub (sparse) | `/skill install github:anthropics/skills/skills/pdf` |
| GitHub (full repo) | `/skill install github:owner/repo` |

`skill-fetch` handles sparse-clone, copy, frontmatter validation, and the install report. Print stdout verbatim.

## INVIOLABLE RULES

- NEVER delete skills with `source: manual` without user confirmation.
- `source:` field is **territorial**: `/scan` writes `source: scan` ONLY; `/skill install|create` writes `source: manual` ONLY. Missing `source:` → treat as `manual` (conservative).
- ALWAYS validate SKILL.md frontmatter on install (kebab-case `name`, description 50-600 chars with trigger word, `source: scan|manual`).
- `create`/`optimize`/`eval`/`update` need `skill-creator` installed MANUALLY (not bundled, no built-in fetch — clone the `anthropics/skills` subdir `skills/skill-creator`) plus Python 3 + `claude` CLI; they are inert otherwise.
