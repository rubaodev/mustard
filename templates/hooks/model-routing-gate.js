#!/usr/bin/env node
'use strict';
/**
 * MODEL-ROUTING-GATE: PreToolUse hook that validates the model selected for
 * Task/Agent dispatches against the pipeline's model routing table.
 *
 * Routing table:
 *   Explore agents          → haiku
 *   Bugfix pipeline         → sonnet
 *   Feature light (≤5 files)→ sonnet
 *   Feature full (5+ files) → opus
 *   Audit/review tasks      → sonnet
 *   Default (no pipeline)   → sonnet
 *
 * Upgrades are blocked (e.g., expected sonnet but got opus).
 * Downgrades are allowed (saving money is fine).
 *
 * MODE (MUSTARD_MODEL_GATE_MODE env var):
 *   warn   — advisory additionalContext, always allow  (DEFAULT)
 *   strict — deny with reason on upgrade violations
 *   off    — completely skip all checks
 *
 * Fail-open: exits 0 on any error — never blocks due to hook bugs.
 *
 * @version 1.0.0
 */

const fs   = require('fs');
const path = require('path');
const { shouldRun } = require('./_lib/hook-env.js');
const { emitMetric } = require('./_lib/metrics-emit.js');

// ── Cost rank: higher = more expensive ──────────────────────────────────────
const MODEL_COST_RANK = { haiku: 1, sonnet: 2, opus: 3 };

/**
 * Normalise a raw model string to one of the rank keys, or null if unknown.
 * Handles strings like "claude-3-haiku-20240307", "claude-sonnet-4-5", "opus", etc.
 * @param {string} raw
 * @returns {'haiku'|'sonnet'|'opus'|null}
 */
function normalizeModel(raw) {
  const s = (raw || '').toLowerCase();
  if (s.includes('haiku'))  return 'haiku';
  if (s.includes('opus'))   return 'opus';
  if (s.includes('sonnet')) return 'sonnet';
  return null;
}

/**
 * Find the newest .json pipeline-state file (excludes .metrics.json).
 * Returns parsed state object or null.
 * @param {string} projectDir
 * @returns {object|null}
 */
function loadNewestPipelineState(projectDir) {
  try {
    const statesDir = path.join(projectDir, '.claude', '.pipeline-states');
    if (!fs.existsSync(statesDir)) return null;

    const files = fs.readdirSync(statesDir)
      .filter(f => f.endsWith('.json') && !f.endsWith('.metrics.json'));
    if (files.length === 0) return null;

    // Sort by mtime descending, pick newest
    const sorted = files
      .map(f => {
        try {
          const fp = path.join(statesDir, f);
          return { f, mtime: fs.statSync(fp).mtimeMs, fp };
        } catch (_) {
          return null;
        }
      })
      .filter(Boolean)
      .sort((a, b) => b.mtime - a.mtime);

    if (sorted.length === 0) return null;

    const content = fs.readFileSync(sorted[0].fp, 'utf8');
    return JSON.parse(content);
  } catch (_) {
    return null;
  }
}

/**
 * Determine the expected model and the human-readable reason.
 * @param {string} subagentType  e.g. 'Explore', 'general-purpose', 'Plan', 'Bash'
 * @param {string} description   Task description text
 * @param {object|null} state    Parsed pipeline state (or null)
 * @returns {{ expected: string, reason: string }}
 */
function determineExpected(subagentType, description, state) {
  const desc = (description || '').toLowerCase();

  // Rule 1: Explore always uses haiku
  if ((subagentType || '').toLowerCase() === 'explore') {
    return { expected: 'haiku', reason: 'Explore agents use haiku' };
  }

  // Rule 2: Active pipeline type drives the choice
  if (state && state.type) {
    const pipelineType = (state.type || '').toLowerCase();
    const scope        = (state.scope || '').toLowerCase();

    // Audit/review description overrides pipeline scope
    if (desc.includes('audit') || desc.includes('review')) {
      return {
        expected: 'sonnet',
        reason: 'Audit/review tasks use sonnet',
      };
    }

    if (pipelineType === 'bugfix') {
      return { expected: 'sonnet', reason: 'Bugfix pipelines use sonnet' };
    }

    if (pipelineType === 'feature') {
      if (scope === 'full') {
        return {
          expected: 'opus',
          reason: 'Feature full scope (5+ files / new patterns) uses opus',
        };
      }
      if (scope === 'light') {
        return {
          expected: 'sonnet',
          reason: 'Feature light scope (≤5 files, known patterns) uses sonnet',
        };
      }
      // Feature with unknown scope → sonnet default
      return {
        expected: 'sonnet',
        reason: 'Feature pipeline (unknown scope) defaults to sonnet',
      };
    }
  }

  // Rule 3: Description hints with no pipeline
  if (desc.includes('audit') || desc.includes('review')) {
    return { expected: 'sonnet', reason: 'Audit/review tasks use sonnet' };
  }

  // Default fallback
  return { expected: 'sonnet', reason: 'Default model for tasks with no active pipeline' };
}

