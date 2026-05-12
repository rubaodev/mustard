# Mustard 2.0 — Phase 4: Hardening, Tests, CI

- **Lang**: ptbr
- **Phase**: PLAN
- **Scope**: Full
- **Type**: feature
- **Model**: opus
- **Depends on**: Phase 1, 2, 3
- **Unlocks**: confidence to ship Mustard 2.0 as stable

## Summary

Test coverage ≥80% em código novo (EventStore, KnowledgeBase, TokenTracker, MCP server). CI pipeline (GitHub Actions) rodando lint + type-check + test + benchmark em cada PR. Migration tested em projeto real (sialia). Doc de upgrade para usuários existentes.

## Problem

Esta sessão revelou que mudanças arquiteturais (Wave 4) podem quebrar comportamento sem alerta. complete-spec.js leu schema removido por semanas sem ninguém perceber. 6 bugs estruturais acumulados.

Sem CI + tests, qualquer Phase 1-3 vira o próximo "Wave 4 break". Hardening é o que torna isso institucional.

## Goal

- **Coverage ≥80%** em `src/runtime/`, `src/telemetry/`, `src/mcp/`, `src/migrate/`
- **CI verde** obrigatório pra merge: lint + types + tests + benchmark regression
- **Migration script tested** em snapshot do sialia: rodar, validar, comparar antes/depois
- **Upgrade doc** explica passos pra projetos pre-2.0

## Acceptance Criteria

1. **Coverage report ≥80% em código novo**
   ```bash
   bunx c8 --reporter=json-summary bun test src/ && node -e "const r=require('./coverage/coverage-summary.json').total;const pct=(r.lines.pct+r.branches.pct+r.statements.pct)/3;console.log('coverage:',pct.toFixed(1)+'%');process.exit(pct>=80?0:1)"
   ```
   Média lines/branches/statements ≥80%.

2. **CI GitHub Actions verde**
   ```bash
   gh run list --workflow=ci.yml --branch=dev_rubens --limit=1 --json conclusion -q '.[0].conclusion' | grep -q success
   ```
   Última run no branch `dev_rubens` = success.

3. **Migration script idempotent em sialia snapshot**
   ```bash
   cp -r 'C:/Atiz/Competi/projetos/sialia/.claude' /tmp/sialia-snapshot && \
     node dist/migrate/jsonl-to-sqlite.js /tmp/sialia-snapshot && \
     SHA1=$(sha1sum /tmp/sialia-snapshot/.harness/mustard.db | cut -d' ' -f1) && \
     node dist/migrate/jsonl-to-sqlite.js /tmp/sialia-snapshot && \
     SHA2=$(sha1sum /tmp/sialia-snapshot/.harness/mustard.db | cut -d' ' -f1) && \
     [ "$SHA1" = "$SHA2" ]
   ```
   2x run = mesmo hash do DB.

4. **Benchmark regression check**
   ```bash
   bun run bench && node tests/bench/regression-check.js
   ```
   Hook cold-start ≤30ms (regression de Bun 10ms baseline). FTS5 query ≤5ms. MCP roundtrip ≤10ms p95.

5. **Lint zero warnings**
   ```bash
   bunx eslint src/ --max-warnings=0
   ```

6. **Type-check strict**
   ```bash
   bunx tsc --noEmit --strict -p src/tsconfig.json
   ```
   `strict: true` em tsconfig. Sem `any` injustificado (max 5 `@ts-expect-error` documentados).

7. **Upgrade doc + migration tested em projeto real**
   ```bash
   test -f docs/upgrade-to-2.0.md && grep -q "## Backup" docs/upgrade-to-2.0.md && grep -q "## Rollback" docs/upgrade-to-2.0.md
   ```
   Doc contém seção Backup + Rollback explícitas.

