#!/usr/bin/env node
'use strict';
/**
 * SUBAGENT TRACKER: Tracks active subagents for statusline display
 *
 * Handles 5 events:
 * - PreToolUse(Task):  queues description + type before agent starts
 * - PostToolUse(Task): detects API overload / dispatch failures and flags pipeline state
 * - SubagentStart:     writes agent state file (consumes from queue)
 * - SubagentStop:      removes agent state file + persists agent findings to .agent-memory + prunes stale queue
 * - SessionStart:      cleans up stale state from previous sessions
 *
 * State dir: .claude/.agent-state/{agent_id}.json
 * Queue:     .claude/.agent-state/_queue.json
 *
 * Also injects agent memory (from .claude/.agent-memory/) into new agents
 * via additionalContext — enabling zero-parent-token cross-wave communication.
 *
 * @version 3.0.0
 */

const fs = require('fs');
const path = require('path');
const { shouldRun, isSelfDelegation } = require('./_lib/hook-env.js');

const QUEUE_FILE = '_queue.json';
const QUEUE_STALE_MS = 60_000; // 60 seconds
const MAX_QUEUE_SIZE = 10;

const DEDUP_FILE = 'explorer-dedup.json';
const DEDUP_DENY_MS  = 60_000;  // deny window: same type within 60s → block
const DEDUP_CLEAN_MS = 120_000; // prune entries older than 120s when reading

const MEMORY_DIR = '.agent-memory';
const MEMORY_INDEX = '_index.json';
const MEMORY_MAX_CHARS = 800;

let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', (chunk) => (input += chunk));
process.stdin.on('end', () => {
  try {
    if (!shouldRun('subagent-tracker')) { process.exit(0); }
    const data = JSON.parse(input);
    const event = data.hook_event_name;
    const projectDir = data.cwd || process.cwd();
    const stateDir = path.join(projectDir, '.claude', '.agent-state');

    if (event === 'PreToolUse' && data.tool_name === 'Task') {
      handlePreToolUse(data, stateDir);
    } else if (event === 'PostToolUse' && data.tool_name === 'Task') {
      handlePostToolUse(data, stateDir);
    } else if (event === 'SubagentStart') {
      handleStart(data, stateDir);
    } else if (event === 'SubagentStop') {
      handleStop(data, stateDir);
    } else if (event === 'SessionStart') {
      handleSessionStart(data, stateDir);
    }

    process.exit(0);
  } catch (err) {
    process.stderr.write(`[subagent-tracker] Error: ${err.message}\n`);
    process.exit(0);
  }
});

/**
 * PreToolUse(Task): Queue description + type before agent spawns.
 * The SubagentStart event doesn't carry description, so we capture it here
 * and match it later via FIFO queue with type-matching preference.
 *
 * Also parses recommended_skills from the Task prompt, persists them in
 * .subagent-registry.json, and increments skillHits.loaded in the active
 * pipeline state.
 */
