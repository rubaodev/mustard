# redesenhar geracao CLAUDE.md hibrido enrich IA Guards lean monorepo

<!-- drafter:tone=didactic — Write this spec narrative in didactic tone — expand abbreviations on first use (AC = Acceptance Criteria, wave = onda) and prefer plain words over jargon. -->

<!-- PRD -->

## Contexto

Os arquivos `CLAUDE.md` gerados pelo `/scan` hoje são determinísticos (sem IA) e de baixa utilidade: trazem `## Stack` (uma lista de dependências que o agente já lê do `Cargo.toml`/`package.json`), `## Commands` (genuinamente útil) e `## Guards` **vazio** (só um seed). A camada de enriquecimento (enrich) por IA que preenchia Guards já existiu (commits `0a8efe2`→`7580c3e`) e foi **removida** quando a engine antiga de scan foi substituída pela crate `apps/scan` (grain) + o `scan_claude.rs` enxuto (`c2bd1dd`/`80201db`). Daí a percepção "mexemos e não melhorou".

Decisão do usuário: **modelo híbrido** — fatos determinísticos minerados pelo grain + IA **só** na prosa que não dá para minerar (Guards/gotchas/"não faça X"), com teto rígido de tamanho, modelo de monorepo em dois níveis (raiz enxuta + por-pacote, carga sob demanda) e respeitando `language`/`tone` do `mustard.json`. O conserto mora no tool (`apps/scan` + `apps/rt` + `apps/cli/templates`), não na spec — [[feedback-mustard-fix-tool-not-spec]].

Reconciliação com min-IA/max-Rust ([[project-mustard-functional-refactor]]): o critério é **estado→Rust, julgamento→LLM**. Escrever Guards é julgamento (como redigir AC = Acceptance Criteria, ou mensagens de commit — já preservados como território de LLM na spec `pipeline-min-ia-max-rust`). Por isso a IA roda **orquestrada**: um agente escreve os Guards; o Rust faz o splice (recorte/encaixe) não-destrutivo via sentinela — sem chamar LLM dentro do Rust, sem chaves de API no binário.

Carregamento real (documentação oficial do Claude Code): só a raiz + ancestrais + `~/.claude/CLAUDE.md` carregam a cada turno; os `CLAUDE.md` de subprojeto carregam **sob demanda** (1×/sessão). A alavanca real é **utilidade**, não tamanho — mas o teto de tamanho permanece como guarda-corpo.

Âncoras (do scan + ANALYZE): `apps/cli/src/commands/init.rs` (cópia de template); `apps/scan/src/main.rs` (`build_projects`, bug da raiz-dupla); `apps/rt/src/commands/scan_claude.rs` (render + sentinela); `apps/rt/src/commands/agent/agent_prompt_render.rs` (dispatch de agente).

## Usuários/Stakeholders

Qualquer pessoa que use o Claude Code neste repositório (e em repositórios inicializados pelo mustard). Ganham `CLAUDE.md` com Guards genuinamente úteis (gotchas reais, não auto-inferíveis), sem o dump de dependências, com custo de token controlado e gasto de IA explícito (opt-in, ou seja, sob demanda — não a cada turno).

## Métrica de sucesso

- A raiz deixa de exibir `· 0 arquivos` (bug da raiz-dupla corrigido no grain).
- `## Stack` deixa de enumerar dependências; mantém só `Tipo:` + sinal de framework relevante quando houver. `## Commands` preservado.
- `## Guards` vira um bloco enriquecível com marcador `pending`; um passo de IA orquestrado escreve 3–6 linhas de do/don't fundamentadas, em `pt-BR`/`didactic`, com splice não-destrutivo.
- Teto rígido de tamanho imposto no render (não só um aviso).
- O `/scan` (SKILL + template) ganha o passo de enrich opt-in; a regra INVIOLÁVEL "no AI" é revista para "IA só nos Guards, opt-in, capada, nunca na raiz".
- Workspace verde (build/test/clippy).

## Não-Objetivos

- NÃO chamar LLM dentro do Rust nem embutir chaves de API no binário.
- NÃO enriquecer o `CLAUDE.md` **da raiz** (ele é parseado para extrair os subprojetos) nem os prompts de sistema dos agentes.
- NÃO reintroduzir a engine de scan antiga (`scan_orchestrate`/`enrich_block`/`guards_seed`, já deletadas) — o enrich é reconstruído sobre a arquitetura nova grain→render.
- NÃO tornar o enrich automático a cada `/scan` por padrão (o gasto de IA é deliberado/opt-in).
- NÃO mexer no dashboard nem em specs já geradas.

## Critérios de Aceitação

- **AC-1** — A raiz não mostra mais "0 arquivos" (raiz-dupla deduplicada no grain).
  Command: `cargo test -p scan -- root_dedup`
- **AC-2** — `render_stack` não enumera dependências (sem dump de deps; só `Tipo:` + sinal de framework).
  Command: `cargo test -p mustard-rt -- render_stack`
- **AC-3** — O render determinístico emite Guards como bloco enriquecível com marcador `pending` + os fatos para o agente.
  Command: `cargo test -p mustard-rt -- guards_pending`
- **AC-4** — Existem os comandos `scan-guards-list` (lista pendentes) e `scan-guards-apply` (splice não-destrutivo, capado).
  Command: `cargo test -p mustard-rt -- scan_guards`
- **AC-5** — O prompt do agente de Guards carrega `language`/`tone` do `mustard.json`.
  Command: `cargo test -p mustard-rt -- guards_prompt_lang`
- **AC-6** — Teto rígido de tamanho imposto no render (não só aviso).
  Command: `cargo test -p mustard-rt -- claude_md_hard_cap`
