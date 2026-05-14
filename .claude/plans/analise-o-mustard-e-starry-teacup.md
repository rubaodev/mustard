# Plano executável: Eval harness de tokens (Fase 1 isolada)

## Contexto

Após avaliação honesta das 5 ideias absorvíveis do [Caveman](https://github.com/JuliusBrussee/caveman), decisão final: **implementar apenas Fase 1 (Eval harness)**. Razão: das 5, só a 1 tem benefício inequívoco preservando qualidade; fases 2-5 trocam complexidade concreta por benefício incerto ou marginal. Com dados de `--compare` em mãos nas próximas semanas, decisão sobre fases 2-5 vira data-driven em vez de especulativa.

## Primícias atendidas

- **Qualidade de código**: código aditivo a script existente; zero impacto em hot path; testável isoladamente com fixtures; falha só afeta relatório, nunca pipeline.
- **Economia de tokens**: transforma otimização em dado objetivo. Regressões ficam visíveis antes de virarem dor.

## Entregáveis

### 1. `templates/scripts/metrics-report.js` — modo `--compare`

Adicionar flag `--compare <from> <to>`:
- `<from>`/`<to>` aceitam git tag (`v3.1.22`) ou data ISO (`2026-04-09`)
- Leitura via `.claude/.metrics/*.jsonl` existente (já populado pelos hooks)
- Agregação:
  - Tokens total (via campos existentes em eventos)
  - Por agent type (Explore / Plan / general-purpose)
  - Por fase de pipeline (ANALYZE / PLAN / EXECUTE / CLOSE)
  - Por hook (hit rate, via `event` field)
- Fallback: se histórico insuficiente (<5 pipelines na janela), imprime warning em stderr e retorna relatório parcial sem bloquear
- Output: tabela markdown padrão Mustard (tabelas > prose, conforme `scan-format.md §5`)

Preserva interface atual (`--since`, `--event`). `--compare` é adição.

### 2. `templates/commands/mustard/metrics/SKILL.md` — documentar flag

Acrescentar a `Optional flags`:
- `--compare <from> <to>` — delta entre duas janelas (tag ou ISO date)
- Exemplo: `/mustard:metrics --compare v3.1.21 v3.1.22`

Zero mudança em triggers ou action principal.

### 3. `templates/scripts/__tests__/metrics-report.test.js` — suite nova

Usa `bun test` (built-in, zero-deps). Cobre:
- Parse de `--compare tag tag` e `--compare iso iso`
- Mix: `--compare tag iso`
- Fallback em histórico esparso (fixture com 2 pipelines)
- Agregação correta por agent type e fase
- Não regride comportamento de `--since` nem `--event`

Fixture: `__tests__/fixtures/metrics-sample.jsonl` (sintético, ~20 eventos cobrindo 2 "versões").

## Arquivos tocados

| Arquivo | Tipo de mudança |
|---|---|
| `templates/scripts/metrics-report.js` | Extensão (novo modo `--compare`) |
| `templates/commands/mustard/metrics/SKILL.md` | Documentação (3-5 linhas) |
| `templates/scripts/__tests__/metrics-report.test.js` | Arquivo novo (teste) |
| `templates/scripts/__tests__/fixtures/metrics-sample.jsonl` | Arquivo novo (fixture) |

**Zero**: novo hook, novo comando, nova dependência externa, mudança em settings.json, mudança em SKILL.md de pipeline.

## Invariantes respeitados

1. Node built-ins apenas — `fs`, `path`, `node:test`, `node:assert`
2. CommonJS mantido
3. `<!-- mustard:generated -->` não se aplica (script não é conteúdo gerado)
4. Git flow em `dev_rubens`; commit atômico; `/mustard:git merge main` opcional ao final
5. Delegação L0: implementação via `Task(general-purpose)` do main para um implementador

## Verificação

1. `rtk bun test templates/scripts/__tests__/metrics-report.test.js` — passa
2. `rtk bun test templates/hooks/__tests__/hooks.test.js` — não regrediu
3. Rodar no repo atual:
   - `rtk node templates/scripts/metrics-report.js --compare v3.1.21 v3.1.22`
   - Validar que produz baseline utilizável
   - Se tags esparsas, fallback por data ativa
4. Rodar com histórico sintético sparse — warning em stderr, exit 0
5. `/mustard:metrics --compare` invocado pelo comando passa o arg através corretamente

## Estimativa

~3h de trabalho efetivo, 1 commit atômico.

## Risco residual

- Métricas históricas podem ser esparsas no repo atual → relatório mostra delta com baixa confiança. Mitigação: warning explícito + fallback para janela por data.
- Parsing de git tag vs ISO date é ambíguo se usuário passar algo tipo `2026-04-09`. Mitigação: regex de tag (`^v?\d+\.\d+\.\d+$`) primeiro, ISO como fallback; ambos erram → mensagem clara.

## O que foi adiado e por quê

| Fase | Motivo para adiar |
|---|---|
| 2. Reasoning/Output split | Superfície grande (6 SKILL.md), benefício é formalização do implícito, risco de contracts rígidos demais |
| 3. Verbosity modes | Flags opt-in viram letra morta; conflita com memória "subtrair > adicionar"; pt-BR não testado |
| 4. Anti-drift heurístico | Heurística por palavra em pt-BR é frágil; detecta sintoma, não causa; ruído > sinal |
| 5. `--compact` | Destrutivo; knowledge.json ainda dentro do cap; resolve problema que não existe |

**Reavaliar fases 2-5 em 2-4 semanas com dados de `--compare` em mãos**. Se os dados mostrarem onde tokens realmente sangram, a decisão vira data-driven.

## Convenção de commit

```
feat(metrics): add --compare mode to metrics-report

Compares token/retry deltas between two git tags or ISO dates.
Falls back to partial report when history is sparse.
Zero behavioral change to existing --since/--event flags.

Refs: .claude/plans/analise-o-mustard-e-starry-teacup.md
```
