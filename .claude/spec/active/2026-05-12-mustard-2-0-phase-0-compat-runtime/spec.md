# Mustard 2.0 — Phase 0: Runtime Compatibility Layer

- **Lang**: ptbr
- **Phase**: PLAN
- **Scope**: Full
- **Type**: feature
- **Model**: opus
- **Depends on**: none
- **Unlocks**: Phase 1 (Event Store)

## Summary

Detectar e adotar Bun runtime (Claude Code v2.1.113+) com fallback transparente pra Node.js em versões antigas. Sem refatorar nada ainda — apenas a fundação que permite Phase 1+ usar `bun:sqlite`, TypeScript nativo e 10x cold-start. Zero breakage em projetos existentes.

## Problem

Hooks rodam como child process por tool call. Cada PreToolUse é bloqueante. No sialia medimos 1172 tool.use events × 26 hooks ≈ ~50 min de cold-start acumulado em histórico (Node spawn ~100-300ms cada). Bun cold-start é 10-30ms.

Hoje Mustard escreve hooks em CommonJS Node-only por causa do "zero npm deps after init". Mas Anthropic adquiriu Bun em dez/2025 e Claude Code v2.1.113+ **já vem com Bun nativo**. Não é mais nova dep — é runtime padrão da casa.

## Goal

`mustard init` e `mustard update` detectam runtime, escrevem shebang correto, e produzem templates que rodam idênticos em Bun ou Node. Hooks ficam compatíveis com ambos sem build step.

## Acceptance Criteria

Todas com comando executável. Strict pass.

1. **Detecção de runtime**
   ```bash
   node -e "const r=require('./dist/runtime/detect-runtime.js'); const x=r.detect(); console.log(x.kind==='bun'||x.kind==='node'?'PASS':'FAIL', x)"
   ```
   Exit 0 quando retorna `{ kind, version, bunSqliteAvailable }`.

2. **Shebang dual no template hook**
   ```bash
   node -e "const fs=require('fs');const f=fs.readFileSync('templates/hooks/_lib/runtime-shim.js','utf8');process.exit(f.includes('#!/usr/bin/env')?0:1)"
   ```
   `runtime-shim.js` existe com shebang `#!/usr/bin/env node` E exporta função `pickRuntime()`.

3. **Hooks rodam sob Bun**
   ```bash
   bun templates/hooks/_lib/__tests__/runtime-shim.test.js
   ```
   Test passa em Bun (se Bun instalado); skip-clean se não.

4. **Hooks rodam sob Node (regression)**
   ```bash
   node --test templates/hooks/__tests__/hooks.test.js
   ```
   Os 100 testes atuais continuam passando.

5. **mustard init grava runtime escolhido**
   ```bash
   node bin/mustard.js init --dry-run --runtime=bun > /tmp/m-bun.txt 2>&1 && grep -q "runtime.*bun" /tmp/m-bun.txt
   ```
   Output do init contém o runtime detectado/escolhido.

6. **Fallback Node se Bun indisponível**
   ```bash
   PATH=$(echo "$PATH" | sed 's|[^:]*bun[^:]*:||g') node bin/mustard.js init --dry-run > /tmp/m-fallback.txt && grep -q "runtime.*node" /tmp/m-fallback.txt
   ```
   Com Bun fora do PATH, init escolhe Node sem erro.

7. **mustard.json registra runtime**
   ```bash
   node -e "const p='.claude/mustard.json';const fs=require('fs');if(!fs.existsSync(p))process.exit(1);const j=JSON.parse(fs.readFileSync(p,'utf8'));process.exit(j.runtime&&j.runtime.kind?0:1)"
   ```
   Campo `runtime: { kind, version, chosenAt }` presente.

8. **Doc de migração**
   ```bash
   test -f docs/runtime-migration.md && grep -q "Bun" docs/runtime-migration.md
   ```
   Doc explica detecção, fallback, e como forçar runtime via env `MUSTARD_RUNTIME=node|bun`.

## Implementation

### New files

- `src/runtime/detect-runtime.ts` — detecta Bun via `typeof Bun !== 'undefined'` + `process.versions.bun`, retorna `{ kind, version, bunSqliteAvailable, claudeCodeVersion }`
- `templates/hooks/_lib/runtime-shim.js` — shebang Node-compat, exporta `pickRuntime()` e helpers que ambos runtimes suportam
- `templates/hooks/_lib/runtime-shim.d.ts` — tipos pra fase futura
- `docs/runtime-migration.md` — doc de transição

### Changed files

- `src/commands/init.ts` — adiciona `--runtime=bun|node|auto`, escreve `mustard.json.runtime`
- `src/commands/update.ts` — preserva `runtime` do mustard.json
- `templates/hooks/_lib/hook-env.js` — usa `runtime-shim.pickRuntime()` pra escolher I/O paths quando relevante

### Env vars novas

- `MUSTARD_RUNTIME=node|bun|auto` (default auto) — força runtime
- `MUSTARD_RUNTIME_VERBOSE=1` — log de detecção em stderr

## Decisions

- **Bun-first quando disponível, Node fallback**: porque Anthropic adquiriu Bun e Claude Code v2.1.113+ ships com ele
- **Sem build step nos hooks**: Bun roda .ts/.js nativo; manter hooks em .js compatível com ambos
- **TypeScript só em `src/` e futuro `_lib/`**: hooks ficam JS por compat máxima

## Risks (já endereçados)

- ~~Bun não no Windows~~ → Bun 1.0+ tem Windows; `bun:sqlite` estável em 2026. Verificação automática + fallback.
- ~~Quebrar projetos existentes~~ → `update.ts` preserva mustard.json existente; novos campos opt-in.

## Out of scope

- SQLite (Phase 1)
- TypeScript em hooks (não vale o build step)
- OpenTelemetry (Phase 2)

## Checklist

- [ ] `src/runtime/detect-runtime.ts` implementado
- [ ] `templates/hooks/_lib/runtime-shim.js` + `.d.ts`
- [ ] `src/commands/init.ts` aceita `--runtime`
- [ ] `src/commands/update.ts` preserva runtime
- [ ] `mustard.json` schema atualizado
- [ ] `docs/runtime-migration.md`
- [ ] Tests Bun + Node passam
- [ ] Sialia testado: `mustard update` não quebra projeto ativo
