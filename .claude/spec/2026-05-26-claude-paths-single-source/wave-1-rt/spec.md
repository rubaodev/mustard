# W1 — `ClaudePaths` struct + `workspace_root()` walker (catálogo + âncora)

### Stage: Execute
### Outcome: Active
### Flags:
### Checkpoint: 2026-05-26T00:00:00Z

## Contexto

Entrega dois primitivos complementares no `mustard-core`, ambos pré-requisito da migração de call-sites (W2):

1. **Struct `ClaudePaths`** — catálogo auditável vivo no código.
2. **Função `workspace_root()`** — walker que resolve a raiz do workspace a partir de qualquer cwd, atravessando subprojetos do monorepo.

A separação importa: `ClaudePaths::for_project(root)` é uma API **neutra** (recebe a raiz como parâmetro); quem deve passar essa raiz é `workspace_root()?`, nunca `cwd` cru. Hoje o `mustard-rt` quando roda como hook a partir de `apps/rt/` escreve `mustard.db`, `.pipeline-states/`, `spec/{name}/.events/*.ndjson` etc. **dentro de `apps/rt/.claude/`** em vez da raiz do monorepo — regressão da migração TS→Rust ([[project_no_bun_rust_only]]) que perdeu a âncora estrutural (no JS o payload vivia em `.claude/scripts/` da raiz, então `path.resolve(__dirname, "..", "..")` sempre acertava; o binário Rust em `$PATH` não tem essa âncora). Sem `workspace_root()`, W2 substituiria 33 strings literais de path por 33 chamadas com a mesma raiz errada.

Invariante estrutural fechada por construção:
- **I1** — `.claude/.claude/` não existe em lugar nenhum do workspace. Walker, funções-folha do harness e doctor defendem essa invariante de ângulos diferentes. Já há evidência viva: `c:\Atiz\sialia\.claude\.claude\.metrics\bash-native-redirect.jsonl` (alguém passou `.claude/` em vez da raiz para o resolver; o resolver fez `.join(".claude")` por cima).

Ambos os primitivos vivem em `packages/core/` porque são consumidos por `apps/rt`, `apps/cli` (no comando `init`) e `apps/dashboard/src-tauri`. Não podem morar em nenhum dos três.

## Tarefas

- [ ] **T1.1** — Criar `packages/core/src/claude_paths.rs` com a árvore canônica:

  ```rust
  pub struct ClaudePaths { root: PathBuf }
  pub struct SpecPaths { root: PathBuf, spec_dir: PathBuf, spec_name: String }
  pub struct WavePaths { spec: SpecPaths, wave_dir: PathBuf, wave_slug: String }
  ```

  Métodos em `ClaudePaths`:
  - `for_project(root: impl AsRef<Path>) -> Self`
  - `for_spec(&self, name: &str) -> Result<SpecPaths>` — valida `name` não vazio, sem `/` nem `..`
  - Acessores: `cache_dir()`, `harness_dir()`, `metrics_dir()`, `agent_state_dir()`, `obsidian_dir()`, `commands_dir()`, `skills_dir()`, `refs_dir()`, `recipes_dir()`, `agents_dir()`, `agent_memory_dir()`, `spec_dir()`, `graph_dir()`
  - Acessores de arquivos raiz: `claude_md_path()`, `settings_json_path()`, `mustard_json_path()`, `entity_registry_json_path()`, `pipeline_config_md_path()`
  - Acessores de cache: `detect_cache_path()`, `scan_dispatch_path()`, `knowledge_seen_path()`, `memory_seen_path()` (todos apontam para dentro de `cache_dir()`)
  - `documented_dirs() -> Vec<&'static str>` — para `claude_dir_prune::DOCUMENTED_DIRS` derivar
  - `cache_files() -> Vec<&'static str>` — para `doctor` derivar
  - `audit_orphans(&self) -> Vec<PathBuf>` — lê filesystem, compara com catálogo, retorna divergências

  Métodos em `SpecPaths`:
  - `spec_md_path()`, `meta_json_path()`, `wave_plan_md_path()`, `adr_dir()`
  - `events_dir()`, `blobs_dir()`
  - `qa_report_json_path()`, `qa_report_html_path()`
  - `economy_baselines_path()`
  - `for_wave(&self, wave_slug: &str) -> Result<WavePaths>` — valida slug `^wave-\d+(-[a-z]+)?$`

  Métodos em `WavePaths`:
  - `spec_md_path()`, `meta_json_path()`
  - `diff_md_path()`, `prompt_md_path()`, `warnings_txt_path()`
  - `qa_report_json_path()` (per-wave QA, distinto do per-spec agregado)

- [ ] **T1.2** — Adicionar `pub mod claude_paths;` em `packages/core/src/lib.rs` e re-export `pub use claude_paths::{ClaudePaths, SpecPaths, WavePaths};`.

- [ ] **T1.3** — Testes em `packages/core/src/claude_paths.rs` (módulo `#[cfg(test)] mod tests`):
  - `for_project_sets_root`
  - `cache_dir_under_root_dot_cache`
  - `for_spec_rejects_empty_name`
  - `for_spec_rejects_path_traversal` (com `..` e `/` no nome)
  - `for_wave_rejects_malformed_slug` (sem prefixo `wave-`, sem número)
  - `documented_dirs_includes_all_top_level_dirs`
  - `cache_files_lists_four_caches`
  - `paths_are_idempotent` (chamar duas vezes retorna mesmo `PathBuf`)
  - `audit_orphans_returns_empty_on_clean_tree` (com `tempfile::tempdir`)