function handlePreToolUse(data, stateDir) {
  if (isSelfDelegation(data)) { return; }
  const toolInput = data.tool_input || {};
  const description = toolInput.description || '';
  const subagentType = toolInput.subagent_type || 'unknown';

  if (!description && !subagentType) return;

  ensureDir(stateDir);

  // ── Explorer dedup: deny if same subagent_type was dispatched within 60s ──
  if (isExplorerAgent(subagentType)) {
    try {
      const { cache, changed } = readDedupCache(stateDir);
      const lastTs = cache[subagentType];
      const now = Date.now();

      if (lastTs !== undefined && (now - lastTs) < DEDUP_DENY_MS) {
        const secondsAgo = Math.round((now - lastTs) / 1000);
        // Flush stale entries if any were pruned (best-effort, not required for deny path)
        if (changed) writeDedupCache(stateDir, cache);
        process.stdout.write(JSON.stringify({
          permissionDecision: 'deny',
          permissionDecisionReason:
            `[Dedup] ${subagentType} already dispatched ${secondsAgo}s ago. ` +
            `Wait or use a different explorer.`,
        }) + '\n');
        process.exit(0);
      }

      // Record this dispatch
      cache[subagentType] = now;
      writeDedupCache(stateDir, cache);
    } catch {} // fail-open: dedup is advisory — allow on any error
  }

  pruneQueue(stateDir);

  const queue = readQueue(stateDir);
  queue.push({
    description,
    type: subagentType,
    queued_at: new Date().toISOString(),
  });
  if (queue.length > MAX_QUEUE_SIZE) {
    queue.splice(0, queue.length - MAX_QUEUE_SIZE);
  }
  writeQueue(stateDir, queue);

  // ── skill_hit_rate: parse recommended_skills from Task prompt ─────────────
  // We look for a "Recommended Skills" section header followed by list items,
  // or a `recommended_skills:` YAML-style block.  Conservative regex — false
  // negatives are acceptable; false positives would corrupt the metric.
  try {
    const prompt = toolInput.prompt || '';
    const recommendedSkills = parseRecommendedSkills(prompt);
    if (recommendedSkills.length === 0) return;

    const projectDir = path.resolve(stateDir, '..', '..');

    // Persist entry to .subagent-registry.json for later Read attribution
    const registryPath = path.join(projectDir, '.claude', '.subagent-registry.json');
    let registry = {};
    try {
      if (fs.existsSync(registryPath)) {
        registry = JSON.parse(fs.readFileSync(registryPath, 'utf8'));
      }
    } catch {}
    // Use timestamp + agentType as pseudo-taskId (best effort — no real taskId available at PreToolUse)
    const taskId = `${Date.now()}-${subagentType}`;
    registry[taskId] = {
      agentType: subagentType,
      recommendedSkills,
      startedAt: new Date().toISOString(),
      // endedAt is written when SubagentStop fires (not implemented here — left undefined)
    };
    // Prune entries older than 2 hours to prevent unbounded growth
    const cutoff = Date.now() - 2 * 60 * 60 * 1000;
    for (const [key, entry] of Object.entries(registry)) {
      if (new Date(entry.startedAt || 0).getTime() < cutoff) {
        delete registry[key];
      }
    }
    fs.writeFileSync(registryPath, JSON.stringify(registry, null, 2), 'utf8');

    // Increment skillHits.loaded in the active pipeline state
    const statesDir = path.join(projectDir, '.claude', '.pipeline-states');
    if (!fs.existsSync(statesDir)) return;
    const stateFiles = fs.readdirSync(statesDir).filter(f => f.endsWith('.json'));
    if (stateFiles.length === 0) return;

    let newestState = null;
    let newestMtime = 0;
    for (const f of stateFiles) {
      try {
        const fp = path.join(statesDir, f);
        const stat = fs.statSync(fp);
        if (stat.mtimeMs > newestMtime) {
          newestMtime = stat.mtimeMs;
          newestState = fp;
        }
      } catch {}
    }
    if (!newestState) return;

    const state = JSON.parse(fs.readFileSync(newestState, 'utf8'));
    if (!state.metrics) state.metrics = { apiCalls: 0, toolBreakdown: {}, retries: 0 };
    if (!state.metrics.skillHits) state.metrics.skillHits = {};
    if (!state.metrics.skillHits[subagentType]) {
      state.metrics.skillHits[subagentType] = { loaded: 0, read: 0 };
    }
    state.metrics.skillHits[subagentType].loaded += recommendedSkills.length;
    fs.writeFileSync(newestState, JSON.stringify(state, null, 2), 'utf8');
  } catch {} // fail-open: skill tracking is advisory
}

/**
 * Parse recommended skills from a Task prompt string.
 * Matches list items under a "Recommended Skills" or "recommended_skills:" header.
 * Returns an array of skill name strings (e.g. ["templates-hook-protocol"]).
 */
