# Token Economy v2 — Plano de Implementação

## Context

O CRM Sialia gasta ~$85/dia com Mustard ativo. Análise CodeBurn de 2026-04-15 revelou que o Mustard otimiza apenas ~15% do custo (CLI output + overhead). Os outros 85% — modelo pensando + contexto carregado — passam direto. Objetivo: reduzir para ~$30-46/dia sem perder qualidade. Routing agressivo (Sonnet para CRUD) foi VETADO pelo usuário.

4 iniciativas aprovadas, implementadas no Mustard templates (afetam todos os projetos que usam Mustard).

---

## Iniciativa 1: Output Budget Hook

**Problema:** Agents retornam respostas verbosas que o orchestrator descarta. 440.3K output tokens/dia, muitos desperdiçados.

**Approach:** Dual-layer — hook de observabilidade + caps explícitos no agent-prompt template.

### Arquivos

| Arquivo | Ação | Descrição |
|---------|------|-----------|
| `templates/hooks/output-budget.js` | **CRIAR** | PostToolUse hook: mede response size, emite métrica, injeta warning via `additionalContext` quando over-budget |
| `templates/commands/mustard/templates/agent-prompt/SKILL.md` | MODIFICAR | Adicionar return cap explícito na seção EFFICIENCY |
| `templates/pipeline-config.md` | MODIFICAR | Adicionar coluna "Max Return" na tabela Token Budget |
| `templates/settings.json` | MODIFICAR | Registrar output-budget.js em PostToolUse → Task |

### Detalhes do hook `output-budget.js`

```
Evento: PostToolUse, matcher: Task
Input: data.tool_input.subagent_type, data.tool_input.description, data.tool_response
Budgets (linhas):
  - Explore: 30 linhas
  - general-purpose (impl): 40 linhas  
  - general-purpose (review): 60 linhas
  - Plan: 80 linhas
Ação: emitMetric + additionalContext warning se over-budget
NÃO bloqueia, NÃO modifica response (outros hooks lêem tool_response do stdin)
```

### Mudança no agent-prompt template (SKILL.md)

Na seção `## EFFICIENCY`, adicionar:
```
- Return cap: follow pipeline-config.md Max Return limits. Focus on: files changed, non-obvious decisions, blockers only.
```

### Mudança no pipeline-config.md

Tabela Token Budget ganha coluna:
```
| Agent Type | Max Context | Max Tool Uses | Max Return |
|------------|-------------|---------------|------------|
| impl       | ≤5K tokens  | —             | 40 lines   |
| explorer   | ≤2.5K tokens| ≤20           | 30 lines   |
| review     | ≤3K tokens  | —             | 60 lines   |
| plan       | —           | —             | 80 lines   |
```

### Registro em settings.json

Adicionar ANTES dos hooks existentes no matcher `Task` do PostToolUse (linha 168):
```json
{
  "type": "command",
  "command": "node \"$CLAUDE_PROJECT_DIR\"/.claude/hooks/output-budget.js",
  "timeout": 3
}
```

---

## Iniciativa 2: Pipeline Skip Intelligence (Extended Light)

**Problema:** Features que seguem padrões conhecidos (add field, add endpoint em entidade existente) são classificadas como Full e passam por PLAN → /approve → /resume — 3+ sessões quando 1 bastaria.

**Approach:** Expandir critérios de Light scope para incluir "Extended Light".

### Arquivos

| Arquivo | Ação | Descrição |
|---------|------|-----------|
| `templates/commands/mustard/feature/SKILL.md` | MODIFICAR | Adicionar Extended Light na tabela de scope (linhas 70-80) |
| `templates/scripts/analyze-validation.js` | MODIFICAR | Validar que Extended Light requer entidade no registry |

### Mudança no feature/SKILL.md (linhas 70-80)

De:
```
| Signal | → Scope |
|--------|---------|
| 1-2 layers, ≤5 files, known pattern, no new entity | **Light** |
| 3+ layers, 5+ files, new entity/CRUD, new pattern | **Full** |
```

Para:
```
| Signal | → Scope |
|--------|---------|
| 1-2 layers, ≤5 files, known pattern, no new entity | **Light** |
| Entity in registry + modification (add field/column/endpoint/behavior) + ≤8 files, no new entity/table/enum | **Extended Light** |
| 3+ layers, 5+ files, new entity/CRUD, new pattern | **Full** |

**Extended Light** = same flow as Light (skip PLAN, inline EXECUTE):
- Entity MUST exist in `entity-registry.json`
- Operation modifies existing entity (NOT creates new)
- Up to 8 files, up to 3 layers — pattern is known
- No new database table, no new enum type, no new module
- If ANY condition fails → reclassify as Full
- Record `scope: "extended-light"` in spec header
- Reclassify to Full if >8 files surface during ANALYZE
```

### Mudança no analyze-validation.js

Adicionar regra: se scope é `extended-light` mas spec referencia entidade não encontrada no registry → issue `"Extended Light requires entity in registry"`.

---

## Iniciativa 3: Lazy Context / Diff-Context Scoping

**Problema:** `diff-context.js` mostra ALL changed files de todos subprojetos. Backend agent vê diffs do frontend e vice-versa. São ~3000 chars × N agents de contexto irrelevante por pipeline.

**Approach:** Adicionar flag `--subproject <path>` ao diff-context.js.

