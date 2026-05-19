# Feature: b5-cli-to-rust

### Status: draft | Phase: PLAN | Scope: full
### Checkpoint: 2026-05-18T00:00:00Z
### Lang: pt

> Spec de backlog (Parte B, item B5). ÉPICO em rascunho grosso. Depende de B2; idealmente após B3/B4 para reaproveitar lógica já portada.

## Contexto

A CLI do Mustard — `init`, `update`, `add`, `config`, `review` — é TypeScript executado via Bun. Para o app Tauri instalar o Mustard ao selecionar uma pasta, essa lógica precisa rodar a partir do app, e o backend do Tauri é Rust. Portar a CLI para Rust elimina a necessidade de um sidecar ou de embarcar um runtime: o app Tauri e o motor de instalação passam a ser o mesmo workspace Rust, e o `init` vira uma chamada nativa. O `init`/`update` também passam a gravar um carimbo de versão no `.claude/` do projeto — algo que hoje não existe e que o dashboard (B6) precisa para detectar a versão instalada.

## Resumo

Portar a CLI (`init`, `update`, `add`, `config`, `review` e os scanners de stack) de `packages/cli/src/*.ts` para Rust, consumindo `mustard-core`. O `init`/`update` passam a gravar um carimbo de versão (`.claude/.mustard-version` ou campo em `mustard.json`). Após o porte, o app Tauri invoca o `init` nativamente.

## Entidades

N/A — infraestrutura de CLI.

## Component Contract

N/A.

## Arquivos

- `packages/cli/` — porte de `src/cli.ts`, `src/commands/*.ts` para Rust
- Lógica de scan de stack (detecção de tecnologias) — porte
- `templates/` — continua sendo o payload copiado por `init` (markdown/JSON intocados)
- Carimbo de versão no `.claude/` gerado por `init`/`update`

## Limites

- `packages/cli/` (código da CLI), o carimbo de versão
- **Fora dos limites:** os hooks/scripts (B3/B4); o conteúdo de `templates/` (payload, não código); o app Tauri em si (B6 conecta a chamada).

## Tarefas

> Estrutura provisória — o ANALYZE refina.

### Wave 1 — init e payload

- [ ] Portar `init`: scan de stack, cópia de `templates/` para `.claude/`, geração de `mustard.json`.
- [ ] `init`/`update` gravam o carimbo de versão do Mustard no `.claude/`.

### Wave 2 — update, add, config, review

- [ ] Portar `update` (backup + regeneração de core files, preservando arquivos de usuário).
- [ ] Portar `add`, `config`, `review`.

### Wave 3 — integração Tauri

- [ ] Expor o `init`/`update` Rust como função invocável pelo backend do app Tauri (sem sidecar).

## Dependências

- B2 (`mustard-core`).
- Após B3/B4 (reaproveita scanners e lógica já em Rust).

## Preocupações

- **CLAUDE.md desatualizado:** a seção `## Structure` cita `src/scanners/` e `src/generators/`, mas o `src/` real (verificado durante a análise) tem `cli.ts`, `commands/`, `mcp/`, `migrate/`, `runtime/`, `services/` — sem `scanners/` nem `generators/`. O ANALYZE precisa inventariar o `src/` real antes de planejar o porte.
- **`mcp/mustard-memory.ts` e `runtime/event-store.ts`:** há código de MCP e de event-store (SQLite) em `src/` — decidir no ANALYZE se entram no porte ou ficam.
- **Paridade do `init`:** `init` é a operação mais visível ao usuário; qualquer divergência de comportamento aparece na primeira instalação.

## Critérios de Aceitação

- [ ] AC-1: A CLI Rust compila — Command: `bash -c 'cd packages/cli && cargo build'`
- [ ] AC-2: `init` numa pasta limpa gera `.claude/` com o carimbo de versão — Command: `bash -c 'cargo test -p mustard-cli init'`

## Não-Objetivos

- Não portar o conteúdo de `templates/` — é payload markdown/JSON, permanece arquivo de texto.
- Não construir a UI de instalação — isso é B6.
- Não manter a CLI JS em paralelo após o porte concluído.
