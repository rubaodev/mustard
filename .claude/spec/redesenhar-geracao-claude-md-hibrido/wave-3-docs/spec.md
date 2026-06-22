# wave-3-docs

## Resumo

Camada de orquestração e documentação. Cabeia o passo de enrich no SKILL do `/scan` (e no template em `apps/cli/templates`), opt-in: após o render determinístico, havendo Guards `pending` e o gatilho `--enrich`/confirmação, despachar UM agente por subprojeto via `agent-prompt-render --role guards` e aplicar com `scan-guards-apply`. Revisa a regra INVIOLÁVEL "no AI" do `/scan` e garante que `language`/`tone` venham do `mustard.json`. Mantém template ↔ `.claude` em sincronia.

## Arquivos

- `apps/cli/templates/commands/mustard/scan/SKILL.md` — (fonte da verdade) adicionar a seção do passo de enrich opt-in: depois de `mustard-rt run scan`, rodar `mustard-rt run scan-guards-list`; para cada subprojeto com Guards `pending`, rodar o `prompt_cmd` (`agent-prompt-render --role guards`) e despachar via Task; relayar a saída do agente para `mustard-rt run scan-guards-apply`. Revisar a regra INVIOLÁVEL: de "no AI / nunca escreve em subprojeto" para "IA SÓ nos Guards, opt-in, capada, nunca na raiz; o render determinístico continua sem IA".
- `.claude/commands/mustard/scan/SKILL.md` — espelhar verbatim o template (sync template ↔ `.claude`).
- `apps/cli/src/commands/init.rs` e/ou `apps/rt/src/commands/maint/refresh_claude.rs` — atualizar SOMENTE se algum arquivo de template novo precisar ser copiado verbatim pelo init/refresh (não inventar cópia se não houver arquivo novo).
- Garantir que o fluxo de `--role guards` leia `language`/`tone` do `mustard.json` (já implementado na Onda 2; aqui é cabeamento/verificação no SKILL).

## Tarefas

- [ ] T3.1 — escrever o passo de enrich opt-in no SKILL do `/scan` (template): list → dispatch por subprojeto via `agent-prompt-render --role guards` → `scan-guards-apply`.
- [ ] T3.2 — revisar a regra INVIOLÁVEL do `/scan` (IA só nos Guards, opt-in, capada, nunca na raiz).
- [ ] T3.3 — espelhar o template em `.claude/commands/mustard/scan/SKILL.md` (sync verbatim).
- [ ] T3.4 — atualizar `init.rs`/`refresh_claude` só se houver template novo a copiar.
- [ ] T3.5 — validar o workspace inteiro: `cargo test && cargo clippy --all-targets` verdes; conferir AC-7 (`rg -n "enrich" .../scan/SKILL.md`).

## Critérios de Aceitação

- **AC-7** — `rg -n "enrich" apps/cli/templates/commands/mustard/scan/SKILL.md .claude/commands/mustard/scan/SKILL.md`
- **AC-8** — `cargo test && cargo clippy --all-targets`

## Limites

IN: `apps/cli/templates/commands/mustard/scan/SKILL.md`, `.claude/commands/mustard/scan/SKILL.md`, e (condicional) `init.rs`/`refresh_claude`. OUT: lógica de render/comandos (Ondas 1-2, já feitas); dashboard; chamada de LLM no Rust. Depende dos comandos da Onda 2.

## Network

- Parent: [[redesenhar-geracao-claude-md-hibrido]]
- Depends on: [[wave-2-backend]]

<!-- wikilinks-footer-start -->
- [redesenhar-geracao-claude-md-hibrido](?) ⚠ unresolved
- [wave-2-backend](?) ⚠ unresolved
<!-- wikilinks-footer-end -->