### Arquivos

| Arquivo | Ação | Descrição |
|---------|------|-----------|
| `templates/scripts/diff-context.js` | MODIFICAR | Adicionar `--subproject` flag que filtra git commands |
| `templates/commands/mustard/feature/SKILL.md` | MODIFICAR | Instrução para passar `--subproject` ao gerar diff |
| `templates/commands/mustard/resume/SKILL.md` | MODIFICAR | Mesmo: passar `--subproject` |

### Mudança no diff-context.js

Após parsear `--parent` (linha 34), adicionar parse de `--subproject`:
```javascript
const subIdx = args.indexOf('--subproject');
const subPath = subIdx >= 0 && args[subIdx + 1] ? args[subIdx + 1] : null;
```

Em cada chamada `git diff/status/ls-files`, quando `subPath` definido, adicionar `-- ${subPath}` ao final:
- `git diff --cached --stat -- ${subPath}`
- `git diff --cached --name-only -- ${subPath}`
- `git diff --stat -- ${subPath}`
- `git diff --name-only -- ${subPath}`
- `git ls-files --others --exclude-standard -- ${subPath}`
- `git log --oneline ${mergeBase}..HEAD -- ${subPath}`
- `git diff --stat ${mergeBase}..HEAD -- ${subPath}`

Backward compatible: sem `--subproject` mantém comportamento global.

### Mudança nos pipeline commands

Na instrução de diff-context, mudar de:
```
node .claude/scripts/diff-context.js
```
Para:
```
node .claude/scripts/diff-context.js --subproject {subproject_path}
```
Gerar diff uma vez por subprojeto envolvido. Cada agent recebe só o diff do seu subprojeto.

---

## Iniciativa 4: Recipe Engine

**Problema:** Modelo raciocina do zero para cada CRUD quando o pattern é idêntico. 50+ specs completadas no CRM seguem o mesmo Entity→Repository→Service→Module→DTO→Controller.

**Approach:** Script `recipe-match.js` + formato de recipe JSON + integração no pipeline dispatch.

### Arquivos

| Arquivo | Ação | Descrição |
|---------|------|-----------|
| `templates/scripts/recipe-match.js` | **CRIAR** | Lê `.claude/recipes/*.json`, match por entity+operation, output JSON |
| `templates/commands/mustard/feature/SKILL.md` | MODIFICAR | Adicionar passo recipe-match no EXECUTE |
| `templates/commands/mustard/templates/agent-prompt/SKILL.md` | MODIFICAR | Adicionar placeholder `{recipe_context}` |
| `templates/pipeline-config.md` | MODIFICAR | Documentar recipe engine e formato |

### Formato de recipe (`.claude/recipes/{operation}.json`)

```json
{
  "name": "add-field",
  "description": "Add a new field to an existing entity",
  "operations": ["add-field", "add-column", "new-field"],
  "requires_entity": true,
  "files": [
    {
      "pattern": "{backend}/Modules/v1/{Entity}/Entities/{Entity}Entity.cs",
      "action": "modify",
      "hint": "Add property: public {Type} {Name} { get; set; }"
    },
    {
      "pattern": "{backend}/Modules/v1/{Entity}/DTOs/{Entity}ResponseDto.cs",
      "action": "modify", 
      "hint": "Add DTO field"
    }
  ],
  "checklist": [
    "Add property to Entity class",
    "Add field to DTOs (Request + Response)",
    "Add migration if needed",
    "Build + type-check"
  ]
}
```

### Script recipe-match.js

```
Usage: node .claude/scripts/recipe-match.js --entity <name> --operation <type>
Input: reads .claude/recipes/*.json
Output: JSON com recipe matched + resolved file paths (entity interpolated)
Exit 0 com output vazio se nenhum match
```

Padrão Mustard: CommonJS, Node.js built-ins only, fail-open.

### Integração no pipeline dispatch

No EXECUTE do feature/SKILL.md, antes de dispatch:
```
Run: node .claude/scripts/recipe-match.js --entity {entity} --operation {operation}
If output non-empty, prepend to agent prompt:
  ## RECIPE (follow this pattern — fill in specifics)
  {recipe_output}
```

No agent-prompt/SKILL.md, adicionar entre ENTITY e SKILLS:
```
## RECIPE
{recipe_context}
```

---

## Ordem de Implementação

| # | Iniciativa | Arquivos | Prioridade |
|---|-----------|----------|------------|
| 1 | Output Budget Hook | 4 (1 novo, 3 mod) | Quick win |
| 2 | Pipeline Skip (Extended Light) | 2 (ambos mod) | Médio |
| 3 | Lazy Context (diff scoping) | 3 (todos mod) | Médio |
| 4 | Recipe Engine | 4 (1 novo, 3 mod) | Maior impacto |

## Verificação

1. **Output Budget**: `node --test hooks/__tests__/hooks.test.js` + verificar métrica em pipeline state
2. **Pipeline Skip**: Criar spec com entidade existente + 6 files → deve classificar Extended Light
3. **Diff Scoping**: `node scripts/diff-context.js --subproject templates/hooks` → mostra só hooks
4. **Recipe Engine**: `node scripts/recipe-match.js --entity Contract --operation add-field` → output JSON com paths resolvidos
5. **Build geral**: `npm run build && npm test`
