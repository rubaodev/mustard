import type { SpecWave } from "@/lib/types/specs";
import type { SpecWavePlanned } from "@/lib/dashboard";

/**
 * Union the event-derived `SpecWave[]` (from `useSpecWaves`, sourced from the
 * SQLite event stream) with the wave plan scanned from disk
 * (`useSpecWavesPlanned`, sourced from `<repo>/.claude/spec/{spec}/wave-N-{role}/`).
 *
 * Events-derived rows take precedence (they carry real timestamps + status);
 * plan-only waves the event log hasn't reached yet render as `queued`. This is
 * the single source of truth for the merge — both the "Ondas" detail tab
 * (`SpecWavesTab`) and the inline list expand (`SpecChildrenTree`) call it so
 * the full plan shows up in both places, not just the completed waves.
 */
export function mergeWaves(
  waves: SpecWave[],
  planned: SpecWavePlanned[] | undefined,
): SpecWave[] {
  const byWave = new Map<number, SpecWave>();
  for (const w of waves) byWave.set(w.wave, w);
  for (const p of planned ?? []) {
    if (byWave.has(p.wave)) continue;
    byWave.set(p.wave, {
      wave: p.wave,
      role: p.role,
      status: "queued",
      started_at: null,
      completed_at: null,
      agent_type: null,
      files_changed: p.declared_files_count,
    });
  }
  return Array.from(byWave.values()).sort((a, b) => a.wave - b.wave);
}
