---
id: spec.porta-unica-roteamento-linguagem-natural
---

# porta unica roteamento linguagem natural classifica confirma despacha + telemetria pipeline kind por tipo de trabalho

<!-- drafter:tone=didactic — Write this spec narrative in didactic tone — expand abbreviations on first use (AC = Acceptance Criteria, wave = onda) and prefer plain words over jargon. -->

<!-- PRD -->

## Contexto

Hoje o usuário precisa **escolher** entre `/feature`, `/bugfix`, `/task`, `/tactical-fix` — e não sabe qual usar (os nomes misturam dois eixos: intenção × cerimônia). E os caminhos lean (`/task`, `/bugfix` rápido) **não emitem evento de pipeline**, então o dashboard fica cego ao tipo de trabalho e perde a narrativa do que foi pedido.

Esta spec entrega uma **porta única**: o usuário descreve o que quer em linguagem natural; o orquestrador (`CLAUDE.md § Intent Routing`, fonte única) classifica intenção + escopo via `scope-classify` + roteador semântico, **sempre narra** a leitura, **confirma só na ambiguidade**, despacha o fluxo interno e **emite a classificação como evento determinístico `pipeline.kind`** (side-effect, não prosa que a IA possa pular). Os comandos viram **override oculto** (a descrição do frontmatter passa de "Use when the user asks…" para "fluxo interno"). Design aprovado: `docs/PORTA-UNICA-DESIGN.md`.

Âncoras mantidas (core/rt — `.claude` é scan-excluído, mas conhecido):
- `packages/core/src/domain/model/event.rs` — event model (novo `EVENT_PIPELINE_KIND` + payload)
- `apps/rt/src/commands/event/emit_pipeline.rs` — `KNOWN_KINDS`
- `apps/rt/src/hooks/task/skill_usage_observer.rs` — observer de `skill.invoked` (modelo do emit determinístico no caminho lean)
- `apps/rt/src/shared/events/route.rs` — roteamento/escrita do evento
- `.claude/CLAUDE.md` + `apps/cli/templates/CLAUDE.md` — § Intent Routing (o roteador)
- frontmatter de `feature`/`bugfix`/`task`/`tactical-fix` (neutralizar o auto-trigger)

Âncoras dropadas do digest: dashboard (`watcher.rs`, `TelemetryTimeRange`, `lib.rs`) — consumir o `kind` é item separado; e resíduos de `skill-creator` (removido nesta sessão; modelo de scan ainda stale).

## Usuários/Stakeholders

- **Usuário do CLI** — não escolhe mais comando; descreve e o mustard roteia.
- **Quem lê o dashboard** — passa a ver o trabalho separado por tipo + a narrativa do pedido.
- **IA orquestradora** — roteia com fonte única (Intent Routing), sem se confundir entre fluxos.

## Métrica de sucesso

- Um pedido em linguagem natural roteia pro fluxo certo **sem o usuário digitar comando**, com a leitura narrada e confirmação só na dúvida.
- **Todo** run (inclusive lean: task, bugfix rápido) emite `pipeline.kind`.
- As descrições de feature/bugfix/task/tactical-fix **não anunciam mais** escolha de usuário.

## Não-Objetivos

- **Não** renomeia comandos (seguem como override oculto, invocáveis).
- **Não** mexe no dashboard (consumir o `kind` é item separado).
- **Não** implementa `skill-fetch` nem mexe no `/skill`.
- **Não** remove os fluxos internos (feature/bugfix/task seguem inteiros).

## Critérios de Aceitação

- **AC-1** — Build verde
  Command: `cargo build`
- **AC-2** — Testes verdes (rt + core)
  Command: `cargo test`
- **AC-3** — Evento `pipeline.kind` reconhecido pelo emissor
  Command: `grep -q "pipeline.kind" apps/rt/src/commands/event/emit_pipeline.rs`
- **AC-4** — Constante do evento no event model
  Command: `grep -q "EVENT_PIPELINE_KIND" packages/core/src/domain/model/event.rs`
- **AC-5** — Caminho lean emite o tipo (teste determinístico)
  Command: `cargo test pipeline_kind`
- **AC-6** — Descrição do fluxo neutralizada (não anuncia escolha de usuário)
  Command: `grep -q "internal flow" apps/cli/templates/commands/mustard/feature/SKILL.md`

<!-- PLAN -->

## Arquivos

**Wave 1 — telemetria de tipo (#3, core+rt):** `packages/core/src/domain/model/event.rs`, `apps/rt/src/commands/event/emit_pipeline.rs`, `apps/rt/src/hooks/task/skill_usage_observer.rs`, `apps/rt/src/shared/events/route.rs` (+ teste do emit lean).

**Wave 2 — porta única (#2, `.claude` orquestrador):** `.claude/CLAUDE.md` + `apps/cli/templates/CLAUDE.md` (§ Intent Routing → roteador), frontmatter de `feature`/`bugfix`/`task`/`tactical-fix` SKILL.md (×2 trees), entrada de ajuda `/mustard`.

## Limites

IN: roteador em linguagem natural (`CLAUDE.md § Intent Routing`: classifica→narra→confirma→despacha→emite `kind`); neutralizar descrições dos 4 fluxos; ajuda `/mustard`; evento `pipeline.kind` (core+rt) com emissão determinística incluindo caminhos lean; doc em duas audiências.
OUT: dashboard (consumo do `kind`); rename de comandos; `skill-fetch`/`/skill`; remoção de fluxos internos.