// ── Mode resolution ──────────────────────────────────────────────────────────
function getMode() {
  const raw = (process.env.MUSTARD_MODEL_GATE_MODE || 'warn').toLowerCase();
  if (raw === 'strict' || raw === 'off' || raw === 'warn') return raw;
  return 'warn';
}

// ── Main ─────────────────────────────────────────────────────────────────────
let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => (input += chunk));
process.stdin.on('end', () => {
  try {
    if (!shouldRun('model-routing-gate')) { process.exit(0); }

    const data       = JSON.parse(input);
    const toolName   = data.tool_name || '';

    // Only act on Task or Agent tool dispatches
    if (toolName !== 'Task' && toolName !== 'Agent') { process.exit(0); }

    const toolInput    = data.tool_input    || {};
    const rawModel     = toolInput.model    || '';
    const subagentType = toolInput.subagent_type || '';
    const description  = toolInput.description  || '';
    const projectDir   = process.env.CLAUDE_PROJECT_DIR || data.cwd || process.cwd();

    // If no model specified Claude uses its own default — nothing to validate
    if (!rawModel) { process.exit(0); }

    const model = normalizeModel(rawModel);
    if (!model) { process.exit(0); } // Unknown model name — can't rank, skip

    const mode = getMode();
    if (mode === 'off') { process.exit(0); }

    // Resolve expected model
    const state              = loadNewestPipelineState(projectDir);
    const { expected, reason } = determineExpected(subagentType, description, state);

    const modelRank    = MODEL_COST_RANK[model]    || 2; // unknown → sonnet tier
    const expectedRank = MODEL_COST_RANK[expected] || 2;

    const isViolation = modelRank > expectedRank;
    const noteLabel   = isViolation ? 'violation' : 'passed';

    // Emit metric on every gate check
    emitMetric('model-routing-gate', {
      tokensAffected: 0,
      tokensSaved: isViolation
        ? estimateSavings(model, expected)
        : 0,
      note: noteLabel,
      extras: {
        expected,
        actual: model,
        pipeline_type: state ? (state.type || 'unknown') : 'none',
        scope:         state ? (state.scope || 'unknown') : 'none',
        reason,
        mode,
        subagent_type: subagentType,
      },
    });

    if (!isViolation) { process.exit(0); }

    // ── Gate violation ───────────────────────────────────────────────────────
    if (mode === 'warn') {
      process.stdout.write(JSON.stringify({
        hookSpecificOutput: {
          hookEventName: 'PreToolUse',
          additionalContext:
            `[Model Gate] Expected ${expected} for this task (${reason}). ` +
            `Consider using model: '${expected}' to reduce costs.`,
        },
      }) + '\n');
      process.exit(0);
    }

    // mode === 'strict'
    process.stdout.write(JSON.stringify({
      permissionDecision: 'deny',
      permissionDecisionReason:
        `[Model Gate] Task requires '${expected}' model, not '${model}'. ` +
        `Reason: ${reason}. Re-dispatch with model: '${expected}'.`,
    }) + '\n');
    process.exit(0);

  } catch (err) {
    process.stderr.write('[model-routing-gate] ' + err.message + '\n');
    process.exit(0); // fail-open
  }
});

/**
 * Rough token-savings estimate when an upgrade violation occurs.
 * Based on approximate cost ratio differences (not exact — advisory only).
 * @param {string} actual    The requested (more expensive) model key
 * @param {string} expected  The recommended (cheaper) model key
 * @returns {number}
 */
function estimateSavings(actual, expected) {
  // Very rough: each tier step saves ~1000 tokens worth of API cost equivalent
  const diff = (MODEL_COST_RANK[actual] || 2) - (MODEL_COST_RANK[expected] || 2);
  return Math.max(0, diff * 1000);
}
