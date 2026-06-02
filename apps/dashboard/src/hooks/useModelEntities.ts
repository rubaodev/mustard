import { useQuery } from '@tanstack/react-query';
import { readModelEntities } from '@/api/prd';

/**
 * Loads the project's known entity names (declaration names from the repo model
 * `.claude/grain.model.json`, read via the scan tool). Disabled until
 * `projectPath` is set; the Rust side returns an empty list when the model
 * doesn't exist, so the UI never crashes on a project that hasn't been scanned.
 *
 * Keyed by absolute project path so switching projects refetches.
 */
export function useModelEntities(projectPath: string | null) {
  return useQuery<string[]>({
    queryKey: ['model-entities', projectPath],
    queryFn: () => readModelEntities(projectPath as string),
    enabled: !!projectPath,
    staleTime: 60_000,
  });
}
