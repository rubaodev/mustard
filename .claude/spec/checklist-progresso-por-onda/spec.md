# Checklist de progresso por onda: estado no meta.json de cada onda + evento NDJSON + consolidação no close-gate

<!-- drafter:tone=didactic — Write this spec narrative in didactic tone — expand abbreviations on first use (AC = Acceptance Criteria, wave = onda) and prefer plain words over jargon. -->

<!-- PRD -->

## Contexto

Em specs decompostas em ondas (wave plan = plano dividido em ondas), hoje **nenhuma checklist de andamento é gerada** — nem no spec pai, nem nas ondas. Não há controle item-a-item do que já foi feito.

Por que isso acontece (verificado no código):
- O spec **pai** de um wave plan suprime a seção `## Checklist` de propósito — ele é documento de coordenação (`spec_scaffold.rs:73-107`). Isso está correto.
- Mas o gerador do esqueleto de onda (`wave_scaffold.rs::render_wave_spec`) só emite `## Summary` + `## Network` — **nunca semeia a checklist**. E a SKILL `pipeline-execution` só instrui checklist no escopo Light (leve).
- Resultado: zero itens `[ ]` no spec inteiro; o hook de auto-marcação fica sem alvo; e o `close-gate` (o portão que valida o fechamento), ao consolidar as ondas, não acha checklist em nenhuma e **passa "tendo checado nada"** — o gate órfão que o próprio comentário em `close_gate.rs:423` diz querer evitar.

**Decisão de design (já acordada com o dono).** O estado de checklist sai do markdown e passa a viver no **`meta.json` de cada onda** — não no meta do raiz (evita disputa de escrita entre ondas paralelas), nem como markdown-fonte (o dashboard está abandonando o parsing de markdown rumo a leitores tipados de NDJSON). Reusa-se o tipo já existente `ChecklistItem {label, path}` mais um campo `done`. A marcação passa a emitir um evento NDJSON `checklist.item.marked`; o `close-gate` consolida lendo os `meta.json` das ondas; e o dashboard renderiza o progresso **por onda** a partir dos eventos. A seção markdown `## Checklist` vira projeção somente-leitura opcional, fora do caminho crítico.

Âncoras reais (já lidas nesta investigação):
- `packages/core/src/domain/spec/contract.rs` — tipo `ChecklistItem` (fonte) + `render_checklist_item`.
- `packages/core/src/domain/meta/*` — tipo `Meta` (contrato serde público; ganha o campo `checklist`).
- `packages/core/src/domain/model/event/*` — enum de eventos (ganha `checklist.item.marked`).
- `apps/rt/src/commands/wave/wave_scaffold.rs` — semear o `checklist` no `meta.json` de cada onda.
- `apps/rt/src/commands/checklist/mark_checklist_item.rs` + hook `checklist-auto-mark` — escrever `done` no meta + emitir o evento.
- `apps/rt/src/hooks/write/close_gate.rs:424` — `find_unmarked_checklist` consolida via meta das ondas.
- `apps/rt/src/commands/spec/spec_validate.rs` — validar checklist a partir do meta.
- `apps/dashboard/src-tauri/src/*` + `apps/dashboard/src/features/specs/SpecWavesTab`, `WaveRowLabel`, `WaveMarkdownDrawer` — progresso por onda dos eventos.
- `apps/cli/templates/skills/pipeline-execution/SKILL.md` (+ cópia em `.claude/skills/`) — instruir o PLAN de onda.

## Usuários/Stakeholders

- **Dono/operador do mustard:** quer abrir uma spec e ver o andamento item-a-item de cada onda — hoje não consegue.
- **O close-gate:** precisa de itens reais para consolidar; senão libera o CLOSE sem ter checado nada.
- **O dashboard:** precisa de um sinal de progresso estruturado por onda (a view `criteria` está órfã).

## Métrica de sucesso

- Toda onda de um wave plan nasce com uma checklist trackável no seu `meta.json`.
- Cada arquivo tocado pelo agente marca seu item (`done=true`) e emite `checklist.item.marked`, com carimbo de tempo.
- O CLOSE é bloqueado enquanto qualquer item de qualquer onda estiver `done=false`.
- O dashboard mostra "N/M itens" por onda, derivado dos eventos.

## Não-Objetivos

- **Mover a checklist para o `meta.json` do raiz** — descartado por disputa de escrita entre ondas e por forçar o dashboard a desagregar.
- **Manter o markdown `## Checklist` como fonte de verdade** — ele vira, no máximo, projeção somente-leitura opcional (não nesta entrega).
- **Reescrever o sistema de eventos/telemetria** — só se adiciona o evento `checklist.item.marked`; o resto do NDJSON fica como está.
- **Migrar specs antigas já arquivadas** — o comportamento vale para specs novas; specs já fechadas não são reprocessadas.

## Critérios de Aceitação

- **AC-1** — `wave-scaffold` grava um `checklist` estruturado (array de `{label, path, done:false}`) no `meta.json` de cada onda; o pai (wave plan) segue sem checklist. Testes verdes.
  Command: `cd apps/rt && cargo test wave_scaffold`
