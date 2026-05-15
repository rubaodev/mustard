#!/usr/bin/env bun
'use strict';
/**
 * SESSION-KNOWLEDGE-INC: Incremental knowledge extraction after each Task completion.
 * Event: PostToolUse (matcher: Task)
 * Fail-open: exit 0 on any error.
 *
 * .knowledge-seen.json schema:
 * {
 *   "_meta": {
 *     "recentExtractions": ["ISO timestamp", ...]   // rolling window for throttle
 *   },
 *   "<patternName>": <ISO timestamp>                 // idempotency: skip if < 24 h old
 * }
 *
 * @version 1.0.0
 */

const fs = require('fs');
const path = require('path');
const { execFileSync } = require('child_process');
const { guardedRun } = require('./_lib/hook-env.js');
const { extractPatternsFromStates, extractFrictionFromStates } = require('./_lib/knowledge-extract.js');

var THROTTLE_MAX = 3;          // max extractions per rolling hour
var THROTTLE_WINDOW_MS = 3600000; // 1 hour in ms
var IDEMPOTENCY_WINDOW_MS = 86400000; // 24 hours in ms

var input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', function (chunk) { input += chunk; });
process.stdin.on('end', function () {
  try {
    if (!guardedRun('session-knowledge-inc', null, 2)) { process.exit(0); }

    var data = JSON.parse(input);
    var cwd = data.cwd || process.cwd();
    var claudeDir = path.join(cwd, '.claude');
    var knowledgeScript = path.join(claudeDir, 'scripts', 'knowledge-update.js');

    // Bail if knowledge-update.js doesn't exist
    if (!fs.existsSync(knowledgeScript)) { process.exit(0); }

    var seenPath = path.join(claudeDir, '.knowledge-seen.json');
    var seen = readSeenFile(seenPath);

    // ── Throttle check ──────────────────────────────────────────────
    var now = Date.now();
    var recentExtractions = (seen._meta && seen._meta.recentExtractions) || [];
    // Prune entries outside the rolling window
    recentExtractions = recentExtractions.filter(function (ts) {
      return now - new Date(ts).getTime() < THROTTLE_WINDOW_MS;
    });
    if (recentExtractions.length >= THROTTLE_MAX) {
      process.exit(0); // throttled — silently skip
    }

    // ── Find most recently modified pipeline state ──────────────────
    var statesDir = path.join(claudeDir, '.pipeline-states');
    if (!fs.existsSync(statesDir)) { process.exit(0); }

    var stateFiles = fs.readdirSync(statesDir).filter(function (f) { return f.endsWith('.json'); });
    if (stateFiles.length === 0) { process.exit(0); }

    // Sort by mtime descending — pick the most recent
    var sorted = stateFiles.map(function (f) {
      var fp = path.join(statesDir, f);
      var mtime = 0;
      try { mtime = fs.statSync(fp).mtimeMs; } catch (e) {}
      return { file: f, mtime: mtime };
    }).sort(function (a, b) { return b.mtime - a.mtime; });

    var latestFile = sorted[0].file;
    var latestState;
    try {
      latestState = JSON.parse(fs.readFileSync(path.join(statesDir, latestFile), 'utf8'));
      latestState._file = latestFile.replace('.json', '');
    } catch (e) {
      process.exit(0);
    }

    // ── Friction telemetry → .claude/.metrics/friction.json ─────────
    // high-hook-retry / heavy-pipeline are atrito, not knowledge — they go
    // to friction.json (type: 'friction'), never to knowledge.json.
    saveFriction(extractFrictionFromStates([latestState]), claudeDir);

    // ── Extract patterns from the latest state ──────────────────────
    var candidates = extractPatternsFromStates([latestState]);
    if (candidates.length === 0) { process.exit(0); }

    // ── Idempotency filter ──────────────────────────────────────────
    var eligible = candidates.filter(function (p) {
      var lastSeen = seen[p.name];
      if (!lastSeen) return true;
      return now - new Date(lastSeen).getTime() >= IDEMPOTENCY_WINDOW_MS;
    });

    if (eligible.length === 0) { process.exit(0); }

    // ── Persist exactly one pattern ────────────────────────────────
    var toSave = eligible[0];
    try {
      execFileSync(process.execPath, [knowledgeScript], {
        input: JSON.stringify(Object.assign({ cwd: cwd }, toSave)),
        timeout: 3000,
        stdio: ['pipe', 'pipe', 'pipe'],
      });
    } catch (e) { /* fail-open */ }

    // ── Update .knowledge-seen.json ────────────────────────────────
    var nowIso = new Date(now).toISOString();
    seen[toSave.name] = nowIso;
    recentExtractions.push(nowIso);
    seen._meta = { recentExtractions: recentExtractions };
    writeSeenFile(seenPath, seen);

    process.exit(0);
  } catch (err) {
    process.stderr.write('[session-knowledge-inc] ' + err.message + '\n');
    process.exit(0); // fail-open
  }
});
process.stdin.resume();

