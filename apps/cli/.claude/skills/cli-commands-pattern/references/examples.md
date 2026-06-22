<!-- mustard:generated -->
# commands — examples in this codebase

<!-- mustard:enrich hash=1ae8a729bcae -->
## Purpose
Concrete `src/commands/` modules to pattern-match against. `add.rs` is the fullest example — an `AddOptions { force }` struct and an `add(cwd, spec, options)` entry that fetches a template (GitHub clone then npm tarball) or installs a skill, validates the name, then copies files and merges `hooks_additions`. `config.rs` shows the deliberately thin wrapper: a `ConfigOptions { yes }` struct whose `config` entry just delegates to `git_flow::configure`. `git_flow.rs` holds the reusable building blocks (`probe_git`, `collect_choices`, `apply_choices`, `configure`) that both `config` and `init` share so the flow rules live in one place.
<!-- /mustard:enrich -->

- Ref: apps/cli/src/commands/add.rs
- Ref: apps/cli/src/commands/config.rs
- Ref: apps/cli/src/commands/git_flow.rs

