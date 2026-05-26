---
name: mustard-knowledge
description: Use when the user runs /knowledge or asks about the project knowledge base, patterns, conventions, glossary, memory audit, or progress reports.
source: manual
---
<!-- mustard:generated -->
# /knowledge - Knowledge Management

## Trigger

`/knowledge <action> [args]`

| Action | Backend | Purpose |
|--------|---------|---------|
| `list` | `mustard-rt run memory list --grouped --format table` | Entries grouped by type |
| `search <term>` | `mustard-rt run memory search` | Case-insensitive match on name/description/tags |
| `glossary [--filter <t>]` | `mustard-rt run knowledge glossary [--filter <t>] --format table` | Entities + doc-comment descriptions |
| `add` | Interactive → `mustard-rt run memory knowledge` | Type/name/description/tags entry |
| `notes [target]` | Edit `{subproject}/.claude/commands/notes.md` | Persistent observations injected into agent context — NEVER overwritten by `/scan` |
| `audit` | Compare auto-memory vs CLAUDE.md/skills | Report-only — never auto-edits |
| `report <period>` / `evolve` / `export` / `import <file>` | → `../../../refs/knowledge/evolve-report.md` | Reporting + sharing |

Per action: run the backend command and print stdout verbatim.

## Glossary descriptions

The `mustard-rt run sync-registry` description-enricher reads the first ref file of each entity, extracts the preceding doc-comment (JSDoc `/** */`, `///`, `//`, `#`), strips markers + `@tag` lines, collapses whitespace, truncates to 200 chars, and sets `entry.description` only when not already set (manual descriptions preserved). Improve coverage by adding doc-comments above entity declarations.

## INVIOLABLE RULES

- `knowledge_patterns` SQLite table is persistent — never deleted by session-cleanup.
- `add` and pipeline capture both call the same `mustard-rt run memory knowledge`.
- NEVER add `<!-- mustard:generated -->` to `notes.md` (user files).
- Always show entry count in list/search output.
