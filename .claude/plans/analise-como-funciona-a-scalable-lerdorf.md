# Harness Event Bus — Memória Unificada entre Agentes e Sessões

## Context

Hoje a memória compartilhada do Mustard está fragmentada em **8 stores** com ownership difuso (ver mapeamento abaixo). Consequências práticas:

- Agentes em paralelo **não se enxergam** (só veem findings da wave anterior via `_index.json`).
- Summary truncado em 300 chars + load 800 chars — descobertas longas são perdidas.
- `/mustard:resume` depende de `pipeline-states/{spec}.json` que só tem fase, não histórico.
- Entre sessões, `decisions.json` / `lessons.json` / `knowledge.json` existem mas são gravados por **scripts separados** (`memory-persist.js`, `knowledge-update.js`) sem timeline comum — impossível responder "o que aconteceu na sessão passada com a spec X?".
- Cinco arquivos diferentes (`subagent-tracker.js`, `memory-persist.js`, `knowledge-update.js`, `metrics-tracker.js`, `session-knowledge.js`) escrevem estado — cada um com seu shape.

**Objetivo:** transformar o harness (runtime do Claude Code + hooks lifecycle) em **broker único via event log append-only**, resolvendo:
1. Visibilidade entre agentes paralelos (tail do log em near-real-time).
2. Memória entre sessões com timeline replayable.
3. Redução de stores concorrentes (8 → 2: log bruto + knowledge consolidado).

---

## Arquitetura Alvo

```
.claude/.harness/
├── events.jsonl           # Log append-only, rotaciona por sessão
├── sessions/
│   └── {sessionId}.jsonl  # Arquivo histórico (rotacionado de events.jsonl no SessionEnd)
└── index.json             # View: última posição conhecida por agentId/spec/wave

.claude/knowledge.json     # Única projeção persistente sobrevivente (fold do log)
.claude/memory/            # Preservado: decisions.json + lessons.json (já são projeções)
```

**Removidos (viram projeções on-demand):**
- `.claude/.agent-memory/` (inteiro)
- `.claude/.agent-state/{id}.json` e `_queue.json`
- `.claude/.pipeline-states/{spec}.metrics.json` (sidecar)

**Preservados (com ajuste de origem dos dados):**
- `.claude/.pipeline-states/{spec}.json` — continua como view de alto nível da fase do pipeline, mas gerado a partir do log.
- `.claude/knowledge.json` — continua sendo o fold confidence-ranked; gerado por `knowledge-update.js` lendo log.

---

## Event Schema

Cada linha do `events.jsonl` é um objeto NDJSON com shape:

```jsonc
{
  "v": 1,                           // schema version (migração futura)
  "ts": "2026-04-23T14:22:01.123Z", // ISO-8601 UTC
  "sessionId": "s-abc123",           // do hook context
  "wave": 3,                         // incremento por batch paralelo
  "spec": "add-login",               // opcional (pipeline context)
  "actor": {
    "kind": "agent|orchestrator|hook|user",
    "id": "ag-xyz",                  // agentId quando kind=agent
    "type": "Explore|Plan|general"   // quando kind=agent
  },
  "event": "agent.start|agent.stop|tool.use|pipeline.phase|finding|decision|lesson|dispatch.failure",
  "payload": { /* shape específico por evento */ }
}
```

### Payloads por tipo de evento

| event | payload principal |
|---|---|
| `agent.start` | `{ description, parentAgentId?, model }` |
| `agent.stop` | `{ summary (≤800 chars), confidence, durationMs, toolCount }` |
| `tool.use` | `{ tool, bytesIn, bytesOut }` (heartbeat leve — sem payload completo) |
| `pipeline.phase` | `{ from, to }` (ex.: `ANALYZE→PLAN`) |
| `finding` | `{ kind: "pattern\|convention\|entity", content, confidence, refs[] }` |
| `decision` | `{ title, rationale }` |
| `lesson` | `{ trigger, takeaway }` |
| `dispatch.failure` | `{ reason, retry }` |

**Por que 800 chars em `agent.stop.summary` (vs. 300 hoje):** elimina o truncamento agressivo atual. Log é cheap — o custo está no *load* de contexto, não no disk.

---

## Views Derivadas (read-only, on-demand)

Cada view é função pura do log. Gerada quando um hook precisa.