- **AC-7** — O SKILL do `/scan` + template descrevem o passo de enrich e a regra revista; template↔`.claude` sincronizados.
  Command: `rg -n "enrich" apps/cli/templates/commands/mustard/scan/SKILL.md .claude/commands/mustard/scan/SKILL.md`
- **AC-8** — Workspace inteiro verde.
  Command: `cargo test && cargo clippy --all-targets`

<!-- PLAN -->

## Entidades

Sem entidade de domínio nova. O trabalho incide sobre: o miner grain (`apps/scan`), o render determinístico + comandos de orquestração (`apps/rt`) e os prompts/SKILL (`apps/cli/templates` + `.claude`). Novidade conceitual: um "papel" (role) de agente `guards` e um bloco-sentinela enriquecível para Guards.

## Arquivos

**Onda 1 — fatos determinísticos (grain + render):**
- `apps/scan/src/main.rs` (`build_projects`, ~l.265-303) — deduplicar projetos-raiz (`dir == ""`): fundir mantendo o de maior `code_files`. Corrige o "0 arquivos".
- `apps/scan/tests/facts_cli.rs` — teste `root_dedup`.
- `apps/rt/src/commands/scan_claude.rs` — (a) `render_stack` (~l.99): podar o dump de dependências → `Tipo:` + sinal de framework; (b) bloco enriquecível de Guards com marcador `pending` carregando os fatos (kind/frameworks/módulos-chave), substituindo o `<!-- seed -->` morto (~l.180-193); (c) teto rígido em `run_full` (~l.274-317) via nova constante de cap, ao lado de `CLAUDE_MD_WARN_BYTES` (l.14).
- Testes em `apps/rt` para `render_stack`, `guards_pending`, `claude_md_hard_cap`.

**Onda 2 — camada de enrich orquestrada (comandos novos + role):**
- `apps/rt/src/commands/scan_guards/mod.rs` + `list.rs` + `apply.rs` (módulo novo): `scan-guards-list` (varre a árvore, lista `CLAUDE.md` com Guards `pending` + os fatos, **exclui a raiz**) e `scan-guards-apply` (recebe o texto do agente, faz splice não-destrutivo no bloco de Guards, impõe cap de linhas, idempotente).
- `apps/rt/src/commands/mod.rs` — registrar as duas variantes + dispatch.
- `apps/rt/src/commands/agent/agent_prompt_render.rs` — suportar `--role guards`: o prompt instrui 3–6 linhas do/don't fundamentadas nos fatos, só o não-inferível, no `{spec_lang}`/tom, capado.
- Testes `scan_guards` (list + apply, incluindo splice não-destrutivo e cap) e `guards_prompt_lang`.

**Onda 3 — SKILL + templates + idioma/tom:**
- `apps/cli/templates/commands/mustard/scan/SKILL.md` (+ `.claude/commands/mustard/scan/SKILL.md`) — adicionar o passo de enrich opt-in (após o render determinístico: havendo Guards `pending` e `--enrich`/confirmação, despachar **um agente por subprojeto** via `agent-prompt-render --role guards`, depois `scan-guards-apply`); revisar a regra INVIOLÁVEL ("IA só nos Guards, opt-in, capada, nunca na raiz").
- Garantir `language`/`tone` lidos do `mustard.json` no fluxo do prompt.
- `apps/cli/src/commands/init.rs` / `refresh_claude` — atualizar caso algum template novo precise ser copiado verbatim.

## Tarefas

- T1 — Onda 1: dedup de raiz no grain + podar Stack + bloco Guards `pending` + teto rígido; testes verdes.
- T2 — Onda 2: módulo `scan_guards` (`list` + `apply`) + role `guards` no render de prompt + registro + testes.
- T3 — Onda 3: passo de enrich no SKILL `/scan` (+ template), regra revista, idioma/tom, sync template↔`.claude`.

## Dependências

Onda 1 → Onda 2 (o `apply` depende do formato do marcador `pending` emitido pela Onda 1). Onda 2 → Onda 3 (o SKILL cabeia os comandos da Onda 2). Sequencial.

## Limites

IN: `apps/scan`, `apps/rt` (`scan_claude`, `scan_guards`, `agent_prompt_render`, `mod.rs`), `apps/cli/templates` + `.claude/commands/mustard/scan`, testes. OUT: chamada de LLM no Rust; enrich da raiz; engine de scan antiga; dashboard; specs já geradas.

## Concerns

Itens não-bloqueantes (avisos da validação estrutural) e decisões a confirmar na aprovação:

- **Aviso esperado (não é defeito):** `apps/rt/src/commands/scan_guards/{mod,list,apply}.rs` constam como "referenciados mas não encontrados" — são arquivos **net-new** (novos) criados na Onda 2.
- **Decisão — enrich opt-in vs. automático:** a proposta é opt-in (`/scan --enrich`, ou confirmação explícita). O `/scan` padrão segue determinístico e barato, deixando os Guards como `pending`. Trade-off: por padrão os Guards só ficam preenchidos depois do passo de enrich.
- **Decisão — Stack:** podar o dump de dependências mantendo só `Tipo:` (+ sinal de framework quando o grain marcar algum). Alternativa possível: remover a seção `## Stack` por inteiro.
- **Fora de escopo, mas é a maior alavanca de token por-turno:** o peso fixo a cada turno é o `.claude/CLAUDE.md` orquestrador (~4,4 KB) + o `~/.claude/CLAUDE.md` global — e NÃO a saída do scan (arquivo de subprojeto carrega sob demanda). Otimizar esses dois é um trabalho separado.

<!-- wikilinks-footer-start -->
- [feedback-mustard-fix-tool-not-spec](?) ⚠ unresolved
- [project-mustard-functional-refactor](?) ⚠ unresolved
<!-- wikilinks-footer-end -->