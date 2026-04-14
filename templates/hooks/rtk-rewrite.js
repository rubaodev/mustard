#!/usr/bin/env node
'use strict';
/**
 * RTK REWRITE: PreToolUse hook that rewrites Bash commands through RTK
 *
 * Uses `rtk rewrite` (the official hook API) to get the optimized command.
 * Exit 0 + stdout = rewritten command; Exit 1 = no RTK equivalent.
 *
 * This approach:
 * - Eliminates the "No hook installed" warning (no `rtk <cmd>` prefix)
 * - Delegates command selection to RTK itself (no manual command set)
 * - Works cross-platform (Windows + Unix)
 *
 * RTK availability is cached in a temp file (60s TTL) to avoid spawning
 * which/where on every command invocation.
 *
 * Fail-open: exits 0 on any error so Claude is never blocked by this hook.
 *
 * @version 2.0.0
 */

const { execFileSync, execSync } = require('child_process');
const fs = require('fs');
const path = require('path');
const os = require('os');
const { shouldRun } = require('./_lib/hook-env.js');
const { emitMetric } = require('./_lib/metrics-emit.js');

const CACHE_FILE = path.join(os.tmpdir(), 'rtk-available.json');
const CACHE_TTL_MS = 60_000;

/**
 * Returns true if `rtk` is available in PATH, using a cached result when
 * the cache is still within TTL.
 */
function isRtkAvailable() {
  try {
    if (fs.existsSync(CACHE_FILE)) {
      const raw = fs.readFileSync(CACHE_FILE, 'utf8');
      const cached = JSON.parse(raw);
      if (Date.now() - cached.ts < CACHE_TTL_MS) {
        return cached.available;
      }
    }
  } catch (_) {
    // Cache read failed — fall through to fresh check
  }

  let available = false;
  try {
    if (process.platform === 'win32') {
      execFileSync('where', ['rtk'], { stdio: 'ignore' });
    } else {
      execFileSync('which', ['rtk'], { stdio: 'ignore' });
    }
    available = true;
  } catch (_) {
    available = false;
  }

  try {
    fs.writeFileSync(CACHE_FILE, JSON.stringify({ available, ts: Date.now() }), 'utf8');
  } catch (_) {
    // Cache write failed — non-fatal
  }

  return available;
}

/**
 * Asks RTK to rewrite the command. Returns the rewritten command string,
 * or null if RTK has no optimized equivalent (exit code 1).
 */
function rtkRewrite(cmd) {
  try {
    // rtk rewrite expects the raw command as a single argv element.
    // Using execFileSync avoids shell re-parsing, which would strip quotes
    // and corrupt regex patterns containing brackets (e.g. grep '[x]').
    const result = execFileSync('rtk', ['rewrite', cmd], {
      encoding: 'utf8',
      stdio: ['pipe', 'pipe', 'ignore'], // ignore stderr
      timeout: 3000,
    });
    const rewritten = result.trim();
    return rewritten || null;
  } catch (_) {
    // Exit 1 = no RTK equivalent, or timeout/error
    return null;
  }
}

let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => (input += chunk));
process.stdin.on('end', () => {
  try {
    if (!shouldRun('rtk-rewrite')) { process.exit(0); }
    const data = JSON.parse(input);
    const cmd = data.tool_input?.command || '';

    // Already prefixed with rtk or RTK not available — pass through
    if (cmd.startsWith('rtk ') || !isRtkAvailable()) {
      process.exit(0);
    }

    // Ask RTK for the rewritten command
    const rewritten = rtkRewrite(cmd);
    if (!rewritten || rewritten === cmd) {
      // No optimization available or same command — pass through
      process.exit(0);
    }

    /**
     * Estimate token savings percentage for known command types.
     * These percentages come from RTK's documented savings rates.
     * Conservative estimates — actual savings are often higher.
     */
    const SAVINGS_RATES = {
      git:      0.70,  // git status/log/diff: 59-80%
      grep:     0.75,  // search output: 75%
      rg:       0.75,
      ls:       0.65,  // directory listing: 65%
      find:     0.70,  // find output: 70%
      cat:      0.60,  // file content: 60%
      head:     0.60,
      tail:     0.60,
      cargo:    0.85,  // build/test output: 80-90%
      dotnet:   0.85,
      npm:      0.80,
      pnpm:     0.80,
      npx:      0.80,
      vitest:   0.95,  // test output: 90-99%
      jest:     0.90,
      playwright: 0.94,
      docker:   0.85,
      kubectl:  0.85,
      tsc:      0.83,
      gh:       0.80,  // GitHub CLI: 26-87% avg ~80%
      curl:     0.70,
      node:     0.70,
      prisma:   0.88,
    };

    function estimateSavings(cmd) {
      // Extract first token from the command (before RTK rewriting)
      const firstToken = cmd.trim().split(/\s+/)[0].replace(/^[A-Z_]+=\S+\s*/, '');
      const base = firstToken.split('/').pop(); // handle full paths like /usr/bin/git
      return SAVINGS_RATES[base] || 0.50; // default 50% for unknown commands
    }

    const rate = estimateSavings(cmd);
    const estimatedOutputTokens = Math.round(cmd.length / 4) * 10; // rough: output is ~10x command length
    const estimatedSaved = Math.round(estimatedOutputTokens * rate);

    emitMetric('rtk-rewrite', {
      tokensAffected: Math.round(cmd.length / 4),
      tokensSaved: estimatedSaved,
      note: 'rewritten via rtk',
      extras: {
        command_head: cmd.slice(0, 60),
        savings_rate: rate,
      },
    });

    console.log(JSON.stringify({
      hookSpecificOutput: {
        hookEventName: 'PreToolUse',
        permissionDecision: 'allow',
        updatedInput: { command: `${rewritten} 2>/dev/null` }
      }
    }));
    process.exit(0);
  } catch (err) {
    process.stderr.write(`[rtk-rewrite] Error: ${err.message}\n`);
    process.exit(0);
  }
});
