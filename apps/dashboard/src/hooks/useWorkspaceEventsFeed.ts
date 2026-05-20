import { useQuery } from "@tanstack/react-query";
import { dashboardEventsFeed, type FeedEvent } from "@/lib/dashboard";

/**
 * Most-recent feed events for the workspace, newest first. Polls every 5s
 * and refetches on window focus so the feed feels live without depending on
 * the fs watcher. Disabled when `repoPath` is null.
 */
export function useWorkspaceEventsFeed(repoPath: string | null, limit: number = 50) {
  return useQuery<FeedEvent[]>({
    queryKey: ["workspace-events-feed", repoPath, limit],
    queryFn: () => dashboardEventsFeed(repoPath as string, limit),
    enabled: !!repoPath,
    refetchInterval: 5_000,
    refetchOnWindowFocus: true,
  });
}
