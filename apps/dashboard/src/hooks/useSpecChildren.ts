import { useQuery } from "@tanstack/react-query";
import { dashboardSpecChildren, type SpecChild } from "@/lib/dashboard";

/**
 * Wave-3 (2026-05-20, spec `2026-05-20-tactical-fix-via-sub-spec`): list
 * sub-specs linked to `parent` via `spec.link` events. Disabled until both
 * `repoPath` and `parent` are set; refetches every 30s while the tab/page is
 * focused so a newly-created sub-spec shows up without a manual reload.
 */
export function useSpecChildren(
  repoPath: string | null,
  parent: string | null,
) {
  return useQuery<SpecChild[]>({
    queryKey: ["spec-children", repoPath, parent],
    queryFn: () => dashboardSpecChildren(repoPath as string, parent as string),
    enabled: !!repoPath && !!parent,
    staleTime: 10_000,
    refetchInterval: 30_000,
    refetchIntervalInBackground: false,
  });
}
