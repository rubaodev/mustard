<!-- mustard:generated -->
# tauri — examples in this codebase

<!-- mustard:enrich hash=a8c2a8880099 -->
## Purpose

Worked examples of the Tauri-binding convention. `api/env.ts` shows the minimal shape — two thin `invoke()` wrappers (`readEnv`/`writeEnv`) with no extra types. `lib/dashboard.ts` is the broad case: dozens of `interface` declarations paired with one `invoke<T>(...)` wrapper each, mapping every `dashboard_*` Rust command. `lib/projects.ts` adds the snake_case-to-camelCase remapping idiom (`RawArtifactDrift` -> `ArtifactDrift`) for commands whose serde output does not match the UI shape.
<!-- /mustard:enrich -->

- Ref: apps/dashboard/src/api/env.ts
- Ref: apps/dashboard/src/lib/dashboard.ts
- Ref: apps/dashboard/src/lib/projects.ts

