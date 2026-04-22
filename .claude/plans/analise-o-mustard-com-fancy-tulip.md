# Consolidar e consertar `/mustard:stats` + `/mustard:metrics`

## Context

Você pediu melhoria em `/mustard:stats` e `/mustard:metrics` — "não está bom, tudo (correção + consolidação + cobertura + saída visual)". Ao rodar ambos em `C:\Atiz\Mustard` encontrei bugs concretos e split-brain de dados. Esta é a sequência mínima para deixar as duas views úteis sem inventar arquitetura nova.

## Estado atual (problemas verificados ao rodar)

1. **Split-brain de dados.** Três diretórios, dois comandos, nenhuma view unificada:
   - `.claude/.pipeline-states/{spec}.json` — state principal (lido)
   - `.claude/.pipeline-states/{spec}.metrics.json` — sidecar do hook `metrics-tracker.js` com `apiCalls`, `toolBreakdown`, `retries`, `gate_saves`, `wave_reentry`, `skillHits`, `agentAttempts` (**escrito e ignorado por `/stats`**)
   - `.claude/metrics/{spec}.json` — archive final escrito por `/complete` (lido)
   - `.claude/.metrics/*.jsonl` — eventos de hook (lido só por `/metrics`)

2. **`/stats` diz "No metrics data found" quando há dado.** `metrics-collect.js:36` filtra `!f.endsWith('.metrics.json')` — então **nunca lê o sidecar**. Se o projeto ainda não rodou `/complete` (sem `.claude/metrics/`), `/stats` fica cego apesar do sidecar ter dados úteis.

3. **`/metrics` tem coluna morta.** `metrics-report.js:242-244` explicitamente descarta `tokens_saved` de `rtk-rewrite` → a célula "Estimated tokens saved" em "RTK Hook Activity" sempre vale `-`.

4. **`/metrics` imprime header sem body.** `metrics-report.js:303` checa `rtkData.total_saved` mas `rtk gain --format json` devolve `saved_tokens`/`savings_pct` (como `metrics-collect.js:204` já usa corretamente). Header `## RTK Token Savings` aparece vazio.

5. **Lógica RTK duplicada e divergente.** Os dois scripts chamam `rtk gain --all --format json`, com parse de campos inconsistente.

6. **Nenhuma view unificada.** Usuário precisa rodar 2 comandos e ainda assim não vê sidecar ativo nem correlação entre hooks (jsonl) e pipelines.

## Target state

**Um único script canônico (`metrics-collect.js`)** que agrega as 3 fontes e emite markdown bem seccionado. `metrics-report.js` vira **helper focado só em eventos de hook** (o que já faz bem) e é invocado internamente pelo collect quando há `.claude/.metrics/`. `/mustard:stats` e `/mustard:metrics` continuam existindo, mas:

- `/mustard:stats` — view completa (pipelines ativos + sidecar + completed + hooks + RTK + Pass@1 + highlights)
- `/mustard:metrics` — view focada em eventos de hook (passa `--hooks-only` para o collect, ou permanece chamando `metrics-report.js` sem mudança). Mantém `--since`, `--event`, `--compare`.

### Dimensões de medição (decidido)

1. **Por spec** — dimensão primária. Já nativa no sidecar.
2. **Por dia/semana** — dimensão secundária. Agregação dos eventos `.jsonl` por `ts`. **Sem** instrumentação nova.
3. ~~Por sessão Claude~~ — **fora de escopo**. Exigiria gravar `session_id` em `_metrics-write.js` + em cada hook chamador, e dados antigos ficariam nulos. Não vale a troca neste ciclo.

Sem criar comando novo. Sem hook novo. Subtração: remove lógica RTK duplicada.

## Mudanças sequenciadas (1 → 2 → 3 → 4)

### 1. Correção — bugs mensuráveis (prioridade máxima)

Arquivos: `templates/scripts/metrics-collect.js`, `templates/scripts/metrics-report.js`.

