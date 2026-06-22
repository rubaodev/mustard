# Adicionar suporte a PHP e Laravel ao scanner de forma data-driven e agnostica

<!-- drafter:tone=didactic — Write this spec narrative in didactic tone — expand abbreviations on first use (AC = Acceptance Criteria, wave = onda) and prefer plain words over jargon. -->

<!-- PRD -->

## Contexto

O scanner determinístico do Mustard reconhece hoje cinco linguagens — C#, TypeScript/TSX, Python, Rust e Go — declaradas como **dados** em `apps/scan/languages.toml`. PHP e Laravel não são reconhecidos: não há a extensão `.php`, nem o manifesto `composer.json`, nem sinais do framework Laravel.

A arquitetura do scanner é **agnóstica por contrato**: o motor em `apps/scan/src/` e `packages/core/src/` nunca cita o nome de uma linguagem, extensão, nó de gramática (a estrutura sintática que a `tree-sitter`, biblioteca de parsing, produz) ou framework. Toda especificidade vive em arquivos de dados (`languages.toml`, `manifests.toml`, `frameworks_builtin.toml`, `queries/<lang>/*.scm`) e em uma linha de dependência no `Cargo.toml`. Portanto adicionar PHP é uma **extensão por dado** (princípio Open/Closed do SOLID — aberto a extensão, fechado a modificação), nunca uma mudança de lógica.

Âncoras reais (verificadas na investigação, não pelo slug):
- `apps/scan/languages.toml` — registro de linguagens (única fonte: extensão → linguagem → gramática)
- `apps/scan/Cargo.toml:35-41` — crates de gramática `tree-sitter` linkadas estaticamente (atualmente `tree-sitter = "0.26"`)
- `apps/scan/build.rs:22-68` — lê o registro e embute as `queries/<lang>/*.scm` no binário em tempo de compilação
- `apps/scan/queries/python/{tags,supertypes}.scm` — template das queries de captura (vocabulário genérico)
- `apps/scan/manifests.toml` — registro de build-systems (nome-do-arquivo → ecossistema, deps, scripts)
- `packages/core/src/domain/vocabulary/frameworks_builtin.toml` — sinais Aho-Corasick (casamento de termos) de framework/ORM/injeção-de-dependência
- `apps/scan/tests/` — padrão dos testes de extração

Por que agora: o usuário precisa rodar o Mustard em projetos PHP/Laravel; a base arquitetural já comporta a extensão sem tocar em lógica.

## Usuários/Stakeholders

Desenvolvedores que usam o Mustard em projetos PHP/Laravel (Composer para dependências, Eloquent como ORM, rotas e comandos via Artisan). Hoje o `/scan` desses projetos produz um modelo cego à stack — sem linguagem, sem deps, sem framework.

## Métrica de sucesso

Rodar `mustard-rt run scan` num projeto Laravel real produz um `grain.model.json` em que: (a) arquivos `.php` aparecem com linguagem `php` e suas classes/funções/`use` (imports) extraídos; (b) `composer.json` é reconhecido como build-system, com dependências e scripts; (c) o projeto é rotulado com o framework Laravel — **e** nenhum literal de PHP/Laravel aparece em `src/`.

## Não-Objetivos

- Inferir comandos convencionais de PHP **sem** manifesto (ex.: deduzir `php artisan` na ausência de `composer.json`). Comandos saem dos `scripts` declarados no `composer.json` — caminho data-driven; inferência convencional fica fora (não vamos hardcodar comandos PHP em `command_detect.rs`).
- Tratar Blade (`.blade.php`) como linguagem própria.
- Highlighting ou gramáticas no dashboard.
- Detectar outros frameworks PHP (Symfony, CakePHP) — apenas Laravel neste escopo.

## Critérios de Aceitação

- **AC-1 — O scan compila com a gramática PHP.** A crate `tree-sitter-php` precisa ser compatível com `tree-sitter 0.26` (as gramáticas atuais são 0.23–0.25; resolver a versão é o primeiro risco a fechar).
  Command: `cargo build -p scan`
- **AC-2 — Extração de PHP funciona.** Um teste parseia um `.php` com `namespace`, `use`, `class` e função, e verifica os símbolos extraídos pelo motor genérico (sem nó de gramática em `src/`).
  Command: `cargo test -p scan php_extraction`
