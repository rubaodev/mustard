import { useQuery } from "@tanstack/react-query";
import { dashboardMemoryCrossWave } from "@/lib/dashboard";

/**
 * Wave-5 (spec `2026-05-21-dashboard-spec-tabs`): cross-wave memory for the
 * Rede tab sidebar. Returns the raw markdown produced by
 * `mustard-rt run memory cross-wave --spec <spec> --wave <wave>`. Disabled
 * until both `repoPath` and `wave > 1` are known (wave 1 has no priors).
 */
export function useSpecMemoryCrossWave(
  repoPath: string | null,
  spec: string,
  wave: number | null,
) {
  return useQuery<string>({
    queryKey: ["spec-memory-cross-wave", repoPath, spec, wave],
    queryFn: () => dashboardMemoryCrossWave(repoPath as string, spec, wave as number),
    enabled: !!repoPath && !!spec && wave != null && wave > 1,
    staleTime: 60_000,
  });
}
