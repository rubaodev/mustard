# Mustard Memory MCP Server

`mustard-memory` is the local MCP (Model Context Protocol) server that
exposes Mustard's EventStore + KnowledgeBase as a stable, read-only tool
surface. Any agent that speaks MCP — Claude Code, Cursor, Aider — can query
past learnings, events, specs, metrics, and spans without re-implementing
JSONL parsing or `pipeline-state.json` aggregation.

- Runtime: Bun (uses `bun:sqlite` via the EventStore)
- Transport: stdio (line-delimited JSON-RPC 2.0)
- DB path: `MUSTARD_DB_PATH` env var (defaults to
  `<cwd>/.claude/.harness/mustard.db`)
- Phase: Mustard 2.0 — Phase 3

## Architecture

```
Claude Code (host)  ──spawn──▶  bun dist/mcp/mustard-memory.js
       │                              │
       │  JSON-RPC 2.0 over stdio     │
       │  (initialize, tools/list,    │
       │   tools/call)                ▼
       └──────────────────▶  EventStore (bun:sqlite, WAL)
                                       │
                                       ▼
                            .claude/.harness/mustard.db
```

Writes never go through MCP. Hooks (`PreToolUse`, `PostToolUse`,
`SessionEnd`) own all write paths — that's where session/wave/spec context
is authentic. Exposing writes via MCP would open injection attacks from
adversarial tool callers.

## Configuration

`templates/settings.json` registers the server under `mcpServers`:

```json
{
  "mcpServers": {
    "mustard-memory": {
      "command": "bun",
      "args": ["dist/mcp/mustard-memory.js"],
      "env": { "MUSTARD_DB_PATH": ".claude/.harness/mustard.db" }
    }
  }
}
```

> **Path note**: `args` is relative to the cwd Claude Code launches the
> server with (typically the project root). In a working Mustard tree this
> resolves to `<repo>/dist/mcp/mustard-memory.js`. When Mustard is
> installed as an npm dependency, point `args` at
> `node_modules/mustard-claude/dist/mcp/mustard-memory.js`.

## Tools

All tools return `{ content: [{ type: 'text', text: '<JSON>' }] }`. Parse
`content[0].text` to recover structured results.

### 1. `search_knowledge`

Full-text-style search over `knowledge` rows.

| Field | Type | Default |
|---|---|---|
| `query` | string (≥1 char) | — |
| `type` | `pattern` \| `convention` \| `entity` | optional |
| `limit` | int 1–50 | 10 |

Returns an array of `KnowledgeRecord`. Current implementation does
in-process substring filtering against `name + description`; a future
swap to FTS5 will keep the signature stable.

### 2. `query_events`

Filter events by spec/event/since.

| Field | Type | Default |
|---|---|---|
| `spec` | string | optional |
| `event` | string | optional |
| `since` | ISO timestamp | optional |
| `limit` | int 1–500 | 100 |

Returns an array of `EventRecord` (`{ ts, sessionId, wave, spec, event,
actor, payload }`).

### 3. `find_similar_specs`

Rank specs by token overlap (name + phase + affectedFiles) against a
free-text description.

| Field | Type | Default |
|---|---|---|
| `description` | string (≥1 char) | — |
| `limit` | int 1–20 | 5 |

Returns `[{ spec: SpecRecord, score: number }]` sorted by score desc.

### 4. `get_spec_metrics`

Fetch a single spec's projection from `metrics_projection`.

| Field | Type | Default |
|---|---|---|
| `spec` | string (≥1 char) | — |

Returns the `MetricsRecord` or `{ error: 'no metrics for spec', spec }`.

### 5. `get_span_summary`

Aggregate token/duration totals across `spans` (Phase 2 telemetry),
grouped by model.

| Field | Type | Default |
|---|---|---|
| `spec` | string | optional |
| `phase` | string | optional |
| `limit` | int 1–5000 | 1000 |

Returns
`{ count, totalInputTokens, totalOutputTokens, totalDurationMs, byModel }`.

## Consuming from agents

### Claude Code

Once `mcpServers.mustard-memory` is in `settings.json`, Claude Code spawns
the server at session start. Tools surface as
`mcp__mustard_memory__<tool_name>` in the available toolset (the exact
prefix depends on the host's tool naming convention).

### Cursor / Aider / generic clients

Point the client at `bun dist/mcp/mustard-memory.js` with
`MUSTARD_DB_PATH` set to the target project's `.harness/mustard.db`. Any
spec-compliant MCP client works — there is no Mustard-specific protocol.

## Limitations (current phase)

- **Read-only.** No write tools exposed. Phase 4 may add controlled writes
  with schema validation.
- **Single project.** One DB per server instance. Multi-project federation
  is a future phase.
- **No semantic search.** Knowledge search is substring-based; vector
  search is a future phase.
- **Bun required.** Server depends on `bun:sqlite`. A Node fallback via
  `better-sqlite3` is on the Phase 1 roadmap.

## Dashboard

The local JS dashboard (`templates/scripts/dashboard*.js`) was removed in
this release. Visualization moves to the standalone `mustard-dashboard`
(Tauri desktop app), distributed separately and consuming the same MCP
server documented here. See spec
`mustard-dashboard-1-0-standalone-tauri` for the migration context.

## Testing

The integration tests under `tests/integration/mcp-*.cjs` spawn the
server as a child process, perform the handshake, and exercise each
tool. Run them with:

```bash
node tests/integration/mcp-search-knowledge.cjs
node tests/integration/mcp-query-events.cjs
node tests/integration/mcp-similar-specs.cjs
node tests/integration/mcp-latency.cjs       # p95 <10ms over 100 calls
node tests/integration/mcp-sandbox.cjs       # asserts read-only
```

Tests seed test data via `tests/integration/mcp-seed.mjs` rather than the
JSONL→SQLite migration, to sidestep a pre-existing FTS5 external-content
issue on Windows (the seeder writes the `knowledge` base table directly
and skips `knowledge_fts`).
