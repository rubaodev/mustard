<!-- mustard:generated -->
# vocabulary — examples in this codebase

<!-- mustard:enrich hash=0c982a8c756b -->
## Purpose

These files implement deterministic vocabulary matching over a shared engine. `aho.rs` isolates the `aho-corasick` dependency behind `KeyedAutomaton`, a generic leftmost-first DFA that tags each matched term with a key and deduplicates collisions in priority order — both the framework detector and the regression matcher reuse it instead of building their own automaton. `frameworks.rs` is `FrameworkVocabulary`: it scans source content for literal ORM / framework / DI signals (`pgTable(`, `@Entity`, `@Injectable`, `DbSet<`, …) keyed on `FrameworkCategory`, with a built-in TOML base and a wholesale on-disk override. `architecture.rs` classifies path segments into `LayerRole`s as bounded tokens and applies a pure decision rule over the role-presence set and dependency-edge directions to infer the architectural style plus a coarse SOLID-adherence note.
<!-- /mustard:enrich -->

- Ref: packages/core/src/domain/vocabulary/aho.rs
- Ref: packages/core/src/domain/vocabulary/architecture.rs
- Ref: packages/core/src/domain/vocabulary/frameworks.rs