| View | Gerada por | Input filtrado | Uso |
|---|---|---|---|
| `agent-visibility` | `subagent-tracker.js` em `SubagentStart` | últimos N eventos da `wave` atual + findings ≥ confidence 0.7 | Injetado como `additionalContext` no novo agente |
| `pipeline-state/{spec}` | `/mustard:resume`, `metrics-tracker.js` | eventos com `spec=X` | Fase atual, métricas, dispatch failures |
| `session-summary/{sessionId}` | `session-knowledge.js` em `SessionEnd` | todos eventos da sessão | Fold em knowledge.json + arquivamento |
| `cross-session-timeline` | `session-memory.js` em `SessionStart` | últimas 3 sessões em `.harness/sessions/*.jsonl` | Contexto "o que aconteceu antes" |

**Implementação:** 1 módulo único `.claude/scripts/harness-views.js` exportando funções puras `buildView(events, filter)`. Hooks importam dele.

---

## Memória entre Sessões (ponto explicitamente pedido)

### Como funciona hoje
- `session-memory.js` em `SessionStart` carrega `decisions.json` + `lessons.json` + `knowledge.json` (≤2000 chars) — **sem timeline, sem correlação com sessão específica**.
- `session-knowledge.js` em `SessionEnd` gera até 5 patterns de knowledge — sem contexto de *quando* a decisão foi tomada.

### Como fica
1. **Arquivamento no SessionEnd**: `events.jsonl` da sessão é movido para `.harness/sessions/{sessionId}.jsonl` antes do cleanup. Nada é deletado — apenas rotacionado.
2. **Fold consolidado**: `session-knowledge.js` gera a partir do log:
   - `knowledge.json` (patterns confidence-ranked, como hoje)
   - **Novo:** `decisions`/`lessons` extraídos de eventos `decision`/`lesson` (hoje tem que ser chamado manual via `memory-persist.js` — passa a ser automático).
3. **Replay no SessionStart**: `session-memory.js` adiciona ao `additionalContext`:
   - Top knowledge por confidence (como hoje)
   - **Novo:** `cross-session-timeline` das últimas 3 sessões: spec tocada, fase final, decisões chave. Dá ao orquestrador "memória episódica".
4. **Rotação**: manter `.harness/sessions/*.jsonl` dos últimos 30 dias. Fold continua vivendo em `knowledge.json`.

### Garantia
A pergunta "o que aconteceu na sessão passada com spec X?" passa a ter resposta: `jq 'select(.spec=="X")' .harness/sessions/*.jsonl`. Hoje é impossível.

---

## Ordem de Migração

Feita em 4 waves pequenas, cada uma deployável isolada. Stores antigos e novos coexistem até a última wave — rollback trivial.

### Wave 1 — Introduzir o log (aditivo, zero breaking)
- **Criar** `templates/hooks/_lib/harness-event.js` — helper `emit(event, payload, context)` que faz append atômico em `events.jsonl` (write com `O_APPEND`, lock via `proper-lockfile`-style noop — ou `fs.appendFileSync` que já é atômico ≤PIPE_BUF).
- **Criar** `templates/scripts/harness-views.js` — funções puras `buildAgentVisibility`, `buildSessionSummary`, `buildCrossSessionTimeline`, `buildPipelineState`.
- **Criar** `templates/hooks/harness-init.js` — hook `SessionStart` que garante diretório `.harness/` e limpa sessões >30 dias.
- **Registrar** em `templates/settings.json`: `harness-init.js` em SessionStart (primeiro).

Validação: log existe, cresce, rotaciona. Nenhum hook existente muda.

### Wave 2 — Emissão dupla (hooks atuais emitem eventos em paralelo)
Cada hook existente passa a chamar `emit(...)` **adicionalmente** ao que já faz. Stores antigos continuam sendo escritos.

| Hook | Eventos emitidos |
|---|---|
| `subagent-tracker.js` PreToolUse(Task) | `agent.start` |
| `subagent-tracker.js` SubagentStop | `agent.stop` com summary completo 800 chars |
| `metrics-tracker.js` PostToolUse | `tool.use` (heartbeat) |
| `session-knowledge.js` SessionEnd | `finding` por pattern extraído |
| `memory-persist.js` | `decision` / `lesson` |
| pipeline commands (via hook `pipeline-phase.js` novo) | `pipeline.phase` |

Validação: log tem todos os eventos esperados ao rodar `/mustard:feature` completo.

### Wave 3 — Views substituem stores antigos
- `subagent-tracker.js` em `SubagentStart` passa a ler **view `agent-visibility`** (gerada do log) em vez de `.agent-memory/_index.json`.
- `session-memory.js` em `SessionStart` passa a usar **`cross-session-timeline`** + knowledge.json.
- `/mustard:resume` lê **view `pipeline-state`** derivada do log.
- `statusline` lê view `agent-visibility` para mostrar agentes ativos.

Stores antigos ainda são escritos (defensivo), mas **ninguém mais lê**.

Validação: agentes paralelos começam a se enxergar. `/mustard:resume` funciona. Timeline cross-session aparece em SessionStart.

