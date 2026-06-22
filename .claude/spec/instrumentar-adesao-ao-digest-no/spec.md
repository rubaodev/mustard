# instrumentar adesao ao digest no analyze do pipeline

<!-- drafter:tone=didactic — Write this spec narrative in didactic tone — expand abbreviations on first use (AC = Acceptance Criteria, wave = onda) and prefer plain words over jargon. -->

<!-- PRD -->

## Contexto

instrumentar adesao ao digest no analyze do pipeline.

Âncoras (do scan):
- apps/dashboard/src/features/knowledge/KnowledgeCard/index.tsx
- apps/dashboard/src/features/telemetry/PipelineTimeline/index.tsx
- apps/dashboard/src/features/workspace/LivePipelineCard/index.tsx

Fatias recorrentes (precedente a espelhar): args (×2), dir+file (×2)

Hoje afirmamos que o digest do Mustard (`mustard-rt run feature`) "economiza tokens" sem dado nenhum que prove adesão. O digest existe para substituir a leitura manual de código na fase ANALYZE — mas não sabemos se, numa sessão real, ele de fato rodou antes de o agente sair lendo arquivos-fonte na mão. Esta feature instrumenta essa adesão com dois sinais determinísticos por spec: se o digest rodou na ANALYZE que pariu o spec (`digestUsed`) e quantas leituras de fonte (Read/Grep/Glob) aconteceram ANTES do digest (`sourceReadsBeforeDigest`).

A sacada de implementação é não inventar telemetria nova: o observer de métricas já emite um evento `tool.use` por chamada de ferramenta, então só precisamos de um marco (`analyze.digest.used`, emitido por `feature.rs`) e de um comando novo (`digest-adherence-finalize`) que dobra os eventos da sessão num resumo escopado ao spec (`analyze.digest.summary`) — o único formato que o dashboard sabe ler, já que ele só varre `.claude/spec/{name}/.events/` e nunca os eventos crus da sessão.

## Usuários/Stakeholders

Mantenedores do Mustard (querem provar a economia com número, não com narrativa); a pessoa desenvolvedora que roda `/feature` (vê no dashboard se o fluxo do digest foi respeitado); quem decide roadmap (usa o sinal de adesão para priorizar onde o digest está sendo furado).

## Métrica de sucesso

Após uma sessão `/feature` Full real, o card do spec no dashboard mostra `digestUsed=true` quando `mustard-rt run feature` rodou na ANALYZE, e `sourceReadsBeforeDigest` reflete a contagem de leituras de fonte anteriores ao digest (zero no caminho ideal). O sinal é determinístico, reproduzível a partir do NDJSON e custa zero token de modelo.

## Não-Objetivos

- **Custo de token da reinjeção de memória** — esse dado vive no transcript do harness (`message.usage`), não no NDJSON do Mustard; fica explicitamente fora de escopo (limite conhecido).
- **Bugfix Fast Path** — não cria spec, logo não há onde ancorar o resumo escopado.
- **Qualquer gate/bloqueio baseado em adesão** — isto é só telemetria, nunca veredito.
- Atribuir leituras de fonte a uma fase específica além da janela ANALYZE da sessão.

## Critérios de Aceitação

- **AC-1** — `is_source_file` classifica fonte vs config/doc/lock
  Command: `cargo test -p mustard-rt source_class`
- **AC-2** — `feature` emite `analyze.digest.used` no sucesso do digest (sessão-escopado)
  Command: `cargo test -p mustard-rt feature::tests`
- **AC-3** — `digest-adherence-finalize` dobra a janela da sessão e emite `analyze.digest.summary` escopado ao spec
  Command: `cargo test -p mustard-rt digest_adherence`
- **AC-4** — `classify_kind` agrupa `analyze.*` sob `"analyze"`
  Command: `cargo test -p mustard-rt classify`
- **AC-5** — dashboard projeta `digest_used` + `source_reads_before_digest` no SpecCard a partir do resumo
  Command: `cargo test --manifest-path apps/dashboard/src-tauri/Cargo.toml spec_card`
