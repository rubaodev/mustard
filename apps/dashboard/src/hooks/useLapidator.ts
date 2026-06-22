import { useCallback, useEffect, useState } from 'react';
import { toast } from 'sonner';
import { checkClaudeAvailable, lapidatePrd } from '@/api/prd';
import type { LapidatedPrd, PrdConfront } from '@/lib/types/prd';

/**
 * Owns all PRD-lapidator local UI state and the call into the Tauri
 * `lapidate_prd` command. Extracted from `pages/Prd.tsx`
 * (spec 2026-05-21-prd-lapidator-polish) so the page can focus on form
 * orchestration instead of CLI plumbing.
 *
 * `lapidate(projectPath, applyToForm)` keeps `projectPath` as a per-call
 * argument because the active project is derived from form state in the
 * caller — wiring it into the hook itself would force a refetch on every
 * project change. The `applyToForm` callback receives the raw result so
 * the caller can run its own `setForm(...)` mapping plus any auxiliary
 * side effects (e.g. `aiSuggestedEntities`, `scopeOverride`).
 */
export interface UseLapidatorResult {
  intent: string;
  setIntent: (next: string) => void;
  isLapidating: boolean;
  lapidateError: string | null;
  confront: PrdConfront | null;
  claudeAvailable: boolean | null;
  selectedEntities: string[];
  setSelectedEntities: (next: string[]) => void;
  lapidate: (
    projectPath: string,
    applyToForm: (r: LapidatedPrd) => void,
  ) => Promise<void>;
  reset: () => void;
}

export function useLapidator(): UseLapidatorResult {
  const [intent, setIntent] = useState('');
  const [isLapidating, setIsLapidating] = useState(false);
  const [lapidateError, setLapidateError] = useState<string | null>(null);
  const [confront, setConfront] = useState<PrdConfront | null>(null);
  const [claudeAvailable, setClaudeAvailable] = useState<boolean | null>(null);
  const [selectedEntities, setSelectedEntities] = useState<string[]>([]);

  // CLI availability probe — runs once at mount.
  useEffect(() => {
    let alive = true;
    checkClaudeAvailable()
      .then((ok) => {
        if (alive) setClaudeAvailable(ok);
      })
      .catch(() => {
        if (alive) setClaudeAvailable(false);
      });
    return () => {
      alive = false;
    };
  }, []);

  const lapidate = useCallback(
    async (
      projectPath: string,
      applyToForm: (r: LapidatedPrd) => void,
    ): Promise<void> => {
      if (!intent.trim() || !projectPath) return;
      setIsLapidating(true);
      setLapidateError(null);
      try {
        const result: LapidatedPrd = await lapidatePrd(intent, projectPath);
        applyToForm(result);
        setSelectedEntities(result._confront.entitiesFound);
        setConfront(result._confront);
        toast.success('PRD lapidado');
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setLapidateError(msg);
        toast.error('Falha ao lapidar: ' + msg);
      } finally {
        setIsLapidating(false);
      }
    },
    [intent],
  );

  const reset = useCallback(() => {
    setIntent('');
    setConfront(null);
    setLapidateError(null);
    setSelectedEntities([]);
  }, []);

  return {
    intent,
    setIntent,
    isLapidating,
    lapidateError,
    confront,
    claudeAvailable,
    selectedEntities,
    setSelectedEntities,
    lapidate,
    reset,
  };
}
