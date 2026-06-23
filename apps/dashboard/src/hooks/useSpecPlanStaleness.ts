import { useMutation } from "@tanstack/react-query";
import { dashboardSpecPlanStaleness, type Staleness } from "@/lib/dashboard";

/**
 * On-demand plan-staleness check for one spec (spec `melhorias-pagina-specs`,
 * item 4). Deliberately a `useMutation`, NOT a `useQuery`: the underlying
 * command shells out to `git log` per census file, so it must fire ONLY on an
 * explicit "Reanalisar" click — never on mount, focus, or a watcher tick.
 *
 * Call `mutate({ repoPath, spec, startedAt })` to run it; read the verdict from
 * `data` (a {@link Staleness}). The backend is fail-open, so `data` always
 * resolves — `verdict: "unknown"` carries a `reason` rather than rejecting.
 * Each spec row owns its own hook instance so verdicts don't bleed across rows.
 */
export function useSpecPlanStaleness() {
  return useMutation<
    Staleness,
    Error,
    { repoPath: string; spec: string; startedAt: string | null }
  >({
    mutationFn: ({ repoPath, spec, startedAt }) =>
      dashboardSpecPlanStaleness(repoPath, spec, startedAt),
  });
}
