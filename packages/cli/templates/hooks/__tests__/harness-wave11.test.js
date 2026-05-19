#!/usr/bin/env bun
'use strict';
/**
 * Harness Wave 11 — Slope report projection tests
 *
 * Covers:
 * 1.  buildSlopeReport: counts warns correctly across events
 *
 * Run with: bun test templates/hooks/__tests__/harness-wave11.test.js
 */

const { describe, it } = require('bun:test');
const assert = require('node:assert/strict');

// ── Helpers ───────────────────────────────────────────────────────────────────

function makeHarnessEvent(eventName, payload, overrides = {}) {
  return Object.assign({
    v: 1,
    ts: new Date().toISOString(),
    sessionId: 's-test',
    wave: 0,
    actor: { kind: 'hook' },
    event: eventName,
    payload,
  }, overrides);
}

// ── buildSlopeReport counts warns correctly ──────────────────────────────────

describe('Wave 11 — buildSlopeReport: counts anti-slope warns correctly', () => {
  it('counts duplication.warn and convention.warn from events', () => {
    const views = require('../../scripts/event-projections.js');

    const events = [
      makeHarnessEvent('duplication.warn', { file: 'src/a.ts', symbols: ['AuthServices'] }),
      makeHarnessEvent('duplication.warn', { file: 'src/b.ts', symbols: ['UserServices'] }),
      makeHarnessEvent('convention.warn', { file: 'src/c.ts', violations: [] }),
      makeHarnessEvent('agent.start', { description: 'not a slope event' }),
    ];

    const report = views.buildSlopeReport(events, { lookback_sessions: 1 });

    assert.equal(report.duplication, 2, `expected 2 duplication warns, got: ${report.duplication}`);
    assert.equal(report.convention, 1, `expected 1 convention warn, got: ${report.convention}`);
    assert.ok(Array.isArray(report.top_paths), 'top_paths must be array');
  });

  it('returns zeros when no slope events present', () => {
    const views = require('../../scripts/event-projections.js');

    const events = [
      makeHarnessEvent('agent.start', {}),
      makeHarnessEvent('tool.use', {}),
    ];

    const report = views.buildSlopeReport(events, { lookback_sessions: 1 });

    assert.equal(report.duplication, 0);
    assert.equal(report.convention, 0);
    assert.deepEqual(report.top_paths, []);
  });
});
