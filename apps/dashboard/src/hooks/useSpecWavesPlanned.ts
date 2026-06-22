import { useQuery } from "@tanstack/react-query";
import {
  dashboardSpecWavesPlanned,
  type SpecWavePlanned,
} from "@/lib/dashboard";

/**
 * Wave 1 polish (spec `2026-05-21-dashboard-spec-tabs-polish`) — fetches the
 * wave structure declared on disk via a filesystem scan of
 * `<repo>/.claude/spec/{spec}/wave-N-{role}/`. The consumer (`SpecWavesTab`)
 * unions the result with `useSpecWaves` (which reads from the SQLite event
 * stream) so the "Ondas" tab never goes blank during EXECUTE: waves declared
 * by the wave-plan show up as `queued` until the event log catches up with
 * `wave.start`.
 *
 * `staleTime: 30s` matches `useSpecWaveFiles` — the wave-plan filesystem
 * almost never changes during a session (PLAN-only writes), so the cheap FS
 * scan does not need to fire on every refetch tick.
 */
export function useSpecWavesPlanned(
  repoPath: string | null,
  spec: string | null,
) {
  return useQuery<SpecWavePlanned[]>({
    queryKey: ["spec-waves-planned", repoPath, spec],
    queryFn: () =>
      dashboardSpecWavesPlanned(repoPath as string, spec as string),
    enabled: !!repoPath && !!spec,
    staleTime: 30_000,
  });
}