8. **Smoke test pós-migration no sialia**
   ```bash
   cd /tmp/sialia-snapshot && \
     node .claude/scripts/dashboard.js --check && \
     curl -sf http://127.0.0.1:7909/api/metrics | jq '.tokenUsage' | grep -q byPhase
   ```
   Dashboard funciona com DB migrado, retorna tokenUsage da Phase 2.

## Implementation

### Test layout

```
tests/
├── unit/
│   ├── event-store/
│   │   ├── append.test.ts
│   │   ├── query.test.ts
│   │   ├── search.test.ts
│   │   └── rebuild.test.ts
│   ├── knowledge-base/
│   ├── token-tracker/
│   └── migrate/
├── integration/
│   ├── event-store-vs-buildpipelinestate.ts  (regression vs old behavior)
│   ├── mcp-search-knowledge.ts
│   ├── mcp-query-events.ts
│   ├── mcp-similar-specs.ts
│   ├── mcp-latency.ts
│   ├── mcp-sandbox.ts
│   ├── span-duration-correlates.ts
│   └── sialia-migration-snapshot.ts
└── bench/
    ├── hook-cold-start.bench.ts
    ├── fts5-query.bench.ts
    ├── mcp-roundtrip.bench.ts
    └── regression-check.ts
```

### CI workflow

```yaml
# .github/workflows/ci.yml
name: CI
on: [push, pull_request]
jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v1
      - run: bun install
      - run: bunx eslint src/ --max-warnings=0
      - run: bunx tsc --noEmit --strict -p src/tsconfig.json
      - run: bun test --coverage
      - run: bun run bench
      - run: node tests/bench/regression-check.js
  windows:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - uses: oven-sh/setup-bun@v1
      - run: bun install
      - run: bun test src/runtime/  # validates bun:sqlite on Windows
```

### Upgrade doc structure

`docs/upgrade-to-2.0.md`:

```markdown
# Upgrading Mustard 1.x → 2.0

## What changes
- New: `.claude/.harness/mustard.db` (SQLite + FTS5)
- New: `.claude/.harness/spans.jsonl` (OpenTelemetry GenAI)
- New: MCP server `mustard-memory` auto-spawned
- Removed: `.pipeline-states/*.metrics.json`, `agentAttempts` field
- Changed: hooks consume EventStore (compat layer mantido por 1 release)

## Backup
[exact commands to snapshot .claude/]

## Upgrade steps
1. `mustard update --to=2.0`
2. Migration roda automaticamente no próximo SessionStart
3. Validate: `mustard verify`

## Rollback
[exact commands to restore from backup]
[doc explains compat shim is removed in 2.1]
```

### Benchmark baselines

Tracked em `tests/bench/baselines.json`:

```json
{
  "hook_cold_start_p95_ms": 30,
  "fts5_query_p95_ms": 5,
  "mcp_roundtrip_p95_ms": 10,
  "migration_1000_events_ms": 500,
  "dashboard_metrics_endpoint_p95_ms": 50
}
```

CI bench script fails se p95 regress >15% vs baseline.

## Risks

- **Bun on Windows CI flaky**: separar job windows, allow-failure no início, hard requirement quando estável
- **Coverage gating bloqueia merges legítimos**: ratchet (start 70%, sobe pra 80% gradualmente)
- **Migration de projeto enorme demora**: progress reporter no script; AC já cobre 1348 events em <500ms

## Out of scope

- E2E test rodando Claude Code real (futuro, requer Anthropic test harness)
- Performance regression alerting em produção (só CI)

## Checklist

- [ ] Test suite estruturada em `tests/{unit,integration,bench}`
- [ ] CI workflow `.github/workflows/ci.yml`
- [ ] ESLint config strict
- [ ] tsconfig strict mode
- [ ] Coverage threshold gate
- [ ] Benchmark baselines + regression check
- [ ] Upgrade doc + Rollback doc
- [ ] sialia migration smoke test passa
- [ ] CHANGELOG.md atualizado
