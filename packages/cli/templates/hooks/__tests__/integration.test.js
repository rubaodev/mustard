#!/usr/bin/env bun
/**
 * Integration tests for Mustard hooks — cross-hook interaction scenarios.
 * Adds to the existing 26 unit tests without modifying hooks.test.js.
 * Run with: bun test templates/hooks/__tests__/
 */

'use strict';

const { describe, it, beforeAll, afterAll } = require('bun:test');
const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const path = require('node:path');
const fs = require('node:fs');
const os = require('node:os');

const HOOKS_DIR = path.resolve(__dirname, '..');
const PROJECT_DIR = path.resolve(__dirname, '..', '..', '..');

function runHook(hookFile, inputObj, opts = {}) {
  return new Promise((resolve, reject) => {
    const cwd = opts.cwd || PROJECT_DIR;
    const child = spawn(process.execPath, [path.join(HOOKS_DIR, hookFile)], {
      cwd,
      env: {
        ...process.env,
        CLAUDE_PROJECT_DIR: opts.projectDir || PROJECT_DIR,
        CONTEXT_BUDGET_MODE: opts.budgetMode || 'strict',
      },
      stdio: ['pipe', 'pipe', 'pipe'],
    });

    let stdout = '';
    let stderr = '';
    child.stdout.on('data', (d) => (stdout += d));
    child.stderr.on('data', (d) => (stderr += d));
    child.on('error', reject);
    child.on('close', (code) => {
      let parsed = null;
      if (stdout.trim()) {
        try { parsed = JSON.parse(stdout.trim()); } catch { /* not JSON */ }
      }
      resolve({ code, stdout: stdout.trim(), stderr: stderr.trim(), parsed });
    });

    const input = typeof inputObj === 'string' ? inputObj : JSON.stringify(inputObj);
    child.stdin.write(input);
    child.stdin.end();
  });
}

// ─── Suite 1: Fail-open — malformed stdin ────────────────────────────────────

describe('Suite 1: fail-open on malformed input', () => {
  const malformed = ['', 'not-json', '{"tool_name":}', '{}', '{"x":1}'];

  // context-budget.js and subagent-tracker.js were ported to the Rust
  // `mustard-rt` modules (`budget` / `tracker`) in b3 Wave 3 — their fail-open
  // parity now lives in packages/rt/src/hooks/{budget,tracker}.rs.
  for (const hook of ['pre-compact.js']) {
    for (const bad of malformed) {
      it(`${hook} exits 0 on malformed stdin: ${JSON.stringify(bad).slice(0, 30)}`, async () => {
        const result = await runHook(hook, bad);
        assert.equal(result.code, 0, `Expected exit 0, got ${result.code}. stderr: ${result.stderr}`);
      });
    }
  }

  // spec-hygiene reads from cwd fs only — stdin is largely unused, still test empty
  it('spec-hygiene.js exits 0 on empty stdin', async () => {
    const result = await runHook('spec-hygiene.js', '');
    assert.equal(result.code, 0);
  });
});

// ─── Suite 3: spec-hygiene classification ────────────────────────────────────

