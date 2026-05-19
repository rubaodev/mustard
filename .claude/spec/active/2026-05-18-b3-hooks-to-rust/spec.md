# Feature: b3-hooks-to-rust

### Status: draft | Phase: PLAN | Scope: full
### Checkpoint: 2026-05-18T00:00:00Z
### Lang: pt

> Spec de backlog (Parte B, item B3). **ÉPICO** — portar 31 hooks não cabe numa spec. Rascunho grosso; no ANALYZE decompõe em waves por categoria de hook (provavelmente specs-filhas). Depende de B2.

## Contexto

Hoje os 31 hooks do Mustard são arquivos `.js` copiados para o `.claude/` de cada projeto e executados via `node`/`bun`. Isso cria uma classe inteira de bug: se o runtime não está instalado ou não está no PATH, os hooks falham em silêncio. Hooks também rodam em todo tool-use — o cold-start do interpretador (~40-80 ms) entra no caminho crítico de cada ação. Portar os hooks para um único binário Rust despachador (`mustard-rt <hook>`) elimina a dependência de runtime, derruba o cold-start para ~1 ms, e unifica a linguagem com o app Tauri. O `settings.json` pode apontar uns hooks para `node` e outros para o binário ao mesmo tempo, então a migração é incremental — nada quebra no meio.

## Resumo

Portar os 31 hooks de `templates/hooks/*.js` para subcomandos de um binário Rust `mustard-rt`, consumindo o crate `mustard-core` (B2). O contrato stdin-JSON/stdout-JSON e o fail-open são preservados. O `settings.json` é migrado hook a hook, com runtimes mistos durante a transição.

## Entidades

N/A — infraestrutura de enforcement.

## Component Contract

N/A.

## Arquivos

- `packages/cli/rt/` (ou `packages/rt/`) — crate do binário `mustard-rt`, um módulo por hook
- `templates/settings.json` — apontar cada hook migrado para `mustard-rt <hook>`
- `templates/hooks/*.js` — removidos conforme portados
- `templates/hooks/__tests__/` — testes portados para `cargo test`

## Limites

- O crate `mustard-rt`, `templates/settings.json`, `templates/hooks/`
- **Fora dos limites:** scripts (B4), CLI (B5), a lógica de decisão dos hooks (preservada — só muda a linguagem).

## Tarefas

> Estrutura provisória — o ANALYZE define as waves reais por categoria.

### Wave 1 — despachador + hooks de Bash (gate)

- [ ] Esqueleto do binário `mustard-rt` (parsing de subcomando, leitura de stdin JSON, fail-open global).
- [ ] Portar `bash-safety.js`, `bash-native-redirect.js`, `rtk-rewrite.js`.
- [ ] Migrar esses hooks no `settings.json`; rodar os testes de paridade.

### Wave 2 — hooks de Task/Subagent

- [ ] Portar `subagent-tracker.js`, `model-routing-gate.js`, `context-budget.js`, `tool-use-counter.js`, `output-budget.js`.

### Wave 3 — hooks de Write/Edit e gates de spec

- [ ] Portar `close-gate.js`, `auto-format.js`, `checklist-auto-mark.js`, `spec-size-gate.js`, `file-guard.js`, `enforce-registry.js`, demais gates.

### Wave 4 — hooks de sessão e finalização

- [ ] Portar `session-memory.js`, `session-knowledge*.js`, `memory-auto-extract.js`, `session-cleanup.js`, `pre-compact.js`, restantes.
- [ ] Remover os `_lib/*.js` quando o último consumidor JS sair.

## Dependências

- B2 (`mustard-core`) — todos os hooks consomem o crate.
- B1 (monorepo).

## Preocupações

- **Volume real:** 31 hooks. Este "spec" é um épico; o ANALYZE deve produzir specs-filhas por categoria, não tentar uma só.
- **Paridade:** os testes JS atuais (`hooks/__tests__/`) são o oráculo. Cada hook portado precisa passar os mesmos casos.
- **Loop de dev mais lento:** editar hook passa a exigir `cargo build`. Aceito — hooks são conjunto estável.
- **`settings.json` misto:** durante a transição, comandos de hook apontam para `node` ou `mustard-rt`. Garantir que ambos coexistem sem ambiguidade.

## Critérios de Aceitação

- [ ] AC-1: O binário `mustard-rt` compila — Command: `bash -c 'cargo build --bin mustard-rt'`
- [ ] AC-2: Os testes de hooks portados passam — Command: `bash -c 'cargo test'`
- [ ] AC-3: Nenhum hook migrado ainda aponta para `node`/`bun` no settings — Command: `bash -c 'grep -c "mustard-rt" templates/settings.json'`

## Não-Objetivos

- Não portar scripts nem CLI (B4/B5).
- Não alterar a lógica de decisão de nenhum hook — porte fiel.
- Não fazer big-bang — migração incremental, hook a hook.
