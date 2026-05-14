# Plano — Correções Fatais do Mustard (completo)

## Contexto

Auditoria brutal encontrou 3 falhas fatais (métricas inventadas, skills sem validação, generator hardcoded) + itens que originalmente ficariam "pra depois". Usuário pediu pra trazer tudo. Este plano agora cobre 6 PRs, do mais crítico ao mais ambicioso, cada um isolado e deployável sozinho.

**Princípio**: subtrair antes de adicionar. Cada PR é cirúrgico — nenhum toca > 3 arquivos.

**Descobertas novas verificadas**:
- `rtk gain --format json` existe (não `--json`) — dá pra importar telemetria real.
- `/mustard:skill` (`templates/commands/mustard/skill/SKILL.md:89,102,146`) já documenta o campo `source: scan | manual` para distinguir origem. `skill-generator.js` não emite esse campo hoje — fix trivial.
- `skill-generator.js` tem 2.090 linhas, maioria strings-template inline. Extração é justificável agora.
- `sync-compile.js` concatena .md sem dedup. Cada agente carrega overlap.

---

## PR 1 — Parar de inventar métricas RTK (P1, ~2h)

**Problema**: `rtk-rewrite.js:144-163` emite `tokens_saved` via heurística `cmd.length × 10 × rate`. `metrics-report.js` agrega como verdade.

**Arquivos**:
- `templates/hooks/rtk-rewrite.js` (editar)
- `templates/scripts/rtk-gain-import.js` (novo)
- `templates/scripts/metrics-report.js` (ajustar agregação)

**Mudanças**:

1. Remover `SAVINGS_RATES` + `estimateSavings` (linhas 118-149) de `rtk-rewrite.js`.
2. Simplificar `emitMetric` — manter só `tokensAffected` (tamanho do comando). Remover `tokensSaved` e `savings_rate`.
3. Criar `rtk-gain-import.js`: executa `rtk gain --format json -p`, parseia, emite eventos `rtk-gain` para `.claude/.metrics/rtk-gain.jsonl` com valores **reais**.
4. Em `metrics-report.js`: quando agregar, `rtk-rewrite` soma apenas `tokensAffected`. Eventos `rtk-gain` (novo event type) somam economia real.
5. **Bug menor**: filtrar primeira linha não-JSON do `.jsonl` (stderr leak do RTK). Em `_metrics-write.js`/`metrics-emit.js` validar JSON antes do append.

**Verificação**:
```bash
node templates/scripts/rtk-gain-import.js
node templates/scripts/metrics-report.js --event rtk-rewrite  # tokensSaved ausente
node templates/scripts/metrics-report.js --event rtk-gain     # números reais
```

---

## PR 2 — Validação de skills pós-geração (P2, ~1h)

**Problema**: `skill-generator.js:83-104` (writeFile) escreve direto. Frontmatter torto, triggers ausentes ou duplicatas passam.

**Arquivo**: `templates/scripts/skill-generator.js`

**Mudança**: nova função `validateSkill(content, filePath)` chamada antes de cada `writeFile` com sufixo `SKILL.md`. Retorna `{ ok, errors[] }`. Bloqueia escrita se falhar (a menos que `--force`).

**Regras**:
- Frontmatter YAML presente (`---\n...\n---`)
- `name` em kebab-case: `^[a-z][a-z0-9-]+$`
- `description` entre 50 e 600 chars
- `description` contém triggers (`use when|when the user|add|create|new|...`)
- Dedup: `Set` de names já escritos nesta run; duplicata = warn + skip.

**Verificação**:
```bash
# Quebrar template artificialmente (remover description line) e rodar
node templates/scripts/skill-generator.js --dry-run
# Deve listar [invalid] ... missing "description"
bun test templates/hooks/__tests__/hooks.test.js  # nada regride
```

---

## PR 3 — Unificar os 3 caminhos de skills via `source:` (P2, ~30min)

**Problema**: `skill-creator`, `/mustard:skill`, `skill-generator.js` podem se atropelar. A solução já está DOCUMENTADA mas não IMPLEMENTADA:
- `templates/commands/mustard/skill/SKILL.md:89` mostra `source: manual | scan` na tabela `/skill list`.
- `templates/commands/mustard/skill/SKILL.md:102` protege `source: scan` em `/skill remove`.
- `templates/commands/mustard/skill/SKILL.md:146` diz "NEVER delete skills that are `source: manual` without user confirmation".
- **Mas** `skill-generator.js` não emite `source: scan` no frontmatter de nada que gera.

**Mudanças**:

1. Em `skill-generator.js`, adicionar `source: scan` ao frontmatter de todo SKILL.md gerado. Ex:
   ```yaml
   ---
   name: {sub}-entity-creation
   description: "..."
   source: scan
   ---
   ```

2. Em `/mustard:skill install` (fluxo em `templates/commands/mustard/skill/SKILL.md:44-48`): adicionar step de pós-processamento que injeta `source: manual` se o frontmatter do SKILL.md instalado não tiver `source:`.

