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
| `create` | `/skill create <name>` | Invoke `skill-creator` skill (interactive) |
| `list` | `/skill list` | `mustard-rt run skills list --format table` |
| `remove` | `/skill remove <name>` | Delete `.claude/skills/{name}/` (warn if `source: scan`) |
| `optimize` | `/skill optimize <name>` | `skill-creator`'s description-optimization loop |
| `eval` | `/skill eval <name>` | `skill-creator` eval methodology |
| `update` | `/skill update skill-creator` | Sparse-clone `anthropics/skills`, refresh `skill-creator` |

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
- `optimize` and `eval` require Python 3 + `claude` CLI on PATH.