- **Ler o sidecar.** Em `metrics-collect.js`, para cada `{spec}.json` em `.pipeline-states/`, tentar ler o sidecar `{spec}.metrics.json` correspondente. Se existir, usar `sidecar.metrics` (que já tem o schema esperado no loop atual). Remover dependência de `state.metrics` no state principal.
- **Consertar RTK section em `/metrics`.** Em `metrics-report.js:292-313`, trocar leitura de `total_saved`/`total_original` para `saved_tokens`/`savings_pct`/`total_commands`. Se os campos não vierem, **não imprimir o header** (remover body órfão).
- **Remover coluna morta.** Em `metrics-report.js:337-346`, remover a linha "Estimated tokens saved" do bloco "RTK Hook Activity" (sempre `-` por design do PR1). Deixar só `Commands rewritten by hook`.
- **Dedupe RTK.** Extrair chamada a `rtk gain --all --format json` para helper compartilhado (`_rtk-gain.js` em `templates/scripts/`) que retorna `{ saved, pct, byCommand }` normalizado. Os dois scripts consomem o helper — single source of truth.

Critério: rodar `node metrics-collect.js` e `node metrics-report.js` no Mustard. `/stats` deve listar os 19 sidecars em `.pipeline-states/` (em seção "Active" ou "Orphaned"). `/metrics` não deve mais ter célula `-` morta nem header vazio.

### 2. Consolidação — uma fonte

Arquivo: `templates/commands/mustard/stats/SKILL.md`, `templates/commands/mustard/metrics/SKILL.md`, `metrics-collect.js`.

- `metrics-collect.js` ganha seção "## Enforcement Events (hooks)" chamando o novo helper (ou lendo `.claude/.metrics/*.jsonl` direto — a lógica de `metrics-report.js` default-mode é ~30 linhas reaproveitáveis). Flag opcional `--hooks-only` pula as seções pipeline-grained.
- `metrics-report.js` passa a ser **o especialista em `--compare`** (retém toda a lógica de compare mode, que é não-trivial). Default mode fica redundante mas mantido para compatibilidade.
- Skill `stats/SKILL.md`: atualizar descrição para "superset view" e listar as 5 seções que `/stats` passa a emitir.
- Skill `metrics/SKILL.md`: atualizar para "hook-only events and compare mode" + cross-reference para `/stats`.

Critério: um único comando (`/stats`) dá a foto completa. `/metrics` continua útil para `--compare` e `--event`.

### 3. Cobertura — aproveitar o que o sidecar e `.jsonl` já têm

**Dimensão "por spec"** — sidecar já coleta mas `/stats` nunca mostrou: `agentAttempts` (retries por fase), `gate_saves`, `wave_reentry`, `skillHits`. Dois dos quatro já estavam no código de collect mas dependiam do path errado.

Adicionar ao markdown emitido (spec-level):

- **Top tools consumers.** Ordenar `toolBreakdown` e destacar top 3 por spec.
- **Retries por fase.** Ler `agentAttempts` do sidecar. Se alguma fase teve ≥3 retries, marcar com `⚠` na saída.
- **Pass@1 por agente (heurístico).** Cruzar `.claude/.subagent-registry.json` (já existe, usado pelo metrics-tracker) com `agentAttempts`. Se nenhum agente teve retry, agente tem Pass@1 = 100%. É advisory.

**Dimensão "por dia/semana"** — novo bloco em `metrics-collect.js`:

- Ler eventos `.claude/.metrics/*.jsonl`, agrupar por dia (`ts.slice(0, 10)`).
- Emitir seção `## Últimos 7 dias` com contagem de eventos por tipo, tokens afetados totais, top dia. Se `--since` for passado, ajustar a janela.
- Semana corrente vs semana anterior: se há dados suficientes, mostrar delta (`ref→new, Δ%`). Reusa a função `cell()` de `metrics-report.js:132`.

**Sem** inventar novos campos no sidecar nem no `.jsonl`. Tudo sai do que já é gravado.