- **AC-6** — paridade de tipos TypeScript (TS) + i18n compila
  Command: `pnpm --dir apps/dashboard typecheck`

<!-- PLAN -->

## Arquivos

**Wave (onda) 1 — rt backend (emit + finalize + classifier + classify_kind + observer + prosa SKILL):**
- `apps/rt/src/util/source_class.rs` (novo) — helper `is_source_file(path)->bool`.
- `apps/rt/src/util/mod.rs` — declarar `pub mod source_class;`.
- `apps/rt/src/commands/feature.rs` — emitir `analyze.digest.used` quando `digest_query` retorna `Ok`.
- `apps/rt/src/commands/agent/digest_adherence_finalize.rs` (novo) — comando que dobra a janela da sessão e emite `analyze.digest.summary`.
- `apps/rt/src/commands/agent/mod.rs` — declarar o módulo novo.
- `apps/rt/src/commands/mod.rs` — variante `DigestAdherenceFinalize` no enum `RunCmd` + braço no `dispatch()`.
- `apps/rt/src/shared/events/route.rs` — braço `analyze.*` → `"analyze"` em `classify_kind`.
- `apps/rt/src/hooks/task/metrics_observer.rs` — gravar `target.path` em Grep/Glob (destrava a classificação fiel).
- `apps/cli/templates/commands/mustard/feature/SKILL.md` — chamar `digest-adherence-finalize --spec <slug>` após o passo 2 do PLAN.
- `apps/cli/templates/commands/mustard/bugfix/SKILL.md` — chamar `digest-adherence-finalize --spec <slug>` no Full Path; Fast Path fora.

**Wave (onda) 2 — dashboard surfacing (depende da onda 1):**
- `apps/dashboard/src-tauri/src/spec_views.rs` — campos `digest_used`/`source_reads_before_digest` no `SpecCard` + fold do `analyze.digest.summary` + braços no `feed_payload_summary`.
- `apps/dashboard/src/lib/types/specs.ts` — espelhar os dois campos.
- `apps/dashboard/src/lib/phaseTheme.ts` — entradas `EVENT_THEME` para `analyze.digest.*`.
- `apps/dashboard/src/lib/i18n.ts` — chaves i18n (pt+en) + rótulos do badge.
- Componente consumidor do `SpecCard` (localizar via grep) — badge de adesão.

## Limites

IN: emissão `analyze.digest.used` (feature.rs); comando `digest-adherence-finalize`; `is_source_file`; `classify_kind` para `analyze.*`; extensão do `metrics_observer` (path de Grep/Glob); prosa SKILL `/feature` (sempre) e `/bugfix` Full Path; projeção SpecCard + paridade TS + tema + i18n + badge.
OUT: custo de token da reinjeção de memória; bugfix Fast Path; qualquer gate baseado em adesão; SQLite ou sink fora do NDJSON por-spec.

## Concerns

- **WARNs do `analyze-validation` são falso-positivos** (não-bloqueantes): o validador interpretou nomes de evento inline (`analyze.digest.used`, `analyze.digest.summary`, `target.path`) como caminhos de arquivo, e não reconhece o marcador PT `(novo)` como o `(create)` em inglês. Defeito menor da própria ferramenta, anotado fora desta spec.
- **Mudança de shape de payload existente (risco real):** estender o `metrics_observer` para gravar `target.path` em Grep/Glob altera o payload do `tool.use` já consumido. A onda 1 deve ser aditiva (campo novo, não renomear `target.file`/`target.pattern`) para não quebrar a atribuição de trace nem o `ToolEventRow` do dashboard.
- **Deploy:** as mudanças do rt só valem após **rebuild do binário** `mustard-rt`; a prosa SKILL vive em `apps/cli/templates/` e só chega às cópias `.claude/commands/` vivas via re-init. Ambos fora do EXECUTE — registrados para o CLOSE.
- **Escopo deliberado (opção B):** esta spec MEDE adesão; afrouxar a regra "não leia direto" (opção A) e forçar via hook (opção C) ficam para depois, condicionados ao que estes dados mostrarem.

<!-- signals: layers,files,2-subprojects,net-new-infra,no-entity-precedent -->