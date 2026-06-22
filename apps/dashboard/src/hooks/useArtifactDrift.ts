// Per-project artifact-drift fan-out (B6 Wave 3).
//
// Companion to `useProjectDetections`. One TanStack Query per registered
// project, keyed by `project.path`, calling `artifact_update_check` (which
// shells out to `mustard-rt run artifact-update --check`).
//
// Failures (binary missing, manifest absent on non-Mustard repos) collapse to
// `undefined` data instead of breaking the row — the sidebar simply skips the
// badge when the report is unavailable.

import { useQueries, useQuery, type UseQueryResult } from "@tanstack/react-query";
import {
  artifactUpdateCheck,
  isMustardRepo,
  type ArtifactDriftReport,
} from "@/lib/projects";
import { useProjectsStore } from "@/lib/projects-store";

export interface ArtifactDriftRow {
  path: string;
  report: ArtifactDriftReport | undefined;
  isLoading: boolean;
  error: unknown;
}

export function useArtifactDrift(): Record<string, ArtifactDriftRow> {
  const projects = useProjectsStore((s) => s.projects);

  const queries = useQueries({
    queries: projects.map((p) => ({
      queryKey: ["artifact-drift", p.path],
      queryFn: () => artifactUpdateCheck(p.path),
      // Drift moves on upstream-push cadence (minutes-to-hours). 60s keeps the
      // sidebar lively without thrashing `mustard-rt` subprocess spawns.
      staleTime: 60_000,
      // Per dashboard guard: no aggressive refetchInterval — the FS watcher
      // already pushes generic project events; drift queries piggyback on
      // staleTime + manual invalidation after `--apply`.
      retry: false,
    })),
  });

  const map: Record<string, ArtifactDriftRow> = {};
  projects.forEach((p, i) => {
    const q = queries[i] as
      | UseQueryResult<ArtifactDriftReport, unknown>
      | undefined;
    map[p.path] = {
      path: p.path,
      report: q?.data,
      isLoading: q?.isLoading ?? false,
      error: q?.error,
    };
  });
  return map;
}

/// Single-project lookup: does the path hold the canonical artifact manifest?
/// Used by the sidebar to gate the "Update artifacts" menu entry — only the
/// Mustard repo itself owns `apps/cli/templates/.artifacts.json`. The check is
/// a cheap filesystem stat on the Rust side, but we still cache it because the
/// menu opens often and the answer is stable per-project for the session.
export function useIsMustardRepo(projectPath: string): boolean {
  const { data } = useQuery({
    queryKey: ["is-mustard-repo", projectPath],
    queryFn: () => isMustardRepo(projectPath),
    staleTime: 5 * 60_000,
    retry: false,
  });
  return data ?? false;
}
