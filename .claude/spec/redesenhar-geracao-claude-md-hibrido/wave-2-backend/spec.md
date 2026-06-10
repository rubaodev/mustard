# wave-2-backend

## Resumo

Camada de enriquecimento (enrich) orquestrada. Adiciona os comandos `mustard-rt run scan-guards-list` (varre a árvore e lista os `CLAUDE.md` com Guards `pending` + os fatos, EXCLUINDO a raiz) e `scan-guards-apply` (recebe o texto produzido por um agente e faz o splice não-destrutivo no bloco de Guards, com teto de linhas, idempotente). Adiciona o papel (role) `guards` ao `agent-prompt-render`. Nenhuma chamada de LLM dentro do Rust — o Rust só lista e encaixa; quem escreve é o agente, despachado na Onda 3.

## Arquivos

- `apps/rt/src/commands/scan_guards/mod.rs` — (novo) módulo + `Options`/entry, no padrão dos demais comandos sob `commands/`.
- `apps/rt/src/commands/scan_guards/list.rs` — (novo) `scan-guards-list`: glob de `**/CLAUDE.md`, detecta o marcador `pending` (formato emitido na Onda 1), exclui o `CLAUDE.md` da raiz, emite JSON `[{path, subproject, kind, frameworks, facts}]`. Fail-open: erro de IO degrada para `[]`, exit 0.
- `apps/rt/src/commands/scan_guards/apply.rs` — (novo) `scan-guards-apply --path <p> --guards <texto|->`: localiza o bloco-sentinela de Guards, substitui SÓ o miolo pelo texto do agente (preserva todo o resto), impõe teto de linhas (3-6), remove o marcador `pending`, idempotente (reaplicar não duplica). Recusa a raiz.
- `apps/rt/src/commands/mod.rs` — registrar as duas variantes no enum `RunCmd` + dispatch no `match`.
- `apps/rt/src/commands/agent/agent_prompt_render.rs` — suportar `--role guards`: `build_role_block` ganha o caso `guards` com instrução explícita — escrever 3-6 linhas de do/don't FUNDAMENTADAS nos fatos recebidos, só o que NÃO é auto-inferível do manifesto/árvore, no `{spec_lang}` e no tom do `mustard.json`, capado; nunca prosa genérica.
- Testes: `apps/rt` `scan_guards` (list encontra pending e exclui raiz; apply faz splice não-destrutivo, respeita o cap e é idempotente) e `guards_prompt_lang` (o prompt do role `guards` carrega o `spec_lang`/tom).

## Tarefas

- [ ] T2.1 — implementar `scan-guards-list` (varredura + detecção de `pending` + exclusão da raiz + JSON, fail-open).
- [ ] T2.2 — implementar `scan-guards-apply` (splice não-destrutivo no bloco de Guards + cap de linhas + idempotência + recusa da raiz).
- [ ] T2.3 — adicionar o role `guards` ao `agent-prompt-render` (instrução fundamentada, capada, language/tone do mustard.json).
- [ ] T2.4 — registrar as duas variantes em `commands/mod.rs` (enum + dispatch).
- [ ] T2.5 — escrever os testes `scan_guards` e `guards_prompt_lang`; `cargo test` + `cargo clippy --all-targets` verdes.

## Critérios de Aceitação

- **AC-4** — `cargo test -p mustard-rt -- scan_guards`
- **AC-5** — `cargo test -p mustard-rt -- guards_prompt_lang`

## Limites

IN: `apps/rt/src/commands/scan_guards/*` (novo), `commands/mod.rs`, `agent/agent_prompt_render.rs` (+ testes). OUT: edição do render determinístico (Onda 1, já feita); SKILL/templates (Onda 3); chamada de LLM no Rust; enrich da raiz. Depende do formato do marcador `pending` emitido pela Onda 1.

## Network

- Parent: [[redesenhar-geracao-claude-md-hibrido]]
- Depends on: [[wave-1-backend]]

<!-- wikilinks-footer-start -->
- [redesenhar-geracao-claude-md-hibrido](?) ⚠ unresolved
- [wave-1-backend](?) ⚠ unresolved
<!-- wikilinks-footer-end -->