#!/usr/bin/env bun
'use strict';
/**
 * emit-retry — record a `retry.attempt` event into the harness log.
 *
 * Problem this solves: the dashboard's `Quality.tsx` counts events of type
 * `retry.attempt` to fill the RETRIES column — but NOBODY ever emitted that
 * event, so the column was hard-stuck at `0`. The retry signal *does* exist:
 * `session-knowledge.js` reads `metrics.retries` from pipeline-state files and
 * writes `high-hook-retry-*` knowledge entries with the text "Pipeline
 * triggered N hook-level retries". That count never became a consumable event.
 *
 * This script lets any caller (a SKILL, or `session-knowledge.js` itself)
 * emit the marker explicitly:
 *   bun .claude/scripts/emit-retry.js --spec add-login --wave 4 --reason sandbox
 *
 * The emitted event is consumed by the dashboard Quality view and metrics:
 *   event:   retry.attempt
 *   payload: { reason, tool }
 *   spec:    {spec}
 *
 * Cross-shell: no inline `bun -e` quoting. Fail-open: any internal error
 * exits 0 without emitting (telemetry must never break a pipeline).
 *
 * Exit codes:
 *   0  emitted, or fail-silent on internal error / missing harness
 *   1  bad CLI arguments
 */

const fs = require('node:fs');
const path = require('node:path');

function parseArgs(argv) {
  const out = { spec: null, wave: null, reason: null, tool: null };
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i];
    const next = argv[i + 1];
    switch (flag) {
      case '--spec':
        out.spec = next; i++; break;
      case '--wave':
        out.wave = next; i++; break;
      case '--reason':
        out.reason = next; i++; break;
      case '--tool':
        out.tool = next; i++; break;
      case '-h':
      case '--help':
        printHelp();
        process.exit(0);
        break;
      default:
        // ignore unknown flags rather than failing — fail-silent ethos
        break;
    }
  }
  return out;
}

function printHelp() {
  process.stdout.write(`emit-retry — record a retry.attempt event.

Usage:
  bun emit-retry.js --spec <name> [--wave <N>] [--reason <text>] [--tool <name>]

  --spec NAME    spec identifier (required)
  --wave N       wave number (optional; harness infers from index.json if omitted)
  --reason TEXT  why the retry happened, e.g. sandbox, stash-pop (optional)
  --tool NAME    tool involved in the retry, if known (optional)

Exit: 0 on emit/silent-error, 1 on bad args.
`);
}

function resolveProjectDir() {
  if (process.env.CLAUDE_PROJECT_DIR) return process.env.CLAUDE_PROJECT_DIR;
  // Heuristic: script sits at .claude/scripts/, two levels up is project root.
  return path.resolve(__dirname, '..', '..');
}

function loadHarness(projectDir) {
  const harnessLib = path.join(projectDir, '.claude', 'hooks', '_lib', 'harness-event.js');
  if (!fs.existsSync(harnessLib)) return null;
  try {
    return require(harnessLib);
  } catch (_) {
    return null;
  }
}

function main() {
  const args = parseArgs(process.argv.slice(2));

  if (!args.spec) {
    process.stderr.write('error: --spec required\n');
    printHelp();
    process.exit(1);
  }

  const projectDir = resolveProjectDir();
  const harness = loadHarness(projectDir);
  if (!harness) {
    // Fail-silent: harness not installed yet. This is OK during bootstrap.
    process.exit(0);
  }

  const ctx = {
    cwd: projectDir,
    spec: args.spec,
    actor: { kind: 'orchestrator', id: 'emit-retry' },
  };
  // --wave is optional; only override the harness's inference when numeric.
  const waveNum = parseInt(args.wave, 10);
  if (Number.isFinite(waveNum)) ctx.wave = waveNum;

  harness.emit('retry.attempt', {
    reason: args.reason || null,
    tool: args.tool || null,
  }, ctx);
  process.exit(0);
}

main();
