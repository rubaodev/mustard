# Plano: Memória Cross-Session Funcional + Token Economy

## Contexto

O Mustard tem uma arquitetura de memória completa **já escrita** mas **nunca ativada**:
- `memory-write.js` (227 LOC) — grava findings de agentes. Nunca invocado.
- `memory-persist.js` (119 LOC) — grava decisões/lessons. Nunca invocado.
- `session-memory.js` — tenta injetar decisions/lessons no SessionStart, mas os arquivos não existem.
- `.agent-memory/` — deletado a cada SessionEnd pelo session-cleanup.js.

Resultado: agentes redescobrem tudo do zero a cada sessão.

Paralelamente, hooks injetam ~3.625 tokens por Full Feature pipeline com duplicação desnecessária.

---

## Fase 1 — Token Economy (rápido, ~2h)

### 1.1 Dedup persistent memory no SubagentStart

**Problema**: Persistent memory injetada 2x — no SessionStart (session-memory.js, ~500 tokens) e em cada SubagentStart (subagent-tracker.js, ~75 tokens × N agentes).

**Ação**: Remover re-injeção de decisions/lessons do subagent-tracker.js. Subagents já herdam o contexto do parent.

| Arquivo | Mudança |
|---------|---------|
| `templates/hooks/subagent-tracker.js` | Remover bloco persistent memory (~linhas 292-305) |

**Economia**: ~300 tokens por pipeline (4 agentes).

### 1.2 Knowledge injection uma vez no SessionStart

**Problema**: subagent-tracker.js injeta last-10 knowledge entries em cada SubagentStart (~125 tokens × N agentes).

**Ação**: 
- Remover injeção de knowledge.json do subagent-tracker.js
- Mover para session-memory.js com ranking por confidence (top-5, não last-10)

| Arquivo | Mudança |
|---------|---------|
| `templates/hooks/subagent-tracker.js` | Remover bloco [Project Knowledge] (~linhas 308-319) |
| `templates/hooks/session-memory.js` | Adicionar carregamento de knowledge.json filtrado por confidence > 0.5 |

**Economia**: ~500 tokens por pipeline + melhor qualidade (confidence-based).

### 1.3 Reduzir Agent Memory budget

**Ação**: `MEMORY_MAX_CHARS` de 1500 → 800 no subagent-tracker.js.

| Arquivo | Mudança |
|---------|---------|
| `templates/hooks/subagent-tracker.js` | `MEMORY_MAX_CHARS = 800` |

**Economia**: ~175 tokens por agente quando memória estiver ativa.

---

## Fase 2 — Memória Funcional (~5h)

### 2.1 Preservar .agent-memory/ entre sessões

**Problema**: session-cleanup.js deleta .agent-memory/ a cada SessionEnd. Cross-session impossível.

**Ação**: Mudar de "delete all" para "prune > 7 dias".

| Arquivo | Mudança |
|---------|---------|
| `templates/hooks/session-cleanup.js` | Preservar .agent-memory/, deletar apenas entries com >7 dias |

### 2.2 Ativar memory-write.js via SubagentStop

**Problema**: Quando um subagent termina, seus findings morrem.

**Ação**: No subagent-tracker.js, na função handleStop():
1. Ler tool_response do agente (vem no stdin do SubagentStop)
2. Extrair summary (primeiro parágrafo, max 300 chars)
3. Chamar `execFileSync(process.execPath, [memoryWriteScript], { input })`

| Arquivo | Mudança |
|---------|---------|
| `templates/hooks/subagent-tracker.js` | Em handleStop(), invocar memory-write.js com summary do agente |

### 2.3 Ativar memory-persist.js nos pipeline gates

**Problema**: Decisões do /approve e lessons do /complete não são persistidas.

**Ação**: Adicionar instrução nos commands para chamar memory-persist.js via Bash.

| Arquivo | Mudança |
|---------|---------|
| `templates/commands/mustard/approve.md` | Step: persistir decisões via `node .claude/scripts/memory-persist.js` |
| `templates/commands/mustard/complete.md` | Step: persistir lessons via `node .claude/scripts/memory-persist.js` |

### 2.4 session-memory.js inteligente

**Problema**: Injeta "last 10" de arquivos que não existiam. Agora que existirão, precisa ser inteligente.

**Ação**: Reescrever para:
1. knowledge.json → top-5 por `confidence × recency` (confidence > 0.5)
2. decisions.json → last 5 (se existir)
3. lessons.json → last 5 (se existir)
4. Budget: 2000 chars (já existe)
5. Prioridade: decisions > lessons > knowledge

| Arquivo | Mudança |
|---------|---------|
| `templates/hooks/session-memory.js` | Reescrever com ranking por relevância + carregar knowledge.json |

---

## Resumo de Arquivos

| Arquivo | Fase | Tipo |
|---------|------|------|
| `templates/hooks/subagent-tracker.js` | 1.1, 1.2, 1.3, 2.2 | Editar |
| `templates/hooks/session-memory.js` | 1.2, 2.4 | Reescrever |
| `templates/hooks/session-cleanup.js` | 2.1 | Editar |
| `templates/commands/mustard/approve.md` | 2.3 | Editar |
| `templates/commands/mustard/complete.md` | 2.3 | Editar |

## Fluxo Antes vs. Depois

```
ANTES:
  Agente trabalha → findings morrem
  /approve → decisões morrem  
  /complete → lessons morrem
  Próxima sessão → SessionStart injeta nada (arquivos não existem)
  Próximo agente → recebe last-10 KB genérico (125 tokens × cada agente)

DEPOIS:
  Agente termina → memory-write.js grava finding em .agent-memory/
  /approve → memory-persist.js grava decisões em memory/decisions.json
  /complete → memory-persist.js grava lessons em memory/lessons.json
  Próxima sessão → session-memory.js injeta top-5 KB + last-5 decisions + last-5 lessons (uma vez, ~500 tokens)
  Próximo agente → recebe agent memory relevante (800 chars max)
```

## Verificação

- [ ] `node --test hooks/__tests__/hooks.test.js` passa
- [ ] Após SubagentStop: `.claude/.agent-memory/` tem entry com summary
- [ ] Após /approve: `.claude/memory/decisions.json` existe com entries
- [ ] Após /complete: `.claude/memory/lessons.json` existe com entries
- [ ] SessionStart: additionalContext inclui [Persistent Memory] com conteúdo real
- [ ] SessionEnd: `.agent-memory/` preservado (não deletado)
- [ ] Full Feature pipeline: tokens de hooks ≤ 2.500 (antes: 3.625)