describe('Suite 3: spec-hygiene classification', () => {
  let tmpDir;

  beforeAll(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mustard-hygiene-'));
  });

  afterAll(() => {
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch (_) {}
  });

  function makeSpec(name, content) {
    const specDir = path.join(tmpDir, '.claude', 'spec', 'active', name);
    fs.mkdirSync(specDir, { recursive: true });
    fs.writeFileSync(path.join(specDir, 'spec.md'), content, 'utf8');
    return specDir;
  }

  it('Status: completed + all [x] → spec moved to completed (auto-move)', async () => {
    makeSpec('spec-done', [
      '### Status: completed | Phase: CLOSE',
      '## Checklist',
      '- [x] Step one',
      '- [x] Step two',
    ].join('\n'));

    const r = await runHook('spec-hygiene.js', '{}', { cwd: tmpDir, projectDir: tmpDir });
    assert.equal(r.code, 0);
    const completedDir = path.join(tmpDir, '.claude', 'spec', 'completed', 'spec-done');
    assert.ok(fs.existsSync(completedDir), 'Spec should have been moved to completed/');
  });

  it('Status: implementing + all [x] → warn only (not moved)', async () => {
    makeSpec('spec-implementing', [
      '### Status: implementing | Phase: EXECUTE',
      '## Checklist',
      '- [x] Step one',
      '- [x] Step two',
    ].join('\n'));

    const r = await runHook('spec-hygiene.js', '{}', { cwd: tmpDir, projectDir: tmpDir });
    assert.equal(r.code, 0);
    const activeDir = path.join(tmpDir, '.claude', 'spec', 'active', 'spec-implementing');
    assert.ok(fs.existsSync(activeDir), 'Spec should remain in active/ (warn only)');
    assert.ok(r.stderr.includes('implementing') || r.stderr.includes('complete'), 'Should warn in stderr');
  });

  it('Status: draft + partial [ ] → silent (no move, no warn)', async () => {
    makeSpec('spec-draft', [
      '### Status: draft | Phase: ANALYZE',
      '## Checklist',
      '- [x] Step one',
      '- [ ] Step two pending',
    ].join('\n'));

    const r = await runHook('spec-hygiene.js', '{}', { cwd: tmpDir, projectDir: tmpDir });
    assert.equal(r.code, 0);
    const activeDir = path.join(tmpDir, '.claude', 'spec', 'active', 'spec-draft');
    assert.ok(fs.existsSync(activeDir), 'Draft spec should remain untouched');
  });

  it('Spec with ## Concerns BLOCKED → silent even if completed', async () => {
    makeSpec('spec-blocked', [
      '### Status: completed | Phase: CLOSE',
      '## Concerns',
      'BLOCKED: waiting for API approval',
      '## Checklist',
      '- [x] Step one',
    ].join('\n'));

    const r = await runHook('spec-hygiene.js', '{}', { cwd: tmpDir, projectDir: tmpDir });
    assert.equal(r.code, 0);
    const activeDir = path.join(tmpDir, '.claude', 'spec', 'active', 'spec-blocked');
    assert.ok(fs.existsSync(activeDir), 'Blocked spec should not be moved');
  });

  it('Spec without ## Checklist section → silent (defensive)', async () => {
    makeSpec('spec-no-checklist', [
      '### Status: completed | Phase: CLOSE',
      '## Summary',
      'Some feature',
    ].join('\n'));

    const r = await runHook('spec-hygiene.js', '{}', { cwd: tmpDir, projectDir: tmpDir });
    assert.equal(r.code, 0);
    const activeDir = path.join(tmpDir, '.claude', 'spec', 'active', 'spec-no-checklist');
    // No checklist → total=0 → allDone=false → silent (not moved)
    assert.ok(fs.existsSync(activeDir), 'No-checklist spec should not be moved');
  });
});

// ─── Suite 4: Hook sequence (simulated session) ──────────────────────────────

describe('Suite 4: hook sequence (simulated session)', () => {
  it('SessionStart → spec-hygiene.js exits 0', async () => {
    const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'mustard-seq-'));
    try {
      const r = await runHook('spec-hygiene.js', JSON.stringify({ hook_event_name: 'SessionStart' }), {
        cwd: tmpDir, projectDir: tmpDir,
      });
      assert.equal(r.code, 0);
    } finally {
      fs.rmSync(tmpDir, { recursive: true, force: true });
    }
  });

  // The PreToolUse(Task) / PostToolUse(Task) / shared-state cases below this
  // point exercised context-budget.js and subagent-tracker.js, both ported to
  // the Rust `mustard-rt` modules (`budget` / `tracker`) in b3 Wave 3. Their
  // parity now lives in the Rust test suites — see
  // packages/rt/src/hooks/{budget,tracker}.rs.
});
