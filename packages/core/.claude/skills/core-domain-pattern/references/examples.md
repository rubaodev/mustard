<!-- mustard:generated -->
# domain — examples in this codebase

<!-- mustard:enrich hash=8b672402fcce -->
## Purpose

These files are the typed owners of the project's persisted state. `command_detect.rs` detects the build/test/lint/type-check command set from the project's manifests (Cargo, JS package managers, Go, Make), falling back to a neutral placeholder for unknown stacks so nothing stack-specific is ever assumed. `config.rs` is `ProjectConfig` — the single source of truth for `mustard.json`, with camelCase serde, fail-open loading, and accessors that normalise locale, tone, commands, and subproject overrides. The repo model itself now lives in `.claude/grain.model.json`, produced by `mustard-rt run scan` and never parsed directly — it is read only through the scan tool (`scan facts` / `scan digest`) or `mustard_core::read_entity_names` / `read_projects`. (The former `entity_registry.rs` module and its `EntityRegistry` / `RegistryDoc` types, which owned the removed `entity-registry.json` document, have been deleted.)
<!-- /mustard:enrich -->

- Ref: packages/core/src/domain/command_detect.rs
- Ref: packages/core/src/domain/config.rs
- Ref: .claude/grain.model.json (read via `mustard-rt run scan` / `mustard_core::read_entity_names`; replaced the removed entity_registry.rs)