function parseRecommendedSkills(prompt) {
  const skills = [];
  // Match a section header then collect "- skill-name" lines until blank or next header
  const sectionMatch = prompt.match(
    /(?:recommended.skills|recommended_skills)\s*[:\-]?\s*\n((?:\s*-\s*[\w\-]+.*\n?)+)/i
  );
  if (sectionMatch) {
    const lines = sectionMatch[1].split('\n');
    for (const line of lines) {
      const m = line.match(/^\s*-\s*([\w][\w\-]*[\w])/);
      if (m) skills.push(m[1]);
    }
  }
  return skills;
}

/**
 * PostToolUse(Task): Detect API overload / dispatch failures in tool_response
 * and flag the active pipeline state with `lastDispatchFailure` so /resume can
 * auto-recover.
 *
 * We write to pipeline-state ONLY when a failure is detected — happy-path
 * dispatches never touch the state file from here.
 */
function handlePostToolUse(data, stateDir) {
  try {
    if (isSelfDelegation(data)) { return; }

    const toolResponse = data.tool_response || {};
    const responseText = JSON.stringify(toolResponse).toLowerCase();
    // Detect dispatch failures conservatively: require is_error=true (Claude
    // Code sets this on Task tool failures) AND at least one failure keyword.
    // Covers:
    //   - API overload / rate limiting (429, 529, throttle, too many requests)
    //   - Infrastructure errors (tool result missing, HTTP 5xx, service unavailable)
    // The regex avoids false positives on agents that merely *document* error
    // handling in their returned content (see "unrelated error" test below).
    const isDispatchFailure =
      toolResponse.is_error === true &&
      /overload|rate.?limit|\b429\b|\b529\b|throttl|too many requests|tool result missing|\b50[0-4]\b|service unavailable/.test(responseText);

    if (!isDispatchFailure) return;

    const projectDir = path.resolve(stateDir, '..', '..');
    const statesDir = path.join(projectDir, '.claude', '.pipeline-states');
    if (!fs.existsSync(statesDir)) return;

    const files = fs.readdirSync(statesDir)
      .filter(f => f.endsWith('.json') && !f.endsWith('.metrics.json'));
    if (files.length === 0) return;

    let newest = null;
    let newestMtime = 0;
    for (const f of files) {
      try {
        const fp = path.join(statesDir, f);
        const stat = fs.statSync(fp);
        if (stat.mtimeMs > newestMtime) {
          newestMtime = stat.mtimeMs;
          newest = fp;
        }
      } catch {}
    }
    if (!newest) return;

    const toolInput = data.tool_input || {};
    const state = JSON.parse(fs.readFileSync(newest, 'utf8'));
    state.lastDispatchFailure = {
      at: new Date().toISOString(),
      reason: 'dispatch_failure',
      agentType: toolInput.subagent_type || 'unknown',
      description: toolInput.description || '',
      prompt: (toolInput.prompt || '').slice(0, 2000),
    };
    fs.writeFileSync(newest, JSON.stringify(state, null, 2), 'utf8');
  } catch {} // fail-open: failure detection is advisory
}

function handleStart(data, stateDir) {
  const agentId = data.agent_id || `unknown-${Date.now()}`;
  const agentType = data.agent_type || 'unknown';

  ensureDir(stateDir);

  // Try to consume a matching entry from the queue
  let description = '';
  const queue = readQueue(stateDir);

  if (queue.length > 0) {
    // Prefer type-match first
    const typeIdx = queue.findIndex((q) => q.type === agentType);
    if (typeIdx >= 0) {
      description = queue[typeIdx].description;
      queue.splice(typeIdx, 1);
    } else {
      // FIFO fallback
      description = queue[0].description;
      queue.shift();
    }
    writeQueue(stateDir, queue);
  }

  fs.writeFileSync(
    path.join(stateDir, `${agentId}.json`),
    JSON.stringify({
      type: agentType,
      description,
      started_at: new Date().toISOString(),
      session_id: data.session_id,
    }),
  );

  // Build additionalContext with optional memory injection
  const projectDir = path.resolve(stateDir, '..', '..');
  let context = `[Tracker] Agent "${agentType}" registered. Follow all CLAUDE.md rules.`;

  try {
    const memories = loadRelevantMemories(projectDir, agentType);
    if (memories.length > 0) {
      context += '\n\n[Agent Memory] Findings from prior agents:\n' +
        memories.map(m => `- [${m.agent_type}] ${m.summary}`).join('\n');
    }
  } catch {} // fail-open: memory injection is advisory

  const response = {
    hookSpecificOutput: {
      hookEventName: 'SubagentStart',
      additionalContext: context,
    },
  };
  console.log(JSON.stringify(response));
}