### Wave 4 — Subtração
- Remover escritas em `.agent-memory/`, `.agent-state/`, `.pipeline-states/*.metrics.json` dos hooks.
- Remover `session-cleanup.js` das rotinas que limpam esses diretórios (eles não existem mais).
- Deletar código morto de `subagent-tracker.js` (lógica de `_index.json`, `_queue.json`).
- Atualizar docs em `templates/CLAUDE.md` e `AGENTS.md`.

Validação: suite de hooks (`bun test hooks/__tests__/hooks.test.js`) passa. Pipeline completo funciona com apenas `.harness/` + `knowledge.json` + `memory/`.

---

## Arquivos Críticos

### Novos
- `templates/hooks/_lib/harness-event.js` — API `emit()`
- `templates/hooks/harness-init.js` — bootstrap + rotação
- `templates/hooks/pipeline-phase.js` — emite `pipeline.phase` (pequeno, dispara em PreToolUse do comando)
- `templates/scripts/harness-views.js` — funções puras de view
- `templates/hooks/__tests__/harness-event.test.js`
- `templates/hooks/__tests__/harness-views.test.js`

### Modificados (nas waves 2 e 3)
- `templates/hooks/subagent-tracker.js` — emit + ler view
- `templates/hooks/session-memory.js` — ler cross-session-timeline
- `templates/hooks/session-knowledge.js` — ler log para fold
- `templates/hooks/metrics-tracker.js` — emit tool.use
- `templates/hooks/session-cleanup.js` — apenas rotação `.harness/sessions/`
- `templates/scripts/memory-persist.js` — delega para `emit('decision'|'lesson')`
- `templates/scripts/knowledge-update.js` — lê log em vez de `.pipeline-states/` + `.agent-memory/`
- `templates/commands/mustard/resume.md` — referência à nova view
- `templates/settings.json` — novos hooks registrados
- `templates/CLAUDE.md` — documentação do event bus

### Removidos (wave 4)
- Código de `_index.json` / `_queue.json` / `{agentId}.json` em `subagent-tracker.js`
- Arquivo sidecar `.metrics.json` (fica inline no log)

---

## Utilidades Reusadas
- `templates/hooks/_lib/hook-env.js` — profiles e env-based disabling; `harness-event.js` respeita o mesmo padrão (fail-open, exit 0 em erro de I/O).
- `fs.appendFileSync` — suficiente para escrita concorrente ≤4KB por linha (limite PIPE_BUF do Windows).
- `readline` (built-in) — streaming do `.jsonl` nas views; evita carregar tudo em memória.
- Padrão de testes existente (`bun test`) — sem novas dependências.

---

## Verification

### Unit (cada wave)
```bash
bun test templates/hooks/__tests__/harness-event.test.js
bun test templates/hooks/__tests__/harness-views.test.js
bun test templates/hooks/__tests__/hooks.test.js   # garantir não-regressão
```

### Integration (após wave 2)
1. `/mustard:feature add-foo` em repo de teste.
2. Verificar `.harness/events.jsonl` tem eventos `agent.start`, `agent.stop`, `pipeline.phase`, `finding`.
3. Conferir `.agent-memory/_index.json` (antigo) e view nova produzem dados equivalentes.

### Integration (após wave 3)
1. Lançar 2 agentes `Explore` em paralelo dentro do mesmo `/mustard:feature`.
2. Confirmar: no `additionalContext` do 2º agente aparece `agent.start` do 1º (impossível hoje).
3. Fechar sessão, abrir nova, rodar `/mustard:resume`: spec volta com fase correta e últimas decisões.

### Cross-session (após wave 3)
1. Sessão A: `/mustard:feature x`, tomar decisão, `/mustard:complete`.
2. Fechar sessão.
3. Sessão B: `SessionStart` deve injetar timeline mencionando spec `x` e a decisão — sem comando do usuário.
4. `jq 'select(.spec=="x")' .claude/.harness/sessions/*.jsonl` — timeline completa recuperável.

### Rollback
Se wave 3 der problema: reverter commits da wave 3. Stores antigos ainda estão sendo escritos (wave 2 preserva), então nada é perdido. Rollback é `git revert` — zero recuperação manual.

---

## Fora de escopo (explícito)

- Não adicionar protocolo agent↔agent direto (Claude Code não suporta; harness é o canal).
- Não introduzir DB (SQLite etc.) — `.jsonl` é suficiente até 10k eventos/sessão.
- Não criar UI/dashboard para o log — `jq` resolve.
- Não mudar modelos de routing, enforcement de budget ou formato de `knowledge.json` (esse continua igual, só muda origem).
