import { useQuery } from "@tanstack/react-query";
import {
  dashboardSpecChecklistProgress,
  type WaveChecklistProgress,
} from "@/lib/dashboard";

/**
 * Wave 3 (spec `checklist-progresso-por-onda`) — per-wave checklist progress
 * (`done`/`total`) for one spec. Backed by `dashboard_spec_checklist_progress`
 * (meta.json sidecars + `checklist.item.marked` NDJSON events). The watcher
 * invalidates the `spec-checklist` key on `.events/` and spec-dir writes, so
 * the counts refresh event-driven; the interval is only a safety net.
 */
export function useSpecChecklistProgress(
  repoPath: string | null,
  spec: string | null,
) {
  return useQuery<WaveChecklistProgress[]>({
    queryKey: ["spec-checklist", repoPath, spec],
    queryFn: () =>
      dashboardSpecChecklistProgress(repoPath as string, spec as string),
    enabled: !!repoPath && !!spec,
    staleTime: 5_000,
    refetchInterval: 60_000,
    refetchIntervalInBackground: false,
  });
}
