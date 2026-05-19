# Changelog

## [Unreleased]

### Changed — command namespace cleanup

- Renamed the `/complete` command to `/close`, aligning it with the canonical `CLOSE` pipeline phase. The underlying `complete-spec.js` script keeps its name.
- Merged the metrics command into `/stats`: hook-level aggregation now lives behind `/stats --hooks`, with `--since`/`--event`/`--compare`/`--pr`/`--days` flags. The standalone command folder was removed.
- Moved `scan-format` and the `agent-prompt` template out of `commands/` into `refs/` — they are internal agent instructions, not user commands.
- Command surface reduced from 18 to 15 real slash commands. No pipeline behavior changed.

## [2.0.0] - 2026-05-12

Mustard 2.0 — Event Store + Telemetry + MCP + Hardening. Migration automática, zero breaking changes; veja `docs/upgrade-to-2.0.md` para passos de upgrade e rollback.

### Phase 0 — Runtime Compatibility Layer

- Add Bun runtime detection (`src/runtime/detect-runtime.ts`)
- Add `templates/hooks/_lib/runtime-shim.js` (CJS shim — pick Node/Bun)
- `mustard init --runtime=bun|node|auto` flag (default `auto`)
- `.claude/mustard.json` registra runtime escolhido
- Fix `mustard update --force` backup (estava pulando backup com `--force`)

### Phase 1 — Event Store SQLite + Projections

- Add `src/runtime/event-store.ts` (`EventStore` class com FTS5)
- Add `src/migrate/jsonl-to-sqlite.ts` (migration idempotente)
- Remove `.pipeline-states/*.metrics.json`, `.subagent-registry.json`, campo `agentAttempts`
- Dashboard reads via EventStore (fail-open para legacy `events.jsonl`)

### Phase 2 — OpenTelemetry + Token Tracking

- Add `src/telemetry/token-tracker.ts` (manual OTLP JSON emit, sem SDK)
- Spans armazenados em `.claude/.harness/spans.jsonl` + tabela `spans`
- Hook `subagent-tracker.js` emite spans Pre/Post via `toolUseId`
- Dashboard widget "Token Usage Real" (byPhase / byModel / byAgent / costUsd)
- Substituição do heurístico `tokensSaved` por spans medidos

### Phase 3 — MCP Memory Server

- Add `src/mcp/mustard-memory.ts` (5 read-only tools via SDK 1.29 `registerTool`)
- Tools: `search_knowledge`, `query_events`, `find_similar_specs`, `get_spec_metrics`, `get_span_summary`
- `templates/settings.json` registra `mcpServers.mustard-memory`
- Dashboard banner `@deprecated` (Tauri standalone planned)
- Docs: `docs/mcp-tools.md`

### Phase 4 — Hardening (Tests, Lint, Bench, CI, Docs)

#### Fixed (Phase 4 Wave 1 carryover)

- **knowledge_fts external-content rowid mismatch**: schema redeclared as
  standalone FTS5 with UNINDEXED `id` column (was `content='knowledge',
  content_rowid='id'` against TEXT id → "database disk image is malformed"
  on Windows). `EventStore.init()` self-heals pre-existing DBs by detecting
  the old declaration via `sqlite_master` and dropping before recreating.
  Migration validated idempotent on sialia (1787 events, 56 knowledge
  entries) across repeated runs.

#### Added

- `EventStore.knowledge({search})`: FTS5 MATCH com bm25 ranking joined back
  para `knowledge` via UNINDEXED `id` column. Backward-compatible —
  `{minConfidence, limit}` filters continuam funcionando.
- MCP `search_knowledge` tool agora usa `EventStore.knowledge({search})`
  em vez de substring filtering in-process (5x over-fetch para post-filter
  por type; mesmo input schema).
- Suite de testes em `tests/unit/{event-store,token-tracker,migrate,mcp}/` e
  `tests/integration/`. **91+ tests pass**, coverage **96.20% lines /
  95.48% funcs** em `src/{runtime,telemetry,migrate}`.
- ESLint v9 flat config (`eslint.config.js`) — **zero warnings** em `src/`.
- tsconfig `strict: true` + `noUncheckedIndexedAccess: true` — **zero
  `@ts-expect-error`** em `src/`.
- Benchmarks em `tests/bench/` (`hook-cold-start`, `fts5-query`,
  `mcp-roundtrip`) com `baselines.json` + `regression-check.cjs`.
  Medições Windows: fts5 ~1ms, mcp ~3ms, hook cold-start ~53ms.
- CI workflow `.github/workflows/ci.yml` — Linux (hard) + Windows
  (`continue-on-error: true` enquanto Bun on Windows estabiliza).
- Docs: `docs/upgrade-to-2.0.md` com seções **Backup** e **Rollback**
  explícitas + troubleshooting + migration timeline.

