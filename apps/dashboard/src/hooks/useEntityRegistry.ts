import { useQuery } from '@tanstack/react-query';
import { readEntityRegistry } from '@/api/prd';

/**
 * Loads the project's entity-registry entries (top-level entity names from
 * `.claude/entity-registry.json`). Disabled until `projectPath` is set; the
 * Rust side returns an empty list when the file doesn't exist, so the UI
 * never crashes on a project that hasn't been scanned yet.
 *
 * Keyed by absolute project path so switching projects refetches.
 */
export function useEntityRegistry(projectPath: string | null) {
  return useQuery<string[]>({
    queryKey: ['entity-registry', projectPath],
    queryFn: () => readEntityRegistry(projectPath as string),
    enabled: !!projectPath,
    staleTime: 60_000,
  });
}
