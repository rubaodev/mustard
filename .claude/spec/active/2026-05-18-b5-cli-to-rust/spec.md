# Feature: b5-cli-to-rust

### Status: draft | Phase: PLAN | Scope: full
### Checkpoint: 2026-05-18T18:00:00Z
### Lang: pt

> Spec de backlog (Parte B, item B5). ÉPICO em rascunho grosso. Depende de B2; idealmente após B3/B4 para reaproveitar lógica já portada. Revisada 2026-05-18: nomeação do crate, caminhos do monorepo e fronteira com `mustard-core`.

## Contexto

A CLI do Mustard — `init`, `update`, `add`, `config`, `review` — é TypeScript executado via Bun, hoje em `packages/cli/src/*.ts`. Para o app Tauri instalar o Mustard ao selecionar uma pasta, essa lógica precisa rodar a partir do app, e o backend do Tauri é Rust. Portar a CLI para Rust elimina a necessidade de um sidecar ou de embarcar um runtime: o app Tauri e o motor de instalação passam a ser o mesmo workspace Rust, e o `init` vira uma chamada nativa. O `init`/`update` também passam a gravar um carimbo de versão no `.claude/` do projeto — algo que hoje não existe e que o dashboard (B6) precisa para detectar a versão instalada.

## Resumo

Portar a CLI (`init`, `update`, `add`, `config`, `review` e a lógica de scan de stack) de `packages/cli/src/*.ts` para Rust, consumindo `mustard-core`. O crate Rust ocupa o próprio `packages/cli/` (crate `mustard-cli`, binário `mustard`), parsing via `clap`; o diretório `packages/cli/templates/` permanece como payload de dados, intocado. O `init`/`update` passam a gravar um carimbo de versão (campo em `mustard.json` ou `.claude/.mustard-version`). Após o porte, o app Tauri invoca o `init` nativamente.

## Entidades

N/A — infraestrutura de CLI.

## Component Contract

N/A.

## Arquivos

- `packages/cli/Cargo.toml`, `packages/cli/src/` — crate Rust `mustard-cli` (binário `mustard` + `lib` para o Tauri); substitui o `src/*.ts` atual conforme portado
- `packages/cli/templates/` — payload copiado por `init`; permanece markdown/JSON, intocado
- Carimbo de versão no `.claude/` gerado por `init`/`update`
- `Cargo.toml` raiz — registrar `packages/cli` no workspace

## Limites

- `packages/cli/` (código da CLI Rust + `Cargo.toml`), o carimbo de versão, `Cargo.toml` raiz
- **Fora dos limites:** os hooks/scripts (B3/B4); o conteúdo de `packages/cli/templates/` (payload, não código); o app Tauri em si (B6 conecta a chamada).

## Tarefas

> Estrutura provisória — o ANALYZE refina após inventariar o `src/` real.

### Impl Agent (Wave 1) — init e payload

- [ ] Portar `init`: scan de stack, cópia de `packages/cli/templates/` para `.claude/`, geração de `mustard.json`.
- [ ] `init`/`update` gravam o carimbo de versão do Mustard no `.claude/`.

### Impl Agent (Wave 2) — update, add, config, review

- [ ] Portar `update` (backup + regeneração de core files, preservando arquivos de usuário).
- [ ] Portar `add`, `config`, `review`.

### Impl Agent (Wave 3) — integração Tauri

- [ ] Expor o `init`/`update` Rust como função invocável pelo backend do app Tauri (sem sidecar) — o crate expõe `lib` além do `bin`.

## Dependências

- B2 (`mustard-core`) — contrato, I/O e workspace (edition 2024, `clap` 4.6, `anyhow`).
- Após B3/B4 — reaproveita lógica e scanners já em Rust; evita re-portar o que já existe.

## Preocupações

- **CLAUDE.md desatualizado:** a seção `## Structure` cita `src/scanners/` e `src/generators/`, mas o `src/` real tem `cli.ts`, `commands/`, `mcp/`, `migrate/`, `runtime/`, `services/` — sem `scanners/` nem `generators/`. O ANALYZE precisa inventariar o `src/` real (12 arquivos `.ts`) antes de planejar o porte.
- **Fronteira com `mustard-core`:** `runtime/event-store.ts` **não** é re-portado aqui — sua lógica vai para `mustard-core` (`io/event_store.rs`, B2); a CLI consome o crate. O `mcp/mustard-memory.ts` precisa de decisão no ANALYZE: o servidor MCP de memória **não está wired** em `.mcp.json` (só `context7`) — confirmar se ainda é necessário antes de portar.
- **Paridade do `init`:** `init` é a operação mais visível ao usuário; qualquer divergência de comportamento aparece na primeira instalação. Os testes JS atuais são o oráculo.
- **Crate e payload no mesmo diretório:** `packages/cli/` passa a conter o crate Rust E o diretório de dados `templates/`. O `Cargo.toml` compila só `src/` — garantir que `templates/` não seja arrastado como módulo.

## Critérios de Aceitação

- [ ] AC-1: A CLI Rust compila — Command: `bash -c 'cargo build -p mustard-cli'`
- [ ] AC-2: `init` numa pasta limpa gera `.claude/` com o carimbo de versão — Command: `bash -c 'cargo test -p mustard-cli init'`
- [ ] AC-3: O binário `mustard` expõe os subcomandos — Command: `bash -c 'cargo run -p mustard-cli -- --help | grep -qi init'`

## Não-Objetivos

- Não portar o conteúdo de `packages/cli/templates/` — é payload markdown/JSON, permanece arquivo de texto.
- Não construir a UI de instalação — isso é B6.
- Não manter a CLI JS em paralelo após o porte concluído.
