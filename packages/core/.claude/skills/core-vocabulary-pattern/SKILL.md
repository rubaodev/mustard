---
name: core-vocabulary-pattern
description: Use when adding or refactoring a `vocabulary` module that does deterministic Aho-Corasick term matching for framework, architecture, or regression signals
tags: [add, refactor]
appliesTo: [vocabulary]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: vocabulary
---

<!-- mustard:generated -->
# vocabulary pattern

<!-- mustard:enrich hash=a25270c762c3 -->
## Purpose

The `vocabulary` cluster does deterministic, language- and stack-agnostic signal detection with no LLM and no regex alternation — every multi-pattern scan goes through one shared Aho-Corasick engine (`aho.rs`'s `KeyedAutomaton`, generic over the key each term is tagged with). On top of that one engine, `frameworks.rs` answers "what stack does this file belong to?" by matching ORM/framework/DI signals, and `architecture.rs` answers "how is the code organised?" by classifying path segments into layer roles (domain/application/infrastructure/ports/adapters) and inferring a style (layered/hexagonal/clean/DDD). Each vocabulary ships a built-in TOML base that an on-disk `.claude/vocab/*.toml` can override wholesale.
<!-- /mustard:enrich -->

## Convention

- Folder: `src/domain/vocabulary/`
- Extension: `.rs`
- Files: 5

## How to apply

These files are grouped by location under `src/domain/vocabulary/`. Add another `.rs` file in the same folder.

## Examples

- Ref: packages/core/src/domain/vocabulary/aho.rs
- Ref: packages/core/src/domain/vocabulary/architecture.rs
- Ref: packages/core/src/domain/vocabulary/frameworks.rs

## Shape

Declares: 12 struct_item, 3 enum_item.

## References

See `references/examples.md`.
