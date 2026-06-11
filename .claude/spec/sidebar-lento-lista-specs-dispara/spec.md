# Sidebar lento: lista de specs dispara N spec-cards que refazem o fold de contagens N vezes; batch unico com counts compartilhados

<!-- drafter:tone=didactic — Write this spec narrative in didactic tone — expand abbreviations on first use (AC = Acceptance Criteria, wave = onda) and prefer plain words over jargon. -->

<!-- PRD -->

## Contexto

Sidebar lento: lista de specs dispara N spec-cards que refazem o fold de contagens N vezes; batch unico com counts compartilhados.

Âncoras (do scan — BAIXA CONFIANÇA: casamento fraco, confirme lendo antes de usar):
- apps/dashboard/src/components/layout/Sidebar/index.tsx
- apps/rt/src/commands/wave/epic_fold.rs
- apps/rt/src/commands/wave/wave_files.rs
- apps/dashboard/src/features/workspace/WorkspaceSpecsByStatus/index.tsx
- apps/scan/tests/spec_like.rs
- apps/rt/src/hooks/task/main_context_counter.rs
- packages/core/src/view/projection/card.rs
- packages/core/src/view/projection/workspace.rs
- apps/dashboard/src/pages/Specs.tsx
- apps/dashboard/src/hooks/useSpecActions.ts
- apps/rt/src/commands/spec/active_specs.rs
- apps/dashboard/src/features/specs/SpecsList/index.tsx

Fatias recorrentes (precedente a espelhar): Action+Card+Children+Events+Quality+Timeline+Waves (×2), Files+Planned (×2)

Mesmo após a spec `performance-dashboard-rotas-lentas-cache` (cache incremental de eventos + push), trocar de rota pelo sidebar continua lento — pior na rota Specs. O cache eliminou a releitura de disco, mas não a repetição do *fold* (a agregação das contagens por spec sobre todos os eventos em memória).

## Causa raiz

A página Specs (`apps/dashboard/src/pages/Specs.tsx:367-386`) dispara, além da lista, **uma consulta `spec-card` por spec listada** (N em paralelo). Cada `dashboard_spec_card` chama `spec_card_v2` → `attributed_spec_counts(&repo)` (`apps/dashboard/src-tauri/src/lib.rs:352`), que refaz a agregação inteira do workspace **N vezes** (~150 ms cada com cache quente; segundos no total). O padrão correto já existe no próprio arquivo: `dashboard_active_pipelines_impl` (`lib.rs:1925-1931`) computa `attributed_spec_counts` **uma vez** e injeta em cada card via `spec_card_v2_with_counts`. A rota da lista não passa por ele.

Secundário: a página Economia monta 5 consultas com `staleTime` de 15 s + `refetchInterval` 30 s — toda troca de rota após 15 s refaz as 5; harmonizar com o default global (60 s).

## Plano

1. **Backend (batch)**: novo comando `dashboard_spec_cards` em `lib.rs` (em `spawn_blocking`, como os demais): resolve a lista cacheada de specs, computa `attributed_spec_counts` UMA vez e devolve `Vec<SpecCard>` via `spec_card_v2_with_counts` — espelho exato do miolo de `dashboard_active_pipelines_impl`, sem duplicar o fold (extrair/reusar o trecho comum).
2. **Binding**: `fetchSpecCards(repoPath)` em `src/lib/dashboard.ts` (padrão dos wrappers existentes).
3. **Front**: `Specs.tsx` troca as N consultas `["spec-card", ...]` por UMA `["spec-cards", repoPath]` e distribui pelos cards; a rota de **detalhe** mantém seu `useSpecCard` individual (granular, já cacheado).
4. **Economia**: `staleTime` das 5 consultas alinhado ao default global (60 s), mantendo `refetchInterval` como fallback.
5. **Teste**: contador test-visible de invocações de `attributed_spec_counts` — o batch com N specs incrementa 1, não N.

## Critérios de Aceitação

- **AC-1** — Teste novo do batch passa: `dashboard_spec_cards` devolve os cards de todas as specs computando as contagens uma única vez (contador = 1 para N specs)
  Command: `cargo test --manifest-path apps/dashboard/src-tauri/Cargo.toml --lib spec_cards`
- **AC-2** — Suíte do dashboard segue verde
  Command: `cargo test --manifest-path apps/dashboard/src-tauri/Cargo.toml --lib`
- **AC-3** — Front-end compila e passa a checagem de tipos
  Command: `npm --prefix apps/dashboard run build`

<!-- PLAN -->

## Arquivos

- `apps/dashboard/src-tauri/src/lib.rs` — comando batch `dashboard_spec_cards` + registro no generate_handler + teste com contador
- `apps/dashboard/src-tauri/src/telemetry.rs` — contador test-visible de `attributed_spec_counts` (se necessário)
- `apps/dashboard/src/lib/dashboard.ts` — binding `fetchSpecCards`
- `apps/dashboard/src/pages/Specs.tsx` — 1 consulta batch no lugar de N spec-cards
- `apps/dashboard/src/pages/Economia.tsx` — staleTime 60 s nas 5 consultas (ou no hook correspondente)
- `apps/dashboard/src/hooks/useEconomySummary.ts` — cascata: a 5ª consulta de 15 s vive aqui
- `apps/dashboard/src/hooks/useSpecAction.ts` — cascata: invalidar `["spec-cards"]` após ações de ciclo de vida
- `apps/dashboard/src/hooks/useSpecActions.ts` — cascata: idem
- `apps/dashboard/src/lib/watcher.ts` — cascata (flag do review): push de snapshot também invalida `["spec-cards"]` para a lista seguir viva a eventos externos

## Limites

IN: rota Specs (lista) e ajuste de staleTime da Economia.
OUT: rota de detalhe de spec (já granular e cacheada); reescrever os comandos de economia sobre o cache de eventos (mudança maior, valor menor — fica para spec futura se a Economia seguir lenta); formato dos eventos; sidebar/roteador.

## Checklist

- [x] T1 — comando batch `dashboard_spec_cards` (counts 1×) + teste contador
- [x] T2 — binding `fetchSpecCards` em dashboard.ts
- [x] T3 — Specs.tsx consome o batch (N→1 consultas)
- [x] T4 — Economia: staleTime alinhado ao default 60 s