function handleStop(data, stateDir) {
  const agentId = data.agent_id || '';
  const agentType = data.agent_type || 'unknown';
  const stateFile = path.join(stateDir, `${agentId}.json`);

  // Read agent state before deleting (for pipeline info)
  let agentState = {};
  try {
    if (fs.existsSync(stateFile)) {
      agentState = JSON.parse(fs.readFileSync(stateFile, 'utf8'));
    }
  } catch {}

  try {
    if (fs.existsSync(stateFile)) {
      fs.unlinkSync(stateFile);
    }
  } catch {}

  // Persist agent findings to .agent-memory via memory-write.js
  try {
    const toolResponse = data.tool_response || {};
    const responseText = typeof toolResponse === 'string' ? toolResponse : JSON.stringify(toolResponse);
    // Extract summary: first 300 chars, sentence-bounded
    let summary = (responseText || '').slice(0, 500);
    // Try to find first meaningful sentence
    const sentenceEnd = summary.search(/[.!?]\s/);
    if (sentenceEnd > 30) {
      summary = summary.slice(0, sentenceEnd + 1);
    } else if (summary.length > 300) {
      summary = summary.slice(0, 297) + '...';
    }

    if (summary.length > 20) {
      const projectDir = path.resolve(stateDir, '..', '..');
      const memScript = path.join(projectDir, '.claude', 'scripts', 'memory-write.js');
      if (fs.existsSync(memScript)) {
        const { execFileSync } = require('child_process');
        execFileSync(process.execPath, [memScript], {
          input: JSON.stringify({
            agent_type: agentType,
            wave: agentState.wave || null,
            pipeline: agentState.pipeline || '',
            summary: summary,
            cwd: projectDir,
          }),
          timeout: 3000,
          stdio: ['pipe', 'pipe', 'pipe'],
        });
      }
    }
  } catch {} // fail-open: memory persistence is advisory

  // Prune stale queue entries (>60s old)
  pruneQueue(stateDir);

  // Clean empty directory
  try {
    if (fs.existsSync(stateDir)) {
      const remaining = fs.readdirSync(stateDir).filter((f) => f.endsWith('.json'));
      if (remaining.length === 0) {
        fs.rmdirSync(stateDir);
      }
    }
  } catch {}
}

function handleSessionStart(data, stateDir) {
  // Clean up stale state files from previous/crashed sessions.
  // Threshold is 10 minutes: agent tasks rarely exceed this, and anything
  // older on a new SessionStart is certainly from a dead session (ghost).
  const STALE_MS = 10 * 60 * 1000; // 10 minutes
  try {
    if (!fs.existsSync(stateDir)) return;
    const files = fs.readdirSync(stateDir).filter(f => f.endsWith('.json') && f !== QUEUE_FILE);
    const now = Date.now();

    for (const f of files) {
      const filePath = path.join(stateDir, f);
      try {
        const content = JSON.parse(fs.readFileSync(filePath, 'utf8'));
        const fileAge = now - new Date(content.started_at || 0).getTime();
        // Remove if: stale (>10min) OR no session_id (legacy) OR different session
        if (fileAge > STALE_MS || !content.session_id || content.session_id !== data.session_id) {
          fs.unlinkSync(filePath);
        }
      } catch {
        // Corrupt file — remove it
        try { fs.unlinkSync(filePath); } catch {}
      }
    }

    // Prune stale queue entries
    pruneQueue(stateDir);

    // Clean empty directory
    try {
      const remaining = fs.readdirSync(stateDir);
      if (remaining.length === 0) fs.rmdirSync(stateDir);
    } catch {}
  } catch {}
}

