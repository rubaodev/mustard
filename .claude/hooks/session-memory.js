#!/usr/bin/env node
'use strict';
/**
 * SESSION-MEMORY: Injects persistent memory into session context
 *
 * Loads three sources with priority: decisions > lessons > knowledge.
 * Knowledge entries are ranked by confidence × recency (not just "last N").
 *
 * @version 2.0.0
 */
const fs = require('fs');
const path = require('path');
const { shouldRun } = require('./_lib/hook-env.js');

const MAX_CHARS = 2000;
const KB_MIN_CONFIDENCE = 0.5;
const KB_MAX_ENTRIES = 5;

let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => input += chunk);
process.stdin.on('end', () => {
  try {
    if (!shouldRun('session-memory')) { process.exit(0); }
    const data = JSON.parse(input);
    const cwd = data.cwd || process.cwd();
    const claudeDir = path.join(cwd, '.claude');
    const memDir = path.join(claudeDir, 'memory');

    const parts = [];

    // Priority 1: Decisions (most actionable)
    const decisions = loadEntries(path.join(memDir, 'decisions.json'), 5);
    if (decisions.length > 0) {
      parts.push('## Recent Decisions');
      decisions.forEach(d => parts.push(`- [${d.source}] ${d.content}`));
    }

    // Priority 2: Lessons learned
    const lessons = loadEntries(path.join(memDir, 'lessons.json'), 5);
    if (lessons.length > 0) {
      parts.push('## Lessons Learned');
      lessons.forEach(l => parts.push(`- [${l.source}] ${l.content}`));
    }

    // Priority 3: Knowledge base (confidence × recency ranked)
    const kbEntries = loadKnowledge(path.join(claudeDir, 'knowledge.json'));
    if (kbEntries.length > 0) {
      parts.push('## Project Knowledge');
      kbEntries.forEach(e => parts.push(`- [${e.type}] ${e.name}: ${e.description}`));
    }

    if (parts.length > 0) {
      let context = parts.join('\n');
      if (context.length > MAX_CHARS) context = context.slice(0, MAX_CHARS) + '\n...truncated';

      console.log(JSON.stringify({
        hookSpecificOutput: {
          hookEventName: 'SessionStart',
          additionalContext: `[Persistent Memory]\n${context}`
        }
      }));
    }

    process.exit(0);
  } catch (err) {
    process.stderr.write(`[session-memory] Error: ${err.message}\n`);
    process.exit(0);
  }
});

function loadEntries(filePath, max) {
  try {
    if (!fs.existsSync(filePath)) return [];
    const data = JSON.parse(fs.readFileSync(filePath, 'utf8'));
    const entries = data.entries || [];
    return entries.slice(-max);
  } catch { return []; }
}

/**
 * Load knowledge entries filtered by confidence and ranked by confidence × recency.
 * Returns top KB_MAX_ENTRIES entries with confidence >= KB_MIN_CONFIDENCE.
 */
function loadKnowledge(kbPath) {
  try {
    if (!fs.existsSync(kbPath)) return [];
    const kb = JSON.parse(fs.readFileSync(kbPath, 'utf8'));
    const entries = kb.entries || [];
    if (entries.length === 0) return [];

    const now = Date.now();
    // Score: confidence × recency factor (newer = higher)
    // Recency: 1.0 for today, decays to 0.1 over 30 days
    const scored = entries
      .filter(e => (e.confidence || 0) >= KB_MIN_CONFIDENCE)
      .map(e => {
        const ageMs = now - new Date(e.updatedAt || e.createdAt || 0).getTime();
        const ageDays = ageMs / (24 * 60 * 60 * 1000);
        const recency = Math.max(0.1, 1.0 - (ageDays / 30) * 0.9);
        return { ...e, score: (e.confidence || 0) * recency };
      })
      .sort((a, b) => b.score - a.score);

    return scored.slice(0, KB_MAX_ENTRIES);
  } catch { return []; }
}
