# Retorno de campo — `/feature` na sialia (espelhar `/payables` ~ `/receivables`)

> **Status: REGISTRO permanente de retorno de campo (2026-06-01).** Origem: rodar
> `/mustard:feature` num projeto real (`c:\atiz\sialia`), não no repo do mustard.
> Prompt: *"criar no contas a pagar (`/payables`) uma visão semelhante ao contas a
> receber (`/receivables`); vamos analisar"*. A sessão saiu de **~39k → ~114k tokens**.
> Memória relacionada: [[project-mustard-token-economics]].

## Veredito honesto da sessão

Avaliação franca do que o framework de fato entregou nesta tarefa **Light pequena**
(espelhar uma página/slice já existente). Separado em valor real, neutro e dívida.

### O que ajudou de verdade

- **`mustard-rt run feature` (o digest).** Numa chamada devolveu termos casados
  (`payables` 143, `receivables` 143), as fatias recorrentes e — o que mais importou —
  **12 âncoras apontando direto para os arquivos certos**. Sem isso, várias rodadas de
  Grep/Glob tateando estrutura. Atalho de navegação real.

### Neutro

- **`/scan` (gerar `grain.model.json`).** Pré-requisito do digest, mas o valor colhido
  veio do digest, não de ler o modelo (que, por regra, nem se lê).

### Dívida identificada (o que NÃO ajudou na época)

- **`spec-draft` criava esqueleto vazio.** `spec.md` + `meta.json` + `_index.md` só com
  título e slug. Toda a inteligência da spec (descobrir que falta a camada de listagem,
  que o backend já expõe `agingPayables`, que `totalPaidInPeriod` não existe — o único
  furo de design real) veio de **ler os arquivos**, não da ferramenta.
- **Slug horrível e truncado:** `espelhar-em-contas-a-pagar-a-visao-de-listagem-de-contas-a-r`.
  Mantinha stopwords (`em`, `a`, `de`), mutilava acento (`visão` → ia virar lixo) e
  **cortava no meio da última palavra** (`receber` → `r`) por um `.take(60)` cego em chars.

### Economia de token — saldo líquido provavelmente NEGATIVO nesta sessão

- O `rtk` cortou tokens nos comandos de shell (scan, feature, help) — economia real, porém
  pequena em volume absoluto.
- Contra isso pesa muito mais a **reinjeção de `<system-reminder>`** a cada turno: lista
  inteira de skills (duas vezes), `CLAUDE.md` por subprojeto, catálogo de agentes. Esse
  overhead de contexto do harness engole, com folga, o que o `rtk` poupou nos comandos.
- A regra *"leia só as âncoras, no máximo +5"* serve como disciplina de custo, mas a análise
  honesta exigiu comparar `receivables` vs `payables` lado a lado (~8 arquivos). Ajuda a
  pensar em custo; não é economia mágica.

**Tese de token correta:** o ganho do mustard é **amortização** — contexto upfront compra
menos exploração. Só fecha no positivo quando a exploração economizada é grande (feature
**multi-onda**). Numa **Light pequena, o upfront domina** e o saldo é negativo. O ganho
concreto aqui não foi token; foi **estrutura e rastreabilidade** (fases, spec versionada,
gates de QA).

## Ações tomadas (correções já aplicadas e verificadas)

Resposta direta à dívida acima. `cargo test -p mustard-rt -p mustard-core -- slug context_block`
→ **40 passed, 0 failed** (2026-06-01).

1. **Slug corrigido.** `apps/rt/src/commands/spec/spec_draft.rs::slug_from_intent` deixou de
   usar o char-map à mão e passou a **delegar a `mustard_core::slugify`** (dobra de acento +
   drop de stopwords por locale), com corte em **fronteira de palavra** via
   `SLUG_MAX_TOKENS = 8` (nunca mais mid-word). Teste `slug_drops_stopwords_no_midword_cut`
   fixa o caso exato da sialia:
   `"Espelhar em contas a pagar a visão de listagem de contas a receber"`
   → `espelhar-contas-pagar-visao-listagem-contas-receber`.
2. **Scaffold deixou de nascer vazio.** `spec_draft::context_enrichment` + `render_context_block`
   injetam as **âncoras e fatias recorrentes do digest** dentro da seção Context do draft
   (chaves i18n `context.scan_anchors` / `context.scan_slices`). O valor do digest, antes
   perdido, agora fica baked na spec. Teste `render_context_block_lists_anchors_and_slices`.
3. **Economia de token calibrada** na memória `project-mustard-token-economics` (não prometer
   economia em Light pequena; o ganho ali é estrutura/rastreabilidade).

## Lever ainda em aberto (decisão de design, não bug)

O maior custo da sessão — **reinjeção de contexto do harness** — é majoritariamente
comportamento base do Claude Code (system-reminders), **mas é amplificado pelo mustard**:
instalar ~15 skills + um `CLAUDE.md` por subprojeto aumenta o que volta a cada turno. O
único lever de peso no nosso controle é a **pegada do framework** (quantidade de skills
instaladas / peso dos guards `CLAUDE.md` por subprojeto). Isso muda UX e não é correção
óbvia — fica como decisão a tomar com o mantenedor, não mudança unilateral.

Lever menor, já regra inviolável no `SKILL.md` do `/feature`: **não re-ler scaffold/spec
recém-escrito** (round-trip puro de tokens).
