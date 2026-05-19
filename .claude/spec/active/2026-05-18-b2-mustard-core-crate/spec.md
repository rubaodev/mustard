# Feature: b2-mustard-core-crate

### Status: draft | Phase: PLAN | Scope: full
### Checkpoint: 2026-05-18T00:00:00Z
### Lang: pt

> Spec de backlog (Parte B, item B2). ÉPICO em rascunho grosso — criada em lote. Decompõe no ANALYZE. Depende de B1.

## Contexto

A migração para Rust (B3-B5) vai portar 31 hooks, 28 scripts e a CLII. Se cada um for portado isoladamente, a lógica compartilhada — leitura e escrita do log de eventos `events.jsonl`, resolução de ambiente do hook (o atual `_lib/hook-env.js`), emissão de métricas (`metrics-emit.js`), leitura de `pipeline-state` — seria reimplementada dezenas de vezes. Antes de portar qualquer hook é preciso um crate Rust de fundação que concentre esse núcleo. O `mustard-core` é a biblioteca compartilhada que hooks, scripts e CLI vão consumir; é o que torna a migração Rust enxuta em vez de caótica.

## Resumo

Criar o crate Rust `packages/core` (`mustard-core`): tipos `serde` para os eventos do harness, leitura/escrita append-only do `events.jsonl`, leitura/escrita de `pipeline-state`, resolução de ambiente de hook (equivalente ao `_lib/hook-env.js`), e emissão de métricas. É a fundação consumida por B3, B4 e B5.

## Entidades

N/A — biblioteca de infraestrutura.

## Component Contract

N/A.

## Arquivos

- `packages/core/Cargo.toml`, `packages/core/src/lib.rs`
- `packages/core/src/events.rs` — schema serde + I/O do `events.jsonl`
- `packages/core/src/pipeline_state.rs` — leitura/escrita de `.pipeline-states/*.json`
- `packages/core/src/hook_env.rs` — porta de `_lib/hook-env.js` (shouldRun, isSelfDelegation)
- `packages/core/src/metrics.rs` — porta de `_lib/metrics-emit.js`
- `Cargo.toml` raiz — registrar o crate no workspace

## Limites

- `packages/core/`, `Cargo.toml` raiz
- **Fora dos limites:** os hooks/scripts/CLI em si (consomem o crate em B3-B5); o JS atual permanece intocado até ser portado.

## Tarefas

### Wave 1 — schema e I/O de eventos

- [ ] Definir os tipos `serde` dos eventos do harness a partir de `events.jsonl` real e de `_lib/harness-event.js`.
- [ ] Implementar leitura (replay) e escrita append-only do `events.jsonl`, com as projeções essenciais.

### Wave 2 — estado, ambiente, métricas

- [ ] Portar `hook-env.js` (`shouldRun`, `isSelfDelegation`, resolução de cwd/sessão).
- [ ] Portar leitura/escrita de `pipeline-state`.
- [ ] Portar `metrics-emit.js`.
- [ ] Testes `cargo test` cobrindo paridade com o comportamento JS.

## Dependências

- B1 (monorepo) — o crate vive no workspace novo.
- Pré-requisito de B3, B4 e B5.

## Preocupações

- **Paridade comportamental:** o crate precisa reproduzir exatamente o comportamento dos `_lib/*.js` atuais — qualquer divergência propaga para todos os hooks. Os testes JS existentes (`hooks/__tests__/`) são a referência de paridade.
- **Fail-open:** o crate deve oferecer APIs que nunca causem panic em I/O — o padrão fail-open dos hooks depende disso.

## Critérios de Aceitação

- [ ] AC-1: O crate existe e está no workspace — Command: `node -e "const fs=require('fs');if(!fs.existsSync('packages/core/Cargo.toml'))process.exit(1)"`
- [ ] AC-2: O crate compila e os testes passam — Command: `bash -c 'cd packages/core && cargo test'`

## Não-Objetivos

- Não portar nenhum hook/script aqui — só a biblioteca compartilhada.
- Não remover os `_lib/*.js` ainda — eles saem quando o último consumidor JS for portado (fim de B3/B4).