3. **Regra de território** (adicionar em `templates/commands/mustard/skill/SKILL.md` Rules):
   - `skill-generator.js` só escreve `source: scan`. Nunca toca `source: manual`.
   - `/skill install`, `/skill create`, `skill-creator` só escrevem `source: manual`. Nunca tocam `source: scan`.
   - Campo `source:` ausente = tratado como `manual` (conservador, protege usuário).

4. Validator do PR 2 passa a exigir `source` no frontmatter gerado (apenas para skill-generator emissions).

**Verificação**:
```bash
node templates/scripts/skill-generator.js
grep 'source:' .claude/skills/**/SKILL.md  # generated → scan; manuais → manual
```

---

## PR 4 — Mappers hardcoded → JSON (P3, ~1h)

**Problema**: `skill-generator.js:141-164, 204-232` tem 3 mappers inline + detect duplicado com `scanner-loader.js`.

**Mudança**:

1. Criar `templates/scripts/_skill-meta.json`:
```json
{
  "stacks": {
    "dotnet":     { "lang": "csharp",     "label": ".NET",               "files": [".csproj", ".sln"],        "isExt": true },
    "typescript": { "lang": "typescript", "label": "TypeScript/Node.js", "files": ["package.json"]                        },
    "dart":       { "lang": "dart",       "label": "Flutter/Dart",       "files": ["pubspec.yaml"]                        },
    "php":        { "lang": "php",        "label": "Laravel/PHP",        "files": ["composer.json", "artisan"]            },
    "python":     { "lang": "python",     "label": "Python",             "files": ["pyproject.toml", "requirements.txt"]  },
    "java":       { "lang": "java",       "label": "Spring/Java",        "files": ["pom.xml", "build.gradle"]             },
    "go":         { "lang": "go",         "label": "Go",                 "files": ["go.mod"]                              },
    "rust":       { "lang": "rust",       "label": "Rust",               "files": ["Cargo.toml"]                          }
  },
  "roles": {
    "api": "backend", "ui": "frontend", "database": "database",
    "mobile": "mobile", "library": "backend", "general": "general"
  }
}
```

2. Em `skill-generator.js`, substituir os 3 mappers (`roleToAgent`, `stackLang`, `stackLabel`) e `detectStackFromPath` por lookups no JSON carregado uma vez no start.

3. Adicionar Kotlin e Elixir como teste de escalabilidade — deve ser 1 linha no JSON, zero em JS.

**Verificação**:
```bash
grep -c 'const map = {' templates/scripts/skill-generator.js  # era 3, agora 0
# Adicionar kotlin no JSON, rodar sync num projeto Kotlin dummy
```

---

## PR 5 — Extrair templates inline do skill-generator (P3, ~3h)

**Problema**: `skill-generator.js` tem 2.090 linhas, maioria strings-template inline com 19+ pontos de `generated at:${iso} role:${role}`. Modificar um template = editar JS + redeploy. Dificulta contribuições.

**Mudança**:

1. Criar pasta `templates/skill-templates/`:
   ```
   templates/skill-templates/
   ├── entity-creation.md.tmpl
   ├── entity-creation.examples.md.tmpl
   ├── api-endpoint.md.tmpl
   ├── api-endpoint.examples.md.tmpl
   └── ... (uma por skill-type × stack se necessário)
   ```

2. Template engine minimalista em `skill-generator.js` (10 linhas, zero dep):
```js
function render(tmpl, vars) {
  return tmpl.replace(/\{\{(\w+(?:\.\w+)*)\}\}/g, (_, path) => {
    const val = path.split('.').reduce((o, k) => o?.[k], vars);
    return val == null ? '' : String(val);
  });
}
function renderBlock(tmpl, cond, block) { // {{#if baseClass}}...{{/if}}
  return tmpl.replace(/\{\{#if (\w+)\}\}([\s\S]*?)\{\{\/if\}\}/g, (_, k, b) => vars[k] ? b : '');
}
```

3. Substituir blocos `genEntityCreationSkill`, `genApiEndpointSkill`, etc. por:
```js
const tmpl = fs.readFileSync(path.join(TPL_DIR, 'entity-creation.md.tmpl'), 'utf-8');
const skillMd = render(tmpl, { sub, stackId, lang, label, folder, ..., iso, role });
```

4. Arquivo gerador encolhe de ~2.090 para ~600 linhas.

**Risco**: refactor grande, mudança de comportamento possível. **Mitigação**: antes/depois, comparar output byte-a-byte num projeto real; escrever teste que congela saída (`templates/hooks/__tests__/skill-generator.snapshot.test.js`).

**Verificação**:
```bash
# Antes do refactor
node templates/scripts/skill-generator.js --subproject x > before.txt
# Depois
node templates/scripts/skill-generator.js --subproject x > after.txt
diff before.txt after.txt  # vazio
wc -l templates/scripts/skill-generator.js  # <= 700
```

---

## PR 6 — Dedup + pruning em sync-compile (P3, ~2h)

