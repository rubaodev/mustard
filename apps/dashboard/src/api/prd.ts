// Tauri command wrappers for the PRD lapidator (spec
// 2026-05-20-dashboard-prd-ai-lapidator, Wave 3).
//
// Per dashboard guards: components must never call `invoke()` directly —
// every Tauri surface lives in `src/api/*.ts` or `src/lib/dashboard.ts`.

import { invoke } from '@tauri-apps/api/core';
import type { LapidatedPrd } from '@/lib/types/prd';

/**
 * Shells out to the local Claude CLI (via the Rust backend) to lapidate a
 * free-form intent into a structured PRD. `projectPath` is the absolute path
 * to the project root — the CLI runs with cwd = projectPath so it can confront
 * the intent against the real `.claude/entity-registry.json` and filesystem.
 */
export async function lapidatePrd(intent: string, projectPath: string): Promise<LapidatedPrd> {
  return invoke<LapidatedPrd>('lapidate_prd', { intent, projectPath });
}

/**
 * Probes whether the `claude` CLI is available in PATH. Cheap (~10ms);
 * call once at page mount to decide whether to disable the "Lapidar com IA"
 * button + render the install hint.
 */
export async function checkClaudeAvailable(): Promise<boolean> {
  return invoke<boolean>('check_claude_available');
}

/**
 * Reads `.claude/entity-registry.json` from the given project root and returns
 * the discovered entity names. Falls back to top-level keys minus reserved
 * `_*` prefixes when there is no explicit `entities` key (the common shape
 * today). Returns an empty array when the registry is missing.
 */
export async function readEntityRegistry(projectPath: string): Promise<string[]> {
  return invoke<string[]>('read_entity_registry', { repoPath: projectPath });
}
