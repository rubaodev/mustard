# wave-1-backend

## Resumo

Camada determinística (grain + render). Corrige o bug da raiz-dupla ("0 arquivos"), poda o dump de dependências do `render_stack`, transforma `## Guards` num bloco enriquecível com marcador `pending` (carregando os fatos que o agente de enrich vai usar) e impõe um teto rígido de tamanho no render. Sem IA nesta onda.

## Arquivos

- `apps/scan/src/main.rs` — `build_projects` (~l.265-303): após o `sort_by` por `code_files`, deduplicar os projetos cujo `dir == ""` (raiz), fundindo no de maior `code_files`. É a causa do `Tipo: cargo · 0 arquivos` (hoje o consumidor pega a última raiz, cargo=0).
- `apps/scan/tests/facts_cli.rs` — teste `root_dedup`: um workspace com manifesto npm na raiz (code_files>0) e Cargo.toml na raiz emite UMA só entrada de raiz, com o maior `code_files`.
- `apps/rt/src/commands/scan_claude.rs`:
  - `render_stack` (~l.99): podar o dump de dependências → emitir só `Tipo: {kind}` (+ sinal de framework quando o grain marcar algum como arquitetural; nunca a lista de manifest deps).
  - bloco de Guards (~l.180-193 em `scaffold`): substituir o `<!-- seed DO/DON'T aqui -->` morto por um bloco-sentinela enriquecível com marcador `pending` que carrega os fatos para o agente (kind, frameworks, módulos/anchors-chave). Definir constantes de sentinela próprias para Guards (análogas a `SENTINEL_OPEN`/`SENTINEL_CLOSE`).
  - teto rígido: nova constante `CLAUDE_MD_HARD_CAP_BYTES` (ao lado de `CLAUDE_MD_WARN_BYTES`, l.14) imposta em `run_full` (~l.274-317) ANTES do `write_atomic` — erro claro (não panic, não silencioso) quando o conteúdo renderizado excede o teto.
- Testes em `apps/rt` (no mesmo arquivo ou em `tests/`): `render_stack` (sem enumeração de deps), `guards_pending` (bloco pending + fatos presentes), `claude_md_hard_cap` (excedente é rejeitado).

## Tarefas

- [ ] T1.1 — `apps/scan`: deduplicar projetos-raiz em `build_projects`, mantendo o de maior `code_files`; corrigir o "0 arquivos".
- [ ] T1.2 — `scan_claude::render_stack`: podar o dump de dependências, manter só `Tipo:` + sinal de framework.
- [ ] T1.3 — `scan_claude`: emitir `## Guards` como bloco-sentinela enriquecível com marcador `pending` + os fatos (kind/frameworks/anchors).
- [ ] T1.4 — `scan_claude`: impor teto rígido (`CLAUDE_MD_HARD_CAP_BYTES`) no `run_full`, com erro determinístico ao exceder.
- [ ] T1.5 — escrever os testes `root_dedup`, `render_stack`, `guards_pending`, `claude_md_hard_cap`; rodar `cargo test` + `cargo clippy --all-targets` verdes.

## Critérios de Aceitação

- **AC-1** — `cargo test -p scan -- root_dedup`
- **AC-2** — `cargo test -p mustard-rt -- render_stack`
- **AC-3** — `cargo test -p mustard-rt -- guards_pending`
- **AC-6** — `cargo test -p mustard-rt -- claude_md_hard_cap`

## Limites

IN: `apps/scan/src/main.rs` (+ teste), `apps/rt/src/commands/scan_claude.rs` (+ testes). OUT: comandos novos de enrich (Onda 2); SKILL/templates (Onda 3); chamada de LLM; enrich da raiz.

## Network

- Parent: [[redesenhar-geracao-claude-md-hibrido]]

<!-- wikilinks-footer-start -->
- [redesenhar-geracao-claude-md-hibrido](?) ⚠ unresolved
<!-- wikilinks-footer-end -->