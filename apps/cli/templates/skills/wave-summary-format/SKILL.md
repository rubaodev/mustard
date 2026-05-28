---
name: wave-summary-format
description: Esquema canonico do `_summary.md` (7 secoes obrigatorias) gerado ao fim de cada onda Mustard. Use when escrevendo o summary ao fechar uma onda, ao create artefato de fechamento, ou ao add/consumir herança no `_context.md` da onda N+1 (resume-bootstrap).
tags: [any, docs, wave-close]
appliesTo: []
scope: [wave-close, review]
metadata:
  generated_by: foundation
  spec_origin: 2026-05-27-mustard-v4-foundation
source: manual
disable-model-invocation: true
---

# Esquema do `_summary.md`

> O `_summary.md` é o artefato fechado ao final de cada onda. Resume tudo que a próxima onda precisa para herdar o trabalho — sem ler git log, sem reabrir o spec.md. O harness escreve esse arquivo via `mustard-rt run wave-summary --spec <name> --wave <N>`; o esquema das 7 seções abaixo é obrigatório (AC-A-8).

## Localização

`{.claude/spec/<spec>}/wave-<N>-<role>/_summary.md`

## As 7 seções (na ordem)

A ordem importa — `resume-bootstrap` (W6) lê as seções por nome, mas tooling de cap de palavras lê por offset. Não mude a sequência.

### 1. `## Objetivo`

Uma a três linhas. O que a onda se propôs a entregar. Conteúdo livre, mas verbo no infinitivo + objeto direto. Sem bullets.

### 2. `## Herança`

Lista de wikilinks para as ondas anteriores cujo `_summary.md` esta onda consumiu. Um por linha, prefixado por `- `.

```markdown
- [[wave-2-rt]]
- [[memory/scan-rust-first]]
```

### 3. `## Decisões`

Bullets com decisões não-óbvias tomadas durante a onda. Cada bullet é uma frase. Pular se a onda foi 100% mecânica (mas avalie antes — quase nunca é).

### 4. `## Código`

Tabela das funções tocadas, derivada de `## Funções tocadas` do spec.md da onda:

```markdown
| qualifier | status | path |
|-----------|--------|------|
| `wave_summary::build` | NOVO | `apps/rt/src/run/` |
```

Status válidos: `NOVO`, `ESTENDIDO`, `MODIFICADO` (vocabulário pt-BR canônico, conforme `mustard_core::spec::touched_functions::FunctionStatus::label`).

### 5. `## Critérios de Aceitação`

Bullets com o id de cada AC e o resultado. Nota opcional após em-dash.

```markdown
- AC-A-8: pass
- AC-A-9: pass — 12-wave fixture renderizou 4.231 palavras
```

### 6. `## Verdict`

Uma linha: `{label} — {mensagem}`. O label vem de `gate.verdict.{green,amber,red}.label` (catálogo i18n) e a mensagem do par `.message`. Quando a onda não rodou o gate de regressão (doc-only, refactor trivial), use o placeholder do catálogo.

### 7. `## Próximos passos`

Wikilinks para a próxima onda, sub-specs criadas, ou notas de memória abertas. Mesmo formato da seção `## Herança`.

## Regras de idempotência

- O renderer (`mustard-rt run wave-summary`) é puro: mesmo input + mesmo locale → output byte-identical. Não escreva timestamps no corpo do summary — eles são metadados do `meta.json` da onda, não do markdown.
- Wikilinks `[[ ]]` são a única forma de referência. Não use `<a href>`, não use caminhos crus — o footer auto-gerado de `atomic_md::wikilink` resolve a navegação.
- Ao re-rodar a geração após uma correção, o `write_atomic` substitui o arquivo inteiro. Edições manuais entre gerações serão perdidas — capture decisões reentrantes em `memory/`, não no summary.

## Quando esta skill carrega

Quando o agente está fechando uma onda, escrevendo um summary, ou consumindo um summary anterior como herança. Para a onda N+1, o `_context.md` (descrito na skill irmã) é a fonte canônica derivada do `_summary.md` da onda N.

## Referências cruzadas

- [[heading.summary.objective]] / [[heading.summary.inheritance]] / [[heading.summary.decisions]] / [[heading.summary.code]] / [[heading.summary.ac]] / [[heading.summary.verdict]] / [[heading.summary.next_steps]] — chaves do catálogo i18n consumidas pelo renderer.
- `apps/rt/src/run/wave_summary.rs` — implementação canônica.
- `mustard_core::spec::touched_functions` — fonte do vocabulário `NOVO/ESTENDIDO/MODIFICADO` consumido pela seção `## Código`.

> Follow-up: este SKILL.md está autorizado em pt-BR como exceção (W3 da spec A v4). Os templates em `apps/cli/templates/skills/` hoje são EN; uma onda futura pode introduzir localização por `mustard.json#lang` (issue aberta no spec memory).
