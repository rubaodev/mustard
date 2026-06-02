---
name: core-ast-pattern
description: Use when adding or refactoring an `ast` module that parses source with tree-sitter and falls back to an agnostic textual extractor
tags: [add, refactor]
appliesTo: [ast]
scope: [code-editing]
source: scan
metadata:
  generated_by: scan
  cluster:
    label: ast
---

<!-- mustard:generated -->
# ast pattern

<!-- mustard:enrich hash=11770764b8f1 -->
## Purpose

The `ast` cluster wraps tree-sitter for `mustard-core`: a single `GrammarLoader` owns every `tree_sitter::Language` handle (built-in grammars linked into the binary, complemented by externally discovered ones), and extractors like `extract_entities` run a query against the parsed tree when a grammar resolves, otherwise dropping to a language-agnostic textual floor that scans for universal declaration keywords. Every path is fail-open — a missing grammar or query degrades to the textual heuristic rather than panicking or erroring.
<!-- /mustard:enrich -->

## Convention

- Folder: `src/domain/ast/`
- Extension: `.rs`
- Files: 9

## How to apply

These files are grouped by location under `src/domain/ast/`. Add another `.rs` file in the same folder.

## Examples

- Ref: packages/core/src/domain/ast/entity.rs
- Ref: packages/core/src/domain/ast/loader.rs
- Ref: packages/core/src/domain/ast/loader_test_helpers.rs

## Shape

Declares: 3 struct_item.

## References

See `references/examples.md`.
