import { useQuery } from "@tanstack/react-query";
import { dashboardMonthActivity, type DayActivity } from "@/lib/dashboard";

/**
 * Per-day activity counts for the given month, used by the workspace
 * heatmap/calendar. `month` is 1..12. Disabled when `repoPath` is null.
 */
export function useWorkspaceMonthActivity(
  repoPath: string | null,
  year: number,
  month: number,
) {
  return useQuery<DayActivity[]>({
    queryKey: ["workspace-month-activity", repoPath, year, month],
    queryFn: () => dashboardMonthActivity(repoPath as string, year, month),
    enabled: !!repoPath,
    staleTime: 30_000,
  });
}
