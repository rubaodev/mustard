# /mustard:spec — Resume flow (continuar pipeline)

Loaded on demand pelo SKILL Step 5 quando `stage=Execute` (ou `Analyze`/`QaReview`/`ReviewPending`/`QaPending`/`Close`). Toda decisão de modo (`continued` vs `reanalyzed`), resolução de operational spec, detecção de stub, `needsDiff`/`needsContextSlice`, lookup de `waveModel`, parsing de `lastDispatchFailure`, **e a decisão pós-execute REVIEW/QA**, foi movida para `mustard-rt run resume-bootstrap --spec X --json`. A construção literal do prompt do agente foi movida para `mustard-rt run agent-prompt-render`. Este ref guarda só o que o binário não pode decidir sozinho.

## Stage values pós-execute (nunca freelance)

O binário pode retornar três `stage` extras quando todas as waves terminaram. O orquestrador NUNCA decide por conta — sempre dispara o que `nextAction` mandar:

| `stage` | `nextAction` | Companion field | O que fazer |
|---------|--------------|-----------------|-------------|
| `ReviewPending` | `dispatch-review` | `reviewRoles: [...]` | Dispatch um Task de REVIEW por role |
| `QaPending` | `run-qa` | `qaCommand: "..."` | Rode literalmente o comando |
| `Close` | `emit-complete` | — | Só agora `emit-pipeline --kind pipeline.complete` é permitido |

Quando `nextAction` é `null`, ainda há wave para rodar — siga o fluxo normal de wave-dispatch abaixo.

## Hard gate em `emit-pipeline --kind pipeline.complete`

A partir de 2026-05-25 o binário se recusa a emitir `pipeline.complete` sem um `qa.result` (overall=pass) na ndjson da spec — exit 2 + mensagem `BLOCKED: …`. O escape hatch `--allow-no-qa` existe só para o próprio `qa-run` e overrides explícitos do usuário. Não tente contornar.

## Step 12c — Wave Plan Scope (condicional, só se `isWavePlan === true`)

Quando o JSON do bootstrap indica wave plan, o orquestrador despacha só a **wave atual**, nunca a spec inteira:

1. A spec para esta invocação é `operationalSpecPath` retornado pelo bootstrap (já resolvido para `wave-{currentWave}-*/spec.md`).
2. **Entre waves** (post-dispatch da wave N):
   - Commit estilo `/mustard:git commit` com mensagem `feat(wave-{N}/{role}): {summary}`. Fallback: `git add {files} && git commit -m "..."`.
   - Emita wave completion: `mustard-rt run emit-pipeline --kind pipeline.wave.complete --spec {specName} --payload "{\"wave\":{N},\"duration_ms\":{elapsed}}"`. A projeção deriva `completedWaves` + `currentWave` desses eventos — sem JSON state file.
   - Rode `mustard-rt run wave-tree --spec-dir .claude/spec/{specName}` para mostrar progresso.
   - Cache o diff desta wave: `git diff HEAD~1 HEAD > .claude/.pipeline-states/{specName}.wave-{N-1}.diff.md`. O `agent-prompt-render` da próxima wave injeta esse arquivo; orquestrador não passa nada explicitamente.
3. Se `currentWave >= totalWaves` → **NÃO** emita `pipeline.complete`. Re-rode `resume-bootstrap` e siga `nextAction` (REVIEW → QA → CLOSE, nessa ordem).
4. Se uma wave falha (REJECTED após 2 fix-loops, ou BLOCKED) → ver Escalation Statuses + `../resume/fix-loop-wave.md`.

## Step 12d — Dependency Precheck (factual gate)

Antes de despachar a wave, rode:

```bash
mustard-rt run dependency-precheck --spec {operationalSpecPath}
```

Parse o JSON. Se `ok: false`:

1. Imprima inline: `BLOCKED — N símbolos ausentes: {missing.symbol}. Sugestão: criar tactical-fix.`
2. Emita `mustard-rt run emit-pipeline --kind pipeline.dispatch_failure --spec {specName} --payload "..."`.
3. AskUserQuestion: **Criar tactical-fix automaticamente** / **Investigar manualmente** / **Forçar dispatch (override)**.

**Skip se `resume-bootstrap` retornou `mode: continued`** ou env `MUSTARD_DEPENDENCY_PRECHECK_MODE=off`.

## Escalation Statuses

Após cada agente retornar, cheque o return value antes de avançar:

| Status | Tratamento |
|--------|------------|
| Internal error | Re-despache sequencial, max 1 retry. Ainda falhando → STOP + report |
| `CONCERN` | Record verbatim sob `## Concerns`; continue. ≥2 → surface juntas antes de avançar |
| `BLOCKED` | Pare; AskUserQuestion com blocker exato; NÃO avance |
| `PARTIAL` | Granular Retry Protocol; NÃO restart |
| `DEFERRED` | Note na spec; pergunte se load-bearing antes de CLOSE |
| REJECTED (após REVIEW) | Fix Loop Protocol (max 2 loops); 2 fails → STOP |
| Wave failure | Update `failedWaves`, escreva `failure.md`, AskUserQuestion |

Ver `.claude/pipeline-config.md § Escalation Statuses` e `../resume/fix-loop-wave.md` para detalhes.

## INVIOLABLE RULES

- Main context **IS** o Pipeline Runner — NUNCA wrap em single Task agent.
- NEVER implementar código diretamente — ALL via Task agents (1 per subproject per wave).
- Wave dispatch: TODOS os agentes da mesma wave em UMA SINGLE message.
- Cada sub-agent lê seu próprio `{subproject}/CLAUDE.md` + auto-loads relevant skills.
- ALWAYS use `mustard-rt run agent-prompt-render` para montar prompt — NUNCA from scratch.
- ALWAYS use `mustard-rt run resume-bootstrap` para decidir modo/path/diff/slice/`nextAction` — NUNCA reimplementar essas regras no SKILL.
- ALWAYS rode REVIEW + QA antes de CLOSE — o `pipeline.complete` é refused (exit 2) sem `qa.result`(overall=pass). Siga `nextAction` cego.
- ALWAYS rode dependency-precheck (Step 12d) antes de dispatch.
- Wave plan CLOSE só quando `currentWave === totalWaves` E `nextAction === "emit-complete"`.
