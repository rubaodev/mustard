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
 * the intent against the real repo model (`.claude/grain.model.json`, via scan)
 * and the filesystem.
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
 * Reads the repo model's known entity names from `.claude/grain.model.json`
 * **via the scan tool** (the dashboard never parses the model directly).
 * Returns an empty array when the project hasn't been scanned yet.
 */
export async function readModelEntities(projectPath: string): Promise<string[]> {
  return invoke<string[]>('read_model_entities', { repoPath: projectPath });
}
