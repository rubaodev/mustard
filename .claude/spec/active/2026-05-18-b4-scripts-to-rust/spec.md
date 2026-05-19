# Feature: b4-scripts-to-rust

### Status: draft | Phase: PLAN | Scope: full
### Checkpoint: 2026-05-18T00:00:00Z
### Lang: pt

> Spec de backlog (Parte B, item B4). **ÉPICO** — portar 28 scripts não cabe numa spec. Rascunho grosso; no ANALYZE decompõe em waves por família de script. Depende de B2; pode rodar em paralelo a B3.

## Contexto

Os 28 scripts em `templates/scripts/` — `sync-detect`, `sync-registry`, `diff-context`, `qa-run`, `metrics`, `spec-extract`, `event-projections`, `wave-tree` e os demais — são invocados pelos comandos do pipeline via `bun`/`node`. Eles têm a mesma fragilidade de runtime dos hooks e, uma vez que os hooks viram Rust (B3), manter os scripts em JS deixaria o Mustard com dois runtimes pela metade. Portar os scripts para subcomandos do mesmo binário `mustard-rt` completa a unificação: um binário, zero dependência de runtime, e os comandos do pipeline passam a invocar `mustard-rt <script>` em vez de `bun .claude/scripts/<script>.js`.

## Resumo

Portar os 28 scripts de `templates/scripts/` para subcomandos do binário `mustard-rt` (B3), consumindo `mustard-core`. Atualizar todos os comandos do pipeline que invocam `bun .claude/scripts/*.js` para chamar `mustard-rt <script>`. Migração incremental, família por família.

## Entidades

N/A — infraestrutura de scripts.

## Component Contract

N/A.

## Arquivos

- `packages/cli/rt/` — módulos de script no binário `mustard-rt`
- `templates/scripts/*.js` — removidos conforme portados
- `templates/scripts/__tests__/` — testes portados para `cargo test`
- `templates/commands/mustard/*/SKILL.md` — atualizar invocações `bun .claude/scripts/*` → `mustard-rt *`
- `templates/refs/**/*.md` — idem onde houver invocação de script

## Limites

- O crate `mustard-rt`, `templates/scripts/`, e as invocações de script em `templates/commands/` e `templates/refs/`
- **Fora dos limites:** hooks (B3), CLI (B5), a lógica dos scripts (porte fiel).

## Tarefas

> Estrutura provisória — o ANALYZE define as waves reais por família de script.

### Wave 1 — descoberta e registry

- [ ] Portar `sync-detect.js`, `sync-registry.js` e a família `registry/*`.
- [ ] Atualizar as invocações nos comandos.

### Wave 2 — pipeline runtime

- [ ] Portar `diff-context.js`, `spec-extract.js`, `wave-tree.js`, `wave-dependency.js`, `scope-decompose.js`, `exec-rewave-check.js`.

### Wave 3 — QA, métricas, eventos

- [ ] Portar `qa-run.js`, `metrics.js`, `event-projections.js`, `verify-pipeline.js`, `complete-spec.js`, restantes.
- [ ] Atualizar todas as invocações remanescentes em `commands/` e `refs/`.

## Dependências

- B2 (`mustard-core`).
- Pode rodar em paralelo a B3 (compartilham o crate e o binário, mas módulos distintos).

## Preocupações

- **Volume real:** 28 scripts. Épico — decompor em specs-filhas por família no ANALYZE.
- **Invocações espalhadas:** cada comando e vários refs invocam scripts por caminho `bun .claude/scripts/*.js`. A integridade dessas referências é o risco central — um grep exaustivo no ANALYZE é obrigatório.
- **RTK:** o `rtk-rewrite` e o uso de `rtk` em invocações de script precisam continuar funcionando com o binário Rust.

## Critérios de Aceitação

- [ ] AC-1: Nenhuma invocação `bun .claude/scripts/` resta nos comandos migrados — Command: `bash -c 'grep -rc "mustard-rt" templates/commands/mustard'`
- [ ] AC-2: O binário compila e os testes passam — Command: `bash -c 'cargo build --bin mustard-rt && cargo test'`

## Não-Objetivos

- Não portar hooks (B3) nem CLI (B5).
- Não mudar o comportamento de nenhum script — porte fiel.