- [ ] **T1.4** — Doc-comments em **EN** ([[project_code_language_policy]]) explicando a árvore canônica. Inclui ASCII tree no doc do módulo mostrando `.claude/` → cache, harness, metrics, agent-state, spec/{name}/qa-report.json, etc.

- [ ] **T1.5** — Criar `packages/core/src/workspace.rs` com função única:

  ```rust
  pub fn workspace_root(start_dir: &Path) -> Result<PathBuf, WorkspaceError>;

  pub enum WorkspaceError {
      AnchorNotFound { searched_from: PathBuf },
      ForbiddenDotClaudeDotClaude { resolved: PathBuf },
      OverrideInvalid { path: PathBuf, reason: String },
  }
  ```

  Comportamento:
  - Override via env `MUSTARD_WORKSPACE_ROOT`: se setada, curto-circuita o walker e usa o valor (após validar I1 + existência do par `mustard.json + .claude/`).
  - Caso contrário, faz ancestor walk a partir de `start_dir`, retornando o primeiro diretório ancestral que contenha **ambos** `mustard.json` (arquivo) e `.claude/` (diretório).
  - Atravessa `.git/` de submodules livremente (não é critério de parada).
  - **Sem fallback para cwd**: se nenhum ancestral satisfaz o predicado, retorna `AnchorNotFound { searched_from }`.
  - **Guard I1**: se o último segmento do path resolvido for `.claude`, ou se o path contiver a sequência `.claude/.claude/` em qualquer posição, retorna `ForbiddenDotClaudeDotClaude { resolved }`.
  - Cache por processo (memoize por `(start_dir_canonical, override_value)`) para evitar IO repetido em hooks que chamam dezenas de vezes por sessão.

- [ ] **T1.6** — `ClaudePaths::for_project()` ganha guard I1 defensivo: se o path passado terminar em `.claude` ou contiver `.claude/.claude/`, retorna erro tipado (em debug, panic). Não substitui `workspace_root()` — é defesa em profundidade no caso de um call-site futuro passar path errado.

- [ ] **T1.7** — Adicionar `pub mod workspace;` em `packages/core/src/lib.rs` e re-export `pub use workspace::{workspace_root, WorkspaceError};`.

- [ ] **T1.8** — Testes em `packages/core/src/workspace.rs` (módulo `#[cfg(test)] mod tests`):
  - `workspace_root_resolves_from_root_when_anchor_present`
  - `workspace_root_resolves_from_subproject_ancestor_walk` (fixture monorepo de 3 níveis: cwd em `tempdir/apps/foo/src/`, raiz em `tempdir/`)
  - `workspace_root_fails_without_anchor` (tempdir sem `mustard.json + .claude/`)
  - `workspace_root_fails_with_only_mustard_json` (sem `.claude/`)
  - `workspace_root_fails_with_only_claude_dir` (sem `mustard.json`)
  - `workspace_root_traverses_git_submodule` (fixture: tempdir tem `.git/` em diretório intermediário; walker atravessa e acha a raiz)
  - `workspace_root_rejects_resolved_dot_claude_dot_claude` (start_dir aponta para path já contaminado)
  - `workspace_root_honors_env_override` (set `MUSTARD_WORKSPACE_ROOT` para tempdir válido)
  - `workspace_root_rejects_invalid_env_override` (env aponta para path sem âncora)
  - `workspace_root_memoizes_same_input` (segunda chamada não faz IO — observável via mock filesystem ou contador de syscalls)

## Critérios de Aceitação

- [ ] **AC-W1.1** — Compila. Command: `rtk cargo build -p mustard-core`
- [ ] **AC-W1.2** — Clippy limpo. Command: `rtk cargo clippy -p mustard-core -- -D warnings`
- [ ] **AC-W1.3** — Testes verdes. Command: `rtk cargo test -p mustard-core claude_paths workspace`
- [ ] **AC-W1.4** — Struct exportada em `mustard_core::ClaudePaths`. Command: `rtk node -e "const t=require('fs').readFileSync('packages/core/src/lib.rs','utf8');if(!/pub use claude_paths::\{[^}]*ClaudePaths/.test(t))process.exit(1)"`
- [ ] **AC-W1.5** — Doc-comment com ASCII tree presente. Command: `rtk node -e "const t=require('fs').readFileSync('packages/core/src/claude_paths.rs','utf8');if(!/\.claude/.test(t)||!/^\/\/!/m.test(t))process.exit(1)"`
- [ ] **AC-W1.6** — `workspace_root` exportada em `mustard_core::workspace_root`. Command: `rtk node -e "const t=require('fs').readFileSync('packages/core/src/lib.rs','utf8');if(!/pub use workspace::\{[^}]*workspace_root/.test(t))process.exit(1)"`
- [ ] **AC-W1.7** — `workspace_root` falha alto sem âncora. Command: `rtk cargo test -p mustard-core workspace_root_fails_without_anchor`
- [ ] **AC-W1.8** — `workspace_root` rejeita `.claude/.claude/` na resolução. Command: `rtk cargo test -p mustard-core workspace_root_rejects_resolved_dot_claude_dot_claude`

## Limites

`packages/core/src/claude_paths.rs` (novo), `packages/core/src/workspace.rs` (novo), `packages/core/src/lib.rs`, `packages/core/Cargo.toml` (se precisar de `thiserror` que já está; `once_cell` ou `std::sync::OnceLock` para memoize — preferir `OnceLock` para evitar dep nova).

OUT: qualquer arquivo em `apps/`. Migração de call-sites é W2.

## Role

rt (struct em `packages/core`, mas consumida por `apps/rt`)