// ── Helpers ─────────────────────────────────────────────────────────

/**
 * Persist friction telemetry to `.claude/.metrics/friction.json`.
 *
 * Friction (high hook-retry, heavy API usage) is measured atrito — telemetry,
 * not knowledge. Entries are keyed by `name`; re-running updates the existing
 * entry in place. No `occurrences` field — the honest count is `retryCount` /
 * `apiCalls`. Fail-open: any error is swallowed.
 *
 * @param {object[]} frictionEntries  output of extractFrictionFromStates
 * @param {string}   claudeDir        absolute path to the project .claude dir
 */
function saveFriction(frictionEntries, claudeDir) {
  try {
    if (!Array.isArray(frictionEntries) || frictionEntries.length === 0) return;

    var metricsDir = path.join(claudeDir, '.metrics');
    if (!fs.existsSync(metricsDir)) { fs.mkdirSync(metricsDir, { recursive: true }); }

    var frictionPath = path.join(metricsDir, 'friction.json');
    var store = { version: 1, entries: [] };
    try {
      if (fs.existsSync(frictionPath)) {
        store = JSON.parse(fs.readFileSync(frictionPath, 'utf8'));
        if (!Array.isArray(store.entries)) store.entries = [];
      }
    } catch (e) { store = { version: 1, entries: [] }; }

    var ts = new Date().toISOString();
    for (var i = 0; i < frictionEntries.length; i++) {
      var entry = frictionEntries[i];
      if (!entry || !entry.name) continue;
      var idx = -1;
      for (var j = 0; j < store.entries.length; j++) {
        if (store.entries[j] && store.entries[j].name === entry.name) { idx = j; break; }
      }
      var record = Object.assign({}, entry, { updatedAt: ts });
      if (idx >= 0) {
        record.createdAt = store.entries[idx].createdAt || ts;
        store.entries[idx] = record;
      } else {
        record.createdAt = ts;
        store.entries.push(record);
      }
    }

    store.entries.sort(function (a, b) {
      return new Date(b.updatedAt || 0) - new Date(a.updatedAt || 0);
    });
    store.entries = store.entries.slice(0, 100);

    fs.writeFileSync(frictionPath, JSON.stringify(store, null, 2), 'utf8');
  } catch (e) {} // fail-open
}

function readSeenFile(seenPath) {
  try {
    if (fs.existsSync(seenPath)) {
      return JSON.parse(fs.readFileSync(seenPath, 'utf8'));
    }
  } catch (e) {}
  return { _meta: { recentExtractions: [] } };
}

function writeSeenFile(seenPath, seen) {
  try {
    // Rotate if file exceeds 100KB (1 level only: .1)
    try {
      if (fs.existsSync(seenPath) && fs.statSync(seenPath).size > 100 * 1024) {
        fs.renameSync(seenPath, seenPath + '.1');
      }
    } catch (e) { /* fail-open: rotation error is non-fatal */ }
    fs.writeFileSync(seenPath, JSON.stringify(seen, null, 2), 'utf8');
  } catch (e) { /* fail-open */ }
}