Critério: após #1+#2, `/stats` mostra no mínimo: pipeline por nome, duração, API calls, top 3 tools, gate saves, wave reentries, skill hit rate, retries por fase, **e** um bloco temporal "Últimos 7 dias" com totais e delta semana-a-semana.

### 4. Saída visual — sumário primeiro, drill-down depois

`metrics-collect.js`:

- Abrir com bloco "## Summary" de 5–8 linhas: nº de pipelines ativos, nº orfãos, Pass@1 agregado, RTK savings totais, top alerta (ex: "⚠ 1 pipeline com 5+ retries na fase EXECUTE").
- Seções detalhadas viram drill-down (já estão, só reordenar).
- Usar emoji discreto como prefixo (✓ / ⚠ / →) **somente em linhas de summary**; o resto continua markdown puro. Nada de cor ANSI (output vai pro chat).

Critério: ao rodar `/stats` o usuário entende em ≤10s o estado geral sem rolar.

## Arquivos tocados (total)

| Arquivo | Mudança |
|---------|---------|
| `templates/scripts/metrics-collect.js` | Ler sidecar, adicionar summary, adicionar seções hook/cobertura, usar helper RTK |
| `templates/scripts/metrics-report.js` | Fix campos RTK, remover coluna morta, usar helper RTK |
| `templates/scripts/_rtk-gain.js` | **Novo** — helper compartilhado (~30 linhas). Único arquivo novo. |
| `templates/commands/mustard/stats/SKILL.md` | Atualizar descrição (superset) |
| `templates/commands/mustard/metrics/SKILL.md` | Atualizar descrição (focus em hooks + compare) |
| `templates/hooks/__tests__/hooks.test.js` | Adicionar teste: sidecar é lido por metrics-collect |

Zero mudanças em hooks, zero mudanças em pipeline-states, zero mudanças em `/feature`/`/approve`/`/resume`/`/complete`. Strictly additive no helper RTK, subtrativo na duplicação.

## Fora de escopo

- Reescrever o schema do sidecar.
- Criar UI/dashboard.
- Migrar para SQLite ou outro store.
- Mudar `metrics-tracker.js` (ele já coleta o que precisa).
- Novo hook, nova skill, novo comando.

## Verificação

Antes:
```
$ node metrics-collect.js
# Pipeline Metrics
No metrics data found. Run a pipeline first.
```

Depois (em `C:\Atiz\Mustard`):
```
$ node metrics-collect.js
# Pipeline Metrics

## Summary
- 19 pipelines tracked (sidecar) · 0 archived
- Pass@1 (hook-level): 63% (12/19 without hook retries)
- RTK savings: ~463k tokens (86%)
- ⚠ 1 pipeline with 5+ retries in EXECUTE

## By Spec (Active / Orphaned): ...
## Last 7 Days: events by day, totals, delta vs prior week
## Enforcement Events (hooks): ...
## RTK Token Economy: ...
```

Testes:
- `node --test templates/hooks/__tests__/hooks.test.js` continua passando.
- Novo teste: criar sidecar fake em tmp `.pipeline-states/foo.metrics.json` e assertar que `metrics-collect.js` lista "foo" em Active.
- Rodar `node metrics-report.js --compare v3.1.21 v3.1.30` e confirmar que compare mode continua intacto.

## Sequência de execução

Delegar cada etapa em Task separada (per L0 rule):

1. `Task(general-purpose)` — Etapa #1 (correção): 3 arquivos, testes antes/depois. Aprox. 40 linhas mudadas.
2. `Task(general-purpose)` — Etapa #2 (consolidação): 3 arquivos (collect + 2 skill.md). Aprox. 30 linhas.
3. `Task(general-purpose)` — Etapa #3 (cobertura): só `metrics-collect.js`. Aprox. 40 linhas.
4. `Task(general-purpose)` — Etapa #4 (visual): só `metrics-collect.js`. Aprox. 20 linhas.

Cada etapa deve rodar `node metrics-collect.js` no próprio Mustard como smoke test antes de devolver.
