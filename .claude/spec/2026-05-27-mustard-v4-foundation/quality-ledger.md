# Quality Ledger — Spec A (mustard-v4-foundation)

### Inaugurado em: 2026-05-27 (W8 close)
### Propósito

Este é o **primeiro snapshot** do quality-ledger da Mustard v4. Métricas medidas
em runtime real desta spec viram **baseline** para qualquer spec futura medir
regressão de UX e performance. Princípio: tudo no Mustard precisa virar métrica
auditável ([[feedback_everything_measurable]]). Sem estimativa — número real.

## Métricas medidas

### M-Q1 — Tempo de bootstrap (`resume-bootstrap`)

Medido contra a própria Spec A em estado pré-close (currentWave=8, completedWaves
0..7, ~12 sub-specs + wave-plan + fixtures contando como waves para fins de
orçamento).

Comando: `mustard-rt run resume-bootstrap --spec 2026-05-27-mustard-v4-foundation --json`

| Métrica | Valor |
|---|---|
| min_ms | 613 |
| median_ms | 614 |
| max_ms | 29 556 (cold start — hook PostToolUse compilou dispatcher) |
| warm median (n=2) | 613.5 ms |
| tokensUsed reportado | 0 (campo serde em `resume_bootstrap::print_json`) |
| summariesLoaded reportado | 0 |
| contextPath gerado | `.claude/spec/2026-05-27-mustard-v4-foundation/wave-2-core/_context.md` |

**Interpretação:** warm-path bate < 1s, dentro do esperado para o budget AC-A-10
(<10k tokens). `tokensUsed` e `summariesLoaded` em 0 são esperados nesta close:
nenhum `_summary.md` foi gerado em runtime durante W0..W7 (W3 entregou apenas o
schema canônico de `wave_summary::build` / `write`; o consumer real do shim CLI
fica como W5/W7 followup já documentado). Próxima spec que entregar o consumer
real do `wave_summary::write` vai mover esses dois números para >0 — a régua
fica: **regressão tokensUsed >10 000 = bloquear close**.

### M-Q2 — Tamanho típico de `_summary.md`

Glob `**/wave-*-*/_summary.md` na Spec A: **0 arquivos**.

**Estado atual:** apenas os schemas existem (W3 commit `f39c410` em
`apps/rt/src/run/wave_summary.rs` e `apps/rt/src/run/wave_context.rs` —
funções `build` / `write` expostas como API pública do crate). Nenhum `_summary.md`
foi escrito em disco durante a execução das waves W0..W7 desta spec — o consumer
real (que receberia `WaveSummaryInput` populado de `spec.md` + diff + AC results
e chamaria `wave_summary::write`) está listado como follow-up oficial em
`spec.md#Followups` (item "CLI subcommands `wave-summary` e `wave-context`").

**Baseline para futuras specs:** quando o consumer entrar, baseline esperado é
**5–25 KB / wave**, com warn em 60 KB (similar ao critério size-gate aplicado
a specs). Spec B (briefing + AC tipado) é a candidata natural a estrear o
consumer real.

### M-Q3 — Taxa de falso-positivo do gate no review W7

Fonte: `.claude/spec/2026-05-27-mustard-v4-foundation/review-w7-report.md` (240
linhas, em warn-zone aceito).

| Métrica | Valor |
|---|---|
| Casos críticos previstos (W6 no-sqlite stub silencioso) | 9 funções esvaziadas |
| Disparos do gate na fixture | 8 (Moment 3) + sinais agregados Moment 1/2 |
| Falsos positivos (gate disparou em função não-W6) | **0** |
| Falsos negativos (W6 que escapou) | 1 (`rtk_summary` borderline, registrado como follow-up W7#3 — critério `body_emptied` ainda não migrou de `line_changes > N`) |
| Momentos disparados (de 4 críticos) | 3/4 verde em hosts sem grammar; 4/4 após W7#1 resolvido em 2026-05-27 |

**Régua para futuras specs:** falso-positivo do gate de regressão **≤5%** sobre
um corpus controlado de N≥20 fixtures. Hoje estamos em 0/9 = 0% sobre o corpus
W6. Crescer o corpus em Spec C / Spec D.

### M-Q4 — Auditoria de cobertura W8

| Item | Status |
|---|---|
| AC binários cobertos por teste literal | 15/17 (pass; 2 deferred = A-6, A-18) |
| ACs marcados `[x]` na spec.md | 5 (A-1, A-4, A-5, A-7, A-10) — cobertura conservadora; os outros 10 `pass¹` estão entregues mas não tinham `Command:` literal escrito |
| Waves verdes / total | 9 / 11 (sem contar W8.5 paralela) |
| Code crates impactados | `mustard-core` (5 módulos novos) + `mustard-rt` (3 módulos novos + 2 estendidos) |

## Princípios cravados nesta inauguração

1. **Number-driven, no-estimate** ([[feedback_everything_measurable]]) — nenhum
   item neste ledger é "deve dar X ms"; tudo veio de comando real medido.
2. **Baseline > target** — para uma fundação, valor inicial é o **piso real**, não
   um target aspiracional. Regressão futura mede contra este piso.
3. **0 falso-positivo é o teto, não o ponto de partida** — corpus W6 é pequeno
   (n=9) e perfeitamente labelado; números só significam algo com corpus ≥20
   crescido em Spec C/D.
4. **Métrica documentada ≠ métrica capturada em runtime** — itens onde o consumer
   real ainda não chamou o builder (M-Q2 `_summary.md` count = 0) ficam
   explicitamente registrados como "schema only, runtime emission pending".
   Nunca falsificar valor para parecer pronto ([[feedback_no_stub_fail_open]],
   [[feedback_refactor_no_stub_deferral]]).

## Próximas entradas esperadas

Cada spec daqui pra frente deve adicionar bloco `## Snapshot - {spec-id}` ao
final deste arquivo (estilo append-only ledger) reportando:

- min/median/max de `resume-bootstrap`
- contagem + tamanho médio de `_summary.md` gerados
- taxa de falso-positivo do gate de regressão sobre fixtures novas
- delta vs baseline acima

Regra: **regressão > 30% em qualquer métrica bloqueia o pipeline.status Completed**.

<!-- wikilinks-footer-start -->
- [feedback_everything_measurable](?) ⚠ não resolvido
- [feedback_no_stub_fail_open](?) ⚠ não resolvido
- [feedback_refactor_no_stub_deferral](?) ⚠ não resolvido
<!-- wikilinks-footer-end -->