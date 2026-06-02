<!-- mustard:generated -->
# ast — examples in this codebase

<!-- mustard:enrich hash=53d740d09931 -->
## Purpose

These files implement agnostic AST extraction over tree-sitter. `entity.rs` pulls named type declarations (struct/enum/trait/class/…) from a source blob via the `entity_definitions` query, with a keyword-driven textual fallback when no grammar or query resolves. `loader.rs` is the `GrammarLoader` — the sole owner of `tree_sitter::Language` handles, seeding compiled-in built-ins (Rust, TypeScript/TSX, Python, Go, Java, C#) and layering externally discovered grammars on top without ever overwriting a built-in.
<!-- /mustard:enrich -->

- Ref: packages/core/src/domain/ast/entity.rs
- Ref: packages/core/src/domain/ast/loader.rs
- Ref: packages/core/src/domain/ast/loader_test_helpers.rs