/**
 * Load relevant memories from previous agents in the same pipeline.
 * Returns array of { agent_type, summary } objects, budget-capped at MEMORY_MAX_CHARS.
 */
function loadRelevantMemories(projectDir, agentType) {
  const memDir = path.join(projectDir, '.claude', MEMORY_DIR);
  const indexPath = path.join(memDir, MEMORY_INDEX);
  if (!fs.existsSync(indexPath)) return [];

  const index = JSON.parse(fs.readFileSync(indexPath, 'utf8'));
  if (!Array.isArray(index) || index.length === 0) return [];

  // Filter: exclude same agent type, sort by wave then timestamp (newest first)
  const filtered = index
    .filter(m => m.agent_type !== agentType)
    .sort((a, b) => {
      if ((a.wave || 0) !== (b.wave || 0)) return (a.wave || 0) - (b.wave || 0);
      return new Date(b.timestamp) - new Date(a.timestamp);
    });

  // Budget-cap: accumulate summaries until MEMORY_MAX_CHARS
  const result = [];
  let chars = 0;
  for (const m of filtered) {
    const summary = m.summary || '';
    if (chars + summary.length > MEMORY_MAX_CHARS) break;
    result.push(m);
    chars += summary.length;
  }

  return result;
}

// ── Explorer dedup helpers ──

/**
 * Returns true when the subagent_type represents an explorer agent.
 * Matches "Explore" (native Claude Code type) and any custom type containing
 * "explorer" (case-insensitive, e.g. "Sialia.Backend-explorer").
 */
function isExplorerAgent(subagentType) {
  if (!subagentType) return false;
  return subagentType === 'Explore' || /explorer/i.test(subagentType);
}

/**
 * Read the dedup cache, pruning entries older than DEDUP_CLEAN_MS.
 * Returns { cache, changed } where changed=true if stale entries were removed.
 * Fail-open: returns empty cache on any I/O error.
 */
function readDedupCache(stateDir) {
  const filePath = path.join(stateDir, DEDUP_FILE);
  try {
    let raw = {};
    if (fs.existsSync(filePath)) {
      raw = JSON.parse(fs.readFileSync(filePath, 'utf8'));
    }
    const now = Date.now();
    let changed = false;
    for (const [key, ts] of Object.entries(raw)) {
      if (now - ts > DEDUP_CLEAN_MS) {
        delete raw[key];
        changed = true;
      }
    }
    return { cache: raw, changed };
  } catch {
    return { cache: {}, changed: false };
  }
}

/**
 * Persist the dedup cache to disk. Fail-open: silently ignores write errors.
 */
function writeDedupCache(stateDir, cache) {
  try {
    fs.writeFileSync(path.join(stateDir, DEDUP_FILE), JSON.stringify(cache), 'utf8');
  } catch {}
}

// ── Queue helpers ──

function readQueue(stateDir) {
  const queueFile = path.join(stateDir, QUEUE_FILE);
  try {
    if (fs.existsSync(queueFile)) {
      return JSON.parse(fs.readFileSync(queueFile, 'utf8'));
    }
  } catch {}
  return [];
}

function writeQueue(stateDir, queue) {
  const queueFile = path.join(stateDir, QUEUE_FILE);
  try {
    if (queue.length === 0) {
      if (fs.existsSync(queueFile)) fs.unlinkSync(queueFile);
    } else {
      fs.writeFileSync(queueFile, JSON.stringify(queue));
    }
  } catch {}
}

function pruneQueue(stateDir) {
  const queue = readQueue(stateDir);
  if (queue.length === 0) return;

  const now = Date.now();
  const fresh = queue.filter((q) => {
    const age = now - new Date(q.queued_at).getTime();
    return age < QUEUE_STALE_MS;
  });

  if (fresh.length !== queue.length) {
    writeQueue(stateDir, fresh);
  }
}

function ensureDir(dir) {
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
}
