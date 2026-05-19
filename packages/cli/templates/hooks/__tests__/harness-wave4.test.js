#!/usr/bin/env bun
'use strict';
/**
 * Harness Wave 4 — Subtraction Tests
 *
 * Verifies that legacy stores are NO LONGER written:
 * 1. subagent-tracker does NOT create .agent-memory/_index.json
 * 2. subagent-tracker does NOT create .agent-state/_queue.json
 * 3. subagent-tracker does NOT create .agent-state/{id}.json
 * 4. metrics-tracker does NOT create .pipeline-states/*.metrics.json
 * 5. buildPipelineState derived from log contains metrics (tool counts, agent count)
 *
 * Run with: bun test templates/hooks/__tests__/harness-wave4.test.js
 */

const { describe, it, beforeEach, afterEach } = require('bun:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawn } = require('node:child_process');

const HOOKS_DIR = path.resolve(__dirname, '..');
const SCRIPTS_DIR = path.resolve(__dirname, '..', '..', 'scripts');

// ── Helpers ───────────────────────────────────────────────────────────────────

function runHook(hookFile, inputObj, opts = {}) {
  return new Promise((resolve, reject) => {
    const projectDir = opts.projectDir || os.tmpdir();
    const env = {
      ...process.env,
      MUSTARD_DISABLED_HOOKS: opts.disabledHooks || '',
    };

    const child = spawn(process.execPath, [path.join(HOOKS_DIR, hookFile)], {
      cwd: projectDir,
      env,
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (d) => (stdout += d));
    child.stderr.on('data', (d) => (stderr += d));
    child.on('error', reject);
    child.on('close', (code) => {
      let parsed = null;
      try { parsed = JSON.parse(stdout.trim()); } catch (_) {}
      resolve({ code, stdout: stdout.trim(), stderr: stderr.trim(), parsed });
    });
    child.stdin.write(JSON.stringify(inputObj));
    child.stdin.end();
  });
}

/** Create a minimal project dir with harness + pipeline-states dirs. */
function makeProjectDir(base) {
  const dir = fs.mkdtempSync(path.join(base, 'mustard-w4-'));
  fs.mkdirSync(path.join(dir, '.claude', '.harness'), { recursive: true });
  return dir;
}

/** Read events.jsonl into array */
function readEvents(projectDir) {
  const evFile = path.join(projectDir, '.claude', '.harness', 'events.jsonl');
  if (!fs.existsSync(evFile)) return [];
  return fs.readFileSync(evFile, 'utf8')
    .split('\n').filter(Boolean)
    .map(l => { try { return JSON.parse(l); } catch { return null; } })
    .filter(Boolean);
}

// NOTE: subagent-tracker.js and metrics-tracker.js were ported to the Rust
// `mustard-rt` `tracker` module in b3 Wave 3. Their subtraction behavior
// (no .agent-memory / .agent-state / .metrics.json sidecar writes) parity now
// lives in packages/rt/src/hooks/tracker.rs. The buildPipelineState test below
// stays — it exercises event-projections.js, which was NOT ported.

// ── Test 4: buildPipelineState derives metrics from log ───────────────────────

describe('Wave 4 — buildPipelineState: metrics from log', () => {
  const harnessViews = require(path.join(SCRIPTS_DIR, 'event-projections.js'));

  it('aggregates tool.use counts and agent count from events', () => {
    const now = new Date().toISOString();
    const events = [
      { v: 1, ts: now, sessionId: 's1', wave: 1, spec: 'add-login', event: 'pipeline.phase', payload: { from: null, to: 'ANALYZE' }, actor: { kind: 'hook' } },
      { v: 1, ts: now, sessionId: 's1', wave: 1, spec: 'add-login', event: 'agent.start', payload: { description: 'Explore codebase', model: null }, actor: { kind: 'agent', id: 'ag-1', type: 'Explore' } },
      { v: 1, ts: now, sessionId: 's1', wave: 1, spec: 'add-login', event: 'tool.use', payload: { tool: 'Bash', phase: 'ANALYZE' }, actor: { kind: 'hook', id: 'metrics-tracker' } },
      { v: 1, ts: now, sessionId: 's1', wave: 1, spec: 'add-login', event: 'tool.use', payload: { tool: 'Edit', phase: 'ANALYZE' }, actor: { kind: 'hook', id: 'metrics-tracker' } },
      { v: 1, ts: now, sessionId: 's1', wave: 1, spec: 'add-login', event: 'tool.use', payload: { tool: 'Bash', phase: 'EXECUTE' }, actor: { kind: 'hook', id: 'metrics-tracker' } },
      // Retries are now counted from dispatch.failure events (real signal), not
      // from a keyword-derived `retry` flag on tool.use.
      { v: 1, ts: now, sessionId: 's1', wave: 1, spec: 'add-login', event: 'dispatch.failure', payload: { agentType: 'general-purpose', phase: 'EXECUTE' }, actor: { kind: 'hook', id: 'subagent-tracker' } },
      // Read events should NOT count toward apiCalls
      { v: 1, ts: now, sessionId: 's1', wave: 1, spec: 'add-login', event: 'tool.use', payload: { tool: 'Read', phase: 'ANALYZE' }, actor: { kind: 'hook', id: 'metrics-tracker' } },
      { v: 1, ts: now, sessionId: 's1', wave: 1, spec: 'other-spec', event: 'tool.use', payload: { tool: 'Write', phase: 'EXECUTE' }, actor: { kind: 'hook', id: 'metrics-tracker' } },
    ];

    const result = harnessViews.buildPipelineState(events, { spec: 'add-login' });

    assert.equal(result.spec, 'add-login');
    assert.equal(result.phase, 'ANALYZE');
    assert.ok(result.metrics, 'metrics object must be present');
    assert.equal(result.metrics.apiCalls, 3, 'Bash + Edit + Bash = 3 (Read excluded)');
    assert.equal(result.metrics.toolBreakdown.Bash, 2, 'Bash used twice');
    assert.equal(result.metrics.toolBreakdown.Edit, 1, 'Edit used once');
    assert.equal(result.metrics.retries, 1, 'One dispatch.failure event');
    assert.equal(result.metrics.dispatchFailuresByPhase.EXECUTE, 1, 'failure attributed to EXECUTE phase');
    assert.equal(result.metrics.agentCount, 1, 'One agent.start event');
    // other-spec tool.use should not bleed in
    assert.ok(!result.metrics.toolBreakdown.Write, 'Write from other-spec must not appear');
  });

  it('returns zero metrics when no tool.use events', () => {
    const events = [
      { v: 1, ts: new Date().toISOString(), sessionId: 's1', wave: 1, spec: 'empty-spec', event: 'pipeline.phase', payload: { from: null, to: 'ANALYZE' }, actor: {} },
    ];

    const result = harnessViews.buildPipelineState(events, { spec: 'empty-spec' });
    assert.equal(result.metrics.apiCalls, 0);
    assert.equal(result.metrics.retries, 0);
    assert.equal(result.metrics.agentCount, 0);
  });

  it('no-spec mode: aggregates all events regardless of spec', () => {
    const now = new Date().toISOString();
    const events = [
      { v: 1, ts: now, wave: 1, spec: 'spec-a', event: 'tool.use', payload: { tool: 'Edit' }, actor: {} },
      { v: 1, ts: now, wave: 1, spec: 'spec-b', event: 'tool.use', payload: { tool: 'Write' }, actor: {} },
    ];

    const result = harnessViews.buildPipelineState(events, {});
    assert.equal(result.metrics.apiCalls, 2, 'Both events counted when no spec filter');
  });
});