#### Removed

- `@opentelemetry/otlp-transformer` devDep: `ProtobufTraceSerializer.serializeRequest`
  accepts `ReadableSpan[]` from `@opentelemetry/sdk-trace-base`, mas
  Mustard emite OTLP/JSON direto sem SDK. Um round-trip clean exigiria
  adicionar a stack full do SDK (quebra o zero-deps contract dos hooks).
  Os testes de shape assertion-based já cobrem o contrato OTLP; a dep
  não justificou o lugar.

#### Follow-ups (não bloqueiam release)

- **CI verde no GitHub Actions** (spec AC #2): só validável após primeiro
  push para o repo remoto. Workflow file está presente e validado
  localmente; primeira run será reportada manualmente.
- **Sialia upgrade completo**: snapshot da Sialia ainda em Phase 1+2;
  validar `mustard update --force` end-to-end fica para release window.

## [3.0.0] - 2026-03-24

### Breaking Changes

- **CLI simplified**: `mustard init` now just copies templates (no scanning, no generation)
- **Removed Ollama**: No longer used — all intelligence lives in `/scan` skill inside Claude Code
- **Removed grepai**: No longer a dependency
- **Removed CLI flags**: `--ollama`, `--no-grepai`, `--verbose` removed from init/update
- **Removed old systems**: prompts/, context/, core/ directories no longer generated

### Removed

- `generators/commands.ts` — commands are now templates, not generated code
- `generators/hooks.ts` — hooks are now templates, not generated code
- `generators/prompts.ts` — prompt system eliminated
- `generators/claude-md-llm.ts` — Ollama generation removed
- `analyzers/llm.ts` — Ollama analysis removed
- `analyzers/semantic.ts` — grepai analysis removed
- `services/ollama.ts` — Ollama service removed
- `services/grepai.ts` — grepai service removed
- `scanners/` — all scanners removed (detection now done by `/scan` inside Claude Code)
- `templates/context/` — old compiled context system
- `templates/prompts/` — old prompt system
- `templates/core/` — old enforcement/pipeline docs
- `templates/commands/backend-*.md` — project-specific commands
- Dependencies: `ollama`, `glob`

### Changed

- **CLI is now a copier**: `mustard init` = copy `templates/` → `.claude/`, nothing more
- **CLI source**: from ~25 files to 5 files (`cli.ts`, `init.ts`, `update.ts`, `auto-update.ts`, `npm.ts`)
- **Version**: 2.0.14 → 3.0.0
- **Commands format**: flat `.md` files → subdirectories with `SKILL.md` (skill-creator standard)
- **Hooks**: 4 old generated hooks → 8 new template hooks (bash-safety, file-guard, enforce-registry, guard-verify, auto-format, pre-compact, session-cleanup, subagent-tracker)

### Added

- **14 pipeline skills** (SKILL.md format):
  - `feature`, `bugfix`, `approve`, `complete`, `resume` — pipeline lifecycle
  - `scan`, `scan-format` — codebase analysis
  - `git` — commit, push, merge, deploy (monorepo + single repo)
  - `maint` — deps, validate, sync
  - `task` — delegated analysis, audit, compare, review, refactor, docs
  - `knowledge` — notes, memory audit, reports
  - `skill` — install, create, list, remove, optimize, eval
  - `status` — consolidated status
  - `templates/agent-prompt` — agent dispatch template
- **6 bundled skills**: design-craft, react-best-practices, senior-architect, skill-creator, commit-workflow, pipeline-execution
- **8 enforcement hooks**: bash-safety, file-guard, enforce-registry, guard-verify, auto-format, pre-compact, session-cleanup, subagent-tracker
- **3 sync scripts**: sync-detect.js, sync-registry.js, statusline.js
- **pipeline-config.md**: agent dispatch configuration (populated by `/scan`)
- **settings.json**: full hook configuration with PreToolUse, PostToolUse, SessionStart, PreCompact, SessionEnd, SubagentStart, SubagentStop

---

## [2.0.14] - 2026-02-07

### Changed

- Last version with Ollama/grepai support
- Last version with code generation (scanners, analyzers, generators)

---

## [2.0.0] - 2026-02-05

### Added

- `mustard sync` command
- Auto-section markers for preserving user customizations
- Prompt merge functionality

---

## [1.8.0] - 2026-02-05

### Added

- **Mustard CLI** — initial framework-agnostic project setup
- Stack detection (.NET, React, Next.js, Python, Java, Go, Rust, ORMs)
- Monorepo support
- Semantic analysis via grepai
- LLM generation via Ollama

---

## [1.0.0] - 2025-12-01

### Added

- Initial framework
- Pipeline for features/bugfixes
- Rules L0-L5
- Basic commands