**Problema**: `sync-compile.js` concatena `.md` files em `{agent}.context.md` sem detectar overlap. Se `context/shared/foo.md` for referenciado por backend E frontend, ambos carregam. O nome "modular context" implica isolamento; a implementação faz concat.

**Mudança**:

1. Em `sync-compile.js`: ao concatenar, calcular SHA-256 de cada seção (por heading ou por arquivo-fonte). Manter índice `compiled/_index.json`:
```json
{
  "backend.context.md": { "sections": ["sha-A", "sha-B", "sha-C"] },
  "frontend.context.md": { "sections": ["sha-A", "sha-D"] }
}
```

2. Se sha já apareceu no mesmo arquivo-alvo, pular (dedup intra-agente). Dedup inter-agente é opcional — geralmente não queremos porque cada agente precisa auto-contido.

3. **Pruning por `recommended_skills`**: se a spec/pipeline declara `recommended_skills: [x, y]`, sync-compile só inclui esses — ignora skills não mencionados. Hoje (ao que parece) inclui todos os skills do agente. Reduz ~20-40% de contexto quando o user sabe o que quer.

4. Medir: instrumentar `sync-compile.js` para emitir event `compile-size` com `{agent, bytes_in, bytes_out, deduped_bytes}`. `metrics-report.js` expõe isso — números **reais** de economia.

**Verificação**:
```bash
# Antes
wc -c .claude/compiled/backend.context.md
# Aplicar PR e rodar
node templates/scripts/sync-compile.js
wc -c .claude/compiled/backend.context.md
node templates/scripts/metrics-report.js --event compile-size
```

**Sem benchmark não prometer X%**. Só medir e reportar.

---

## O que NÃO está aqui

- Lazy-load de skills. Claude Code já faz isso via description matching — o que parecia bloat era concat não-podada em `sync-compile`. PR 6 cobre.
- Refactor de hooks sólidos — não tocar em `context-budget.js`, `tool-use-counter.js`, `bash-native-redirect.js`.
- Dead notes (`ajuste-mustard-git.md`, `plans/analise-*.md`). Limpeza de higiene, fora do escopo "fatal".
- Windows paths — já estão corretos para Git Bash.

---

## Ordem de execução

| # | PR | Prioridade | Custo | Impacto |
|---|-----|------------|-------|---------|
| 1 | RTK metrics honestas | P1 | 2h | Credibilidade imediata |
| 2 | Validador de skills | P2 | 1h | Proteção de contexto |
| 3 | `source:` field enforcement | P2 | 30min | Unifica os 3 caminhos |
| 4 | Mappers → JSON | P3 | 1h | Escalabilidade de stack |
| 5 | Extrair templates inline | P3 | 3h | Manutenibilidade |
| 6 | Dedup sync-compile | P3 | 2h | Performance real (medível) |

**Total**: ~9.5h. Pode ser dividido em 2 sessões (P1+P2+P3 numa, P4+P5+P6 outra).

**Dependências**: PR 2 (validador) e PR 3 (`source:`) devem ser combinados — o validador passa a exigir `source` que o PR 3 emite. Os demais são independentes.

---

## Arquivos afetados

| Arquivo | PR | Tipo |
|---------|-----|------|
| `templates/hooks/rtk-rewrite.js` | 1 | editar |
| `templates/scripts/rtk-gain-import.js` | 1 | novo |
| `templates/scripts/metrics-report.js` | 1, 6 | editar |
| `templates/hooks/_lib/metrics-emit.js` | 1 | editar (filtrar não-JSON) |
| `templates/scripts/skill-generator.js` | 2, 3, 4, 5 | editar (encolhe) |
| `templates/commands/mustard/skill/SKILL.md` | 3 | editar (rules) |
| `templates/scripts/_skill-meta.json` | 4 | novo |
| `templates/skill-templates/*.md.tmpl` | 5 | novos (~6-10 arquivos) |
| `templates/hooks/__tests__/skill-generator.snapshot.test.js` | 5 | novo |
| `templates/scripts/sync-compile.js` | 6 | editar |

2 arquivos novos em PR 1, 1 em PR 4, 6-10 em PR 5, 1 teste em PR 5. Nenhuma quebra de API pública.

---

## Verificação end-to-end (tudo junto)

```bash
# Métricas honestas
node templates/scripts/rtk-gain-import.js
node templates/scripts/metrics-report.js | grep -E 'rtk-rewrite|rtk-gain'

# Skills válidos com source
node templates/scripts/skill-generator.js --dry-run
grep -rh '^source:' .claude/skills/ | sort | uniq -c  # scan N | manual M

# Sem hardcode
grep -c 'const map = {' templates/scripts/skill-generator.js  # 0
wc -l templates/scripts/skill-generator.js  # <= 700

# Dedup medível
node templates/scripts/sync-compile.js
node templates/scripts/metrics-report.js --event compile-size

# Regressão zero
bun test templates/hooks/__tests__/hooks.test.js
```

Se os 5 comandos acima passam, os 6 PRs estão entregues. Nenhum "acreditar por fé" — tudo mensurável.
