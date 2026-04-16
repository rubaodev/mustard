#!/usr/bin/env node
// metrics-report — aggregate enforcement metrics from .claude/.metrics/*.jsonl
// Usage: node metrics-report.js [--since <ISO>] [--event <type>]
'use strict';
const fs = require('fs');
const path = require('path');

const METRICS_DIR = path.join(process.cwd(), '.claude', '.metrics');

// Parse CLI args
const args = process.argv.slice(2);
let sinceFilter = null;
let eventFilter = null;
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--since' && args[i + 1]) sinceFilter = new Date(args[++i]);
  if (args[i] === '--event' && args[i + 1]) eventFilter = args[++i];
}

// Collect all .jsonl files
if (!fs.existsSync(METRICS_DIR)) {
  console.log('No metrics data yet');
  process.exit(0);
}

const files = fs.readdirSync(METRICS_DIR).filter(f => f.endsWith('.jsonl'));
if (files.length === 0) {
  console.log('No metrics data yet');
  process.exit(0);
}

// Aggregate: { event -> { count, tokensAffected, tokensSaved, notes: Set } }
const agg = {};

for (const file of files) {
  const filePath = path.join(METRICS_DIR, file);
  let content;
  try { content = fs.readFileSync(filePath, 'utf8'); } catch (_) { continue; }
  for (const raw of content.split('\n')) {
    const line = raw.trim();
    if (!line) continue;
    let entry;
    try { entry = JSON.parse(line); } catch (_) { continue; } // skip malformed
    if (!entry.event) continue;
    if (sinceFilter && entry.ts && new Date(entry.ts) < sinceFilter) continue;
    if (eventFilter && entry.event !== eventFilter) continue;
    const key = entry.event;
    if (!agg[key]) agg[key] = { count: 0, tokensAffected: 0, tokensSaved: 0, notes: new Set() };
    agg[key].count++;
    if (typeof entry.tokens_affected === 'number') agg[key].tokensAffected += entry.tokens_affected;
    if (typeof entry.tokens_saved === 'number') agg[key].tokensSaved += entry.tokens_saved;
    if (entry.note) agg[key].notes.add(entry.note);
  }
}

const events = Object.keys(agg);
if (events.length === 0) {
  console.log('No metrics data yet');
  process.exit(0);
}

// Render markdown table
const header = '| Event | Count | Tokens Affected | Tokens Saved | Notes |';
const sep    = '|-------|-------|-----------------|--------------|-------|';
console.log(header);
console.log(sep);
let totalSaved = 0;
let totalAffected = 0;
let totalCount = 0;
for (const evt of events.sort()) {
  const { count, tokensAffected, tokensSaved, notes } = agg[evt];
  const noteStr = [...notes].slice(0, 2).join('; ') || '-';
  // When the event records "affected" but no "saved" (e.g. rtk-rewrite,
  // budget-check passing), surface the affected count instead of `-`.
  const affectedCell = tokensAffected > 0 ? tokensAffected : '-';
  const savedCell = tokensSaved > 0 ? tokensSaved : '-';
  console.log(`| ${evt} | ${count} | ${affectedCell} | ${savedCell} | ${noteStr} |`);
  totalSaved += tokensSaved;
  totalAffected += tokensAffected;
  totalCount += count;
}
console.log(sep);
console.log(`| **TOTAL** | ${totalCount} | ${totalAffected || '-'} | ${totalSaved || '-'} | - |`);

// ── RTK Integration ────────────────────────────────────────────────────
// Query RTK for actual savings data (if RTK is installed)
try {
  const { execFileSync } = require('child_process');
  let rtkAvailable = false;
  try {
    if (process.platform === 'win32') {
      execFileSync('where', ['rtk'], { stdio: 'ignore' });
    } else {
      execFileSync('which', ['rtk'], { stdio: 'ignore' });
    }
    rtkAvailable = true;
  } catch (_) {}

  if (rtkAvailable) {
    const rtkRaw = execFileSync('rtk', ['gain', '--all', '--format', 'json'], {
      encoding: 'utf8',
      timeout: 5000,
      stdio: ['pipe', 'pipe', 'ignore'],
    });
    const rtkData = JSON.parse(rtkRaw);

    console.log('');
    console.log('## RTK Token Savings');
    console.log('');

    if (rtkData.total_saved !== undefined) {
      const totalSaved = rtkData.total_saved || 0;
      const totalOriginal = rtkData.total_original || 0;
      const pct = totalOriginal > 0 ? Math.round((totalSaved / totalOriginal) * 100) : 0;
      console.log(`| Metric | Value |`);
      console.log(`|--------|-------|`);
      console.log(`| Total tokens saved | ${totalSaved.toLocaleString()} |`);
      console.log(`| Total original tokens | ${totalOriginal.toLocaleString()} |`);
      console.log(`| Savings rate | ${pct}% |`);
      console.log(`| Commands rewritten | ${rtkData.total_commands || '-'} |`);
    }

    // Per-command breakdown if available
    if (rtkData.by_command && typeof rtkData.by_command === 'object') {
      const cmds = Object.entries(rtkData.by_command);
      if (cmds.length > 0) {
        console.log('');
        console.log('### By Command');
        console.log('| Command | Saved | Original | Rate |');
        console.log('|---------|-------|----------|------|');
        for (const [cmd, stats] of cmds.sort((a, b) => (b[1].saved || 0) - (a[1].saved || 0)).slice(0, 10)) {
          const saved = stats.saved || 0;
          const orig = stats.original || 0;
          const rate = orig > 0 ? Math.round((saved / orig) * 100) + '%' : '-';
          console.log(`| ${cmd} | ${saved.toLocaleString()} | ${orig.toLocaleString()} | ${rate} |`);
        }
      }
    }
  }
} catch (_) {
  // RTK not installed or gain command failed — skip section silently
}

// ── Correlation: hook rewrites vs RTK actual savings ──────────────────
if (agg['rtk-rewrite']) {
  const hookRewrites = agg['rtk-rewrite'].count;
  const hookEstimatedSaved = agg['rtk-rewrite'].tokensSaved;
  console.log('');
  console.log('## RTK Hook Activity');
  console.log(`| Metric | Value |`);
  console.log(`|--------|-------|`);
  console.log(`| Commands rewritten by hook | ${hookRewrites} |`);
  console.log(`| Estimated tokens saved | ${hookEstimatedSaved > 0 ? hookEstimatedSaved.toLocaleString() : '-'} |`);
}