- **AC-3 — `composer.json` reconhecido como build-system.** Deps de `require`/`require-dev` e `scripts` aparecem no modelo.
  Command: `cargo test -p scan composer_manifest`
- **AC-4 — Laravel detectado como framework.** Uma amostra contendo `Illuminate\` (e afins) é rotulada Laravel pelo vocabulário.
  Command: `cargo test -p mustard-core laravel`
- **AC-5 — Scan e2e de uma fixture Laravel.** Escanear uma fixture de projeto Laravel mínimo produz um modelo com linguagem `php` + manifesto composer + framework Laravel.
  Command: `cargo test -p scan php_laravel_fixture`
- **AC-6 — Invariante agnóstica (anti-hardcode).** Nenhum dos literais `php`/`laravel`/`composer`/`artisan`/`illuminate` (case-insensitive) aparece em `apps/scan/src/` nem em `packages/core/src/`. O grep deve retornar zero ocorrências.
  Command: `! rg -i "php|laravel|composer|artisan|illuminate" apps/scan/src packages/core/src`

<!-- PLAN -->

## Arquivos

Panorâmico (o detalhe por onda vive nas sub-specs). **Tudo é dado; `src/` permanece intocado.**

- `apps/scan/Cargo.toml` — adicionar `grammar_php = { package = "tree-sitter-php", version = "<compatível com 0.26>" }`.
- `apps/scan/languages.toml` — adicionar `[[language]]` para PHP (`name="php"`, `extensions=["php"]`, `dir="php"`, `grammar` → o `LanguageFn` da crate; confirmar o símbolo exportado, p.ex. `grammar_php::LANGUAGE_PHP`).
- `apps/scan/queries/php/tags.scm` — **novo**: captura genérica (`@definition.class`, `@definition.function`, `@import` para `use`, `@namespace`).
- `apps/scan/queries/php/supertypes.scm` — **novo**: `extends`/`implements` → `@supertype`.
- `apps/scan/manifests.toml` — adicionar `[[manifest]]` para `composer.json` (deps em `require`/`require-dev`, `scripts`).
- `packages/core/src/domain/vocabulary/frameworks_builtin.toml` — adicionar sinais do Laravel (`Illuminate\`, `artisan`, `Route::`, `extends Controller`, `use Illuminate\Database\Eloquent\Model`).
- `apps/scan/tests/` — fixture de projeto Laravel mínimo + teste de extração PHP + teste e2e de scan da fixture.
- **Gate** (verificação, não edição): `apps/scan/src/**` e `packages/core/src/**` ficam idênticos (AC-6).

## Dependências

- **Onda 1 (camada `scan`)** e **Onda 2 (camada `core`)** são independentes — tocam arquivos/subprojetos distintos — e rodam em paralelo (mesmo nível).
- **Onda 3 (fixtures + e2e)** depende das ondas 1 e 2: ela só valida o comportamento integrado depois que a linguagem, o manifesto e os sinais existem.

## Limites

IN: registro por dado (`.toml`), queries `.scm`, a linha de dependência da gramática no `Cargo.toml`, fixtures e testes.
OUT: qualquer lógica em `src/`; Blade; outros frameworks PHP; dashboard; inferência de comandos sem manifesto.

## Concerns

- **Risco técnico (Onda 1, fechar primeiro):** versão de `tree-sitter-php` compatível com `tree-sitter 0.26`. As gramáticas atuais são 0.23–0.25; se não houver release compatível com o core pinado, é uma **decisão de design a escalar** — nunca fazer downgrade do core (invariante `links=tree-sitter` do workspace).
- **WARN de validação (esperado, não-bloqueante):** `analyze-validation` marcou `queries/php/tags.scm`, `queries/php/supertypes.scm` e `composer.json` como "referenced but not found". São arquivos **novos** a criar nas ondas 1 e 3 — falso-positivo do validador, não defeito do plano.

<!-- wikilinks-footer-start -->
- [language](?) ⚠ unresolved
- [manifest](?) ⚠ unresolved
<!-- wikilinks-footer-end -->