- **AC-2** — O tipo `Meta` (core) ganha o campo `checklist` sem quebrar o shape serde existente (round-trip de um meta antigo, sem o campo, continua válido). Testes verdes.
  Command: `cargo test -p mustard-core meta`
- **AC-3** — O auto-mark e o `mark-checklist-item` setam `done=true` no `meta.json` da onda e emitem o evento `checklist.item.marked` no NDJSON. Testes verdes.
  Command: `cd apps/rt && cargo test checklist`
- **AC-4** — O `close-gate` bloqueia o CLOSE quando um item de qualquer onda está `done=false` e libera quando todos estão `done=true` (consolidação lendo os metas das ondas). Testes verdes.
  Command: `cd apps/rt && cargo test close_gate`
- **AC-5** — O dashboard mostra o progresso por onda ("N/M itens") derivado dos eventos NDJSON; build verde.
  Command: `pnpm -C apps/dashboard build`
- **AC-6** — Suíte do workspace verde (core + rt) e a crate do dashboard (fora do workspace) verde.
  Command: `cargo test --workspace && cd apps/dashboard/src-tauri && cargo test`

<!-- PLAN -->

## Arquivos

Visão por onda (a decomposição detalhada — tarefas e checklist — vive em cada `wave-N/spec.md`):

**Onda 1 — Core (`packages/core`): contrato.** Base de tudo.
- `src/domain/spec/contract.rs` — `ChecklistItem` ganha `done: bool` (default false); ajustar `render_checklist_item` e a regra `ChecklistEmpty` do validador.
- `src/domain/meta/*` — `Meta` ganha `checklist: Vec<ChecklistItem>` (serde tolerante a metas sem o campo).
- `src/domain/model/event/*` — registrar o evento `checklist.item.marked` (`{spec, wave, item, path}`).

**Onda 2 — Rt (`apps/rt`): geração, marcação e gate.** Depende da Onda 1.
- `src/commands/wave/wave_scaffold.rs` — semear o `checklist` no `meta.json` de cada onda a partir dos arquivos-alvo do plano.
- `src/commands/checklist/mark_checklist_item.rs` + hook `checklist-auto-mark` — marcar `done` no meta + emitir o evento.
- `src/hooks/write/close_gate.rs` — `find_unmarked_checklist` consolida via meta das ondas (não markdown).
- `src/commands/spec/spec_validate.rs` — validar a checklist a partir do meta.

**Onda 3 — Dashboard (`apps/dashboard`): progresso por onda.** Depende das Ondas 1-2.
- `src-tauri/src/*` — agregador NDJSON que projeta `checklist.item.marked` em progresso por onda.
- `src/features/specs/SpecWavesTab`, `src/components/page/WaveRowLabel`, `src/features/specs/WaveMarkdownDrawer` — render "N/M itens" por onda; ocupar a view `criteria` órfã.

**Onda 4 — Templates/SKILL (`apps/cli/templates` + `.claude/skills`).** Depende da Onda 2.
- `templates/skills/pipeline-execution/SKILL.md` (+ cópia em `.claude/skills/`) — instruir o PLAN de onda a popular a checklist; remover os cabeçalhos de lifecycle legados que a SKILL ainda menciona no markdown.

## Limites

**IN:** estado de checklist no `meta.json` por onda; campo `done` em `ChecklistItem`; evento `checklist.item.marked`; auto-mark + `mark-checklist-item` gravando no meta; consolidação do close-gate via meta; render de progresso por onda no dashboard; atualização da SKILL pipeline-execution.

**OUT:** checklist no meta do raiz; markdown como fonte de verdade; reescrita do sistema de eventos; migração de specs já arquivadas; projeção markdown somente-leitura (fica para um ciclo posterior, opcional).

## Decisões

- **Onde mora o estado:** `meta.json` de **cada onda** (não o raiz). Razão: isolamento de escrita entre ondas paralelas + o dashboard consome estado por onda.
- **Fonte de verdade:** o evento NDJSON `checklist.item.marked` é a trilha durável; o `done` no `meta.json` é a projeção materializada para leitura rápida do gate (padrão `core-projection-pattern`).
- **Markdown:** rebaixado a projeção somente-leitura opcional — fora desta entrega.
- **Compatibilidade:** `Meta.checklist` é serde-tolerante (metas antigos sem o campo continuam válidos) — sem migração destrutiva.

## Concerns

- `analyze-validation` (WARN, não-bloqueante) apontou 3 "arquivos não encontrados" — todos **falsos positivos**: `wave-N/spec.md` é referência genérica ao padrão de onda (não um arquivo concreto), e `checklist.item.marked` (×2) é o **nome do evento NDJSON**, não um arquivo. Nenhum exige criação literal desses caminhos.
- **Contrato serde público (`Meta`):** a Onda 1 muda um tipo que rt e dashboard renderizam por cima. A adição de `checklist` deve ser aditiva e `#[serde(default)]` — a REVIEW deve confirmar que nenhum consumidor existente quebra (round-trip de meta antigo).
- **Crate do dashboard fora do workspace:** `apps/dashboard/src-tauri` não é coberta por `cargo test --workspace`; a Onda 3 deve verificar **de dentro** de `src-tauri` (`cargo check`/`cargo test`) + `pnpm build`.