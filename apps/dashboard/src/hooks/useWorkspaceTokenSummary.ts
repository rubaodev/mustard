import { useQuery } from "@tanstack/react-query";
import { dashboardTokenSummary, type TokenSummary } from "@/lib/dashboard";

/**
 * Aggregate token-savings totals + top-N pipelines for a single workspace.
 * Backed by the `dashboard_token_summary` Tauri command. Disabled when
 * `repoPath` is null so the hook is safe to mount before a workspace is
 * selected.
 */
export function useWorkspaceTokenSummary(repoPath: string | null) {
  return useQuery<TokenSummary>({
    queryKey: ["workspace-token-summary", repoPath],
    queryFn: () => dashboardTokenSummary(repoPath as string),
    enabled: !!repoPath,
    staleTime: 10_000,
  });
}
