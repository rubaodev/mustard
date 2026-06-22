# Suporte agnóstico a Flutter e Dart: linkar gramática tree-sitter-dart-orchard, registrar a linguagem no scan e no core, criar queries scm, semear stack Flutter e marcadores de gerado, seguindo Open Closed sem branch por linguagem

<!-- drafter:tone=didactic — Write this spec narrative in didactic tone — expand abbreviations on first use (AC = Acceptance Criteria, wave = onda) and prefer plain words over jargon. -->

<!-- PRD -->

## Contexto

Suporte agnóstico a Flutter e Dart: linkar gramática tree-sitter-dart-orchard, registrar a linguagem no scan e no core, criar queries scm, semear stack Flutter e marcadores de gerado, seguindo Open Closed sem branch por linguagem.

**Arquitetura verificada (dois sistemas de gramática separados).** O mustard tem DUAS vias de extração de AST (árvore sintática), e Dart precisa entrar nas duas:

1. **`apps/scan`** — o binário que produz `grain.model.json` (o modelo durável que a pipeline consome). Resolve linguagem→gramática de forma data-driven via `languages.toml` (`apps/scan/build.rs:88-91` interpola o campo `grammar`), mas a gramática é um crate compilado pinado no `Cargo.toml`. Hoje um arquivo `.dart` é lido (LOC/conteúdo) mas **minerado vazio** (0 declarações/imports) — `apps/scan/src/main.rs:254` cai em `Extracted::default()`. Sem panic, mas sem símbolos.
2. **`packages/core`** — extração de AST em três camadas (in-crate → `loader` de `~/.config/tree-sitter/` → `wasm` sob demanda → extrator textual). O match in-crate é `packages/core/src/domain/ast/loader.rs:453-467` (`builtin_grammars()`). Um `.dart` aqui já recebe extração textual degradada (`signature.rs`/`entity.rs` por palavras-chave) — nunca quebra, só perde precisão.

**Premissa-linchpin verificada empiricamente:** `tree-sitter-dart-orchard 0.3.2` depende apenas de `tree-sitter-language ^0.1` (padrão moderno `LANGUAGE: LanguageFn`, sem pinar o runtime tree-sitter) → **ABI-compatível com o tree-sitter 0.26** do workspace e plugável no mesmo contrato `.into()` dos 7 grammars atuais. Logo, pinar Dart é **extensão Open/Closed** de um registry data-driven (uma linha igual às outras), não special-casing.

**Já pronto (não tocar):** `apps/scan/manifests.toml` reconhece `pubspec.yaml` (kind=pub); `skip_dirs` já tem `.dart_tool`; `test-dirs.toml` cobre o segmento `test/` (Dart usa `test/`).

**Âncoras reais a espelhar:** `apps/scan/languages.toml`, `apps/scan/queries/rust/tags.scm` (molde de roles), `apps/scan/generated-markers.toml`, `packages/core/src/domain/ast/loader.rs`, `packages/core/src/domain/ast/queries.rs`, `packages/core/src/domain/ast/queries_builtin/rust/*.scm` (molde), `packages/core/src/domain/vocabulary/stacks.toml`.

**Por que agora.** O usuário desenvolve em Flutter/Dart (toolchain `fvm`/`dart-sdk` já no PATH) e quer que o scan/digest/feature minere repositórios Flutter com a mesma fidelidade das outras linguagens.

## Usuários/Stakeholders

Quem se beneficia.

## Métrica de sucesso

Métrica de sucesso.

## Não-Objetivos

O que fica de fora.

## Critérios de Aceitação

- **AC-1** — Workspace compila com a nova dependência de gramática
  Command: `cargo build`
- **AC-2** — O scan minera Dart de verdade (declarações não-vazias + import), não cai no fallback vazio
  Command: `cargo test -p scan --test dart_mining_e2e`
- **AC-3** — Paridade de kinds: todo kind declarado (incluindo `mixin`) é produzido pela fixture — prova que nenhum pattern `.scm` foi descartado em silêncio
  Command: `cargo test -p scan --test kinds_parity`
- **AC-4** — Suíte completa do scan verde com Dart adicionado (sem regressão nas outras linguagens)
  Command: `cargo test -p scan`
- **AC-5** — Guard de honestidade do scope-classify (defeito companheiro)
  Command: `cargo test -p mustard-rt --lib scope_decompose`

<!-- PLAN -->

## Arquivos

Suporte Dart — scan (`apps/scan`, 100% data-driven, `src/` intocado):
- `apps/scan/Cargo.toml` — dep `grammar_dart = { package = "tree-sitter-dart-orchard", version = "0.3" }`
- `apps/scan/languages.toml` — entrada `[[language]]` dart (extensions/dir/grammar)
- `apps/scan/queries/dart/tags.scm` (novo) — `@import` + `@definition.{class,mixin,enum,extension,method}`
- `apps/scan/queries/dart/supertypes.scm` (novo) — `@supertype` via `superclass:`/`interfaces:`
- `apps/scan/generated-markers.toml` — globs `*.g.dart`/`*.freezed.dart` + lockfile `pubspec.lock`
- `apps/scan/queries/kinds-manifest.toml` — entrada `[dart]`
- `apps/scan/tests/fixtures/{graph_dart,flutter_app}/**` (novas fixtures)
- `apps/scan/tests/dart_mining_e2e.rs` (novo teste e2e)

Suporte Dart — core (`packages/core`, extensão Open/Closed das tabelas-registry):
- `packages/core/Cargo.toml` — dep `tree-sitter-dart-orchard = "0.3"`
- `packages/core/src/domain/ast/loader.rs` — uma linha em `builtin_grammars()`
- `packages/core/src/domain/ast/queries.rs` — duas linhas em `BUILTIN_QUERIES`
- `packages/core/src/domain/ast/queries_builtin/dart/{entity_definitions,import_edges}.scm` (novos)
- `packages/core/src/domain/vocabulary/stacks.toml` — seed `[[stack]]` flutter

Defeitos de tool companheiros (resolvidos junto):
- `apps/rt/src/commands/spec/scope_decompose.rs` — guard de honestidade (`filesSectionEmpty`/`warning`) + teste
- `.claude/commands/mustard/feature/SKILL.md` + `apps/cli/templates/commands/mustard/feature/SKILL.md` — nota de ordenação
- `packages/core/src/platform/hook_resolve.rs` (novo) + `mod.rs`/`lib.rs`; `apps/cli/src/commands/{init,update}.rs`; `apps/rt/src/commands/maint/rehook.rs` — hooks **e `statusLine`** resolvem `mustard-rt` por caminho absoluto (independente de PATH); `rewrite_statusline_value` irmão de `rewrite_hooks_value` + 3 testes

## Limites

IN: registrar Dart como linguagem nos dois sistemas de gramática (scan + core), de forma agnóstica (só dado + tabelas uniformes, zero `if lang=="dart"`); detecção de stack Flutter; marcadores de gerado; os dois defeitos de tool conversados (scope-classify honesto, hook PATH).
OUT: refatorar/unificar os dois sistemas de gramática num só; cobrir `mcpServers` (binário `mustard-mcp`, distinto) no fix de PATH — `statusLine` já coberto; teste de compilação de builtin-queries no core para TODAS as linguagens (melhoria transversal à parte).

<!-- wikilinks-footer-start -->
- [language](?) ⚠ unresolved
- [stack](?) ⚠ unresolved
<!-- wikilinks-footer-end -->