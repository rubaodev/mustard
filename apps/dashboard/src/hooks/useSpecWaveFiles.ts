import { useQuery } from "@tanstack/react-query";
import { dashboardSpecWaveFiles, type WaveFilesPayload } from "@/lib/dashboard";

/**
 * Wave 2 (2026-05-21, spec `2026-05-21-dashboard-spec-tabs`) — fetches the
 * real file count + full markdown for a single wave sub-spec. The `enabled`
 * gate ensures we only spawn `mustard-rt run wave-files` once we have a real
 * `repoPath` + `spec` + a non-negative wave number; `wave < 0` is reserved
 * for "no wave selected" placeholders in callers.
 *
 * Wave 2 (spec `2026-05-21-dashboard-spec-tabs-polish`): `wave === 0` now
 * resolves to the parent's `wave-plan.md` (or `spec.md` fallback) so the
 * "Onda #0" row in the Ondas tab can share this hook.
 *
 * `staleTime: 30s` is intentionally longer than the spec-card refetch (5s)
 * because the underlying file (wave-N/spec.md) almost never changes during
 * a session — the only writes are during PLAN, which is rare.
 */
export function useSpecWaveFiles(
  repoPath: string | null,
  spec: string,
  wave: number,
) {
  return useQuery<WaveFilesPayload>({
    queryKey: ["spec-wave-files", repoPath, spec, wave],
    queryFn: () => dashboardSpecWaveFiles(repoPath as string, spec, wave),
    enabled: !!repoPath && !!spec && wave >= 0,
    staleTime: 30_000,
  });
}
