import { useMutation, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  completeSpec,
  cancelSpec,
  reactivateSpec,
  type SpecBucket,
  type SpecRow,
} from "@/lib/dashboard";

/**
 * Pointwise cache update after a lifecycle action on ONE spec (spec
 * `performance-dashboard-rotas-lentas-cache`, W3). The authoritative refresh
 * arrives via the `dashboard:specs-snapshot` push — the meta.json write fires
 * the watcher — so no list-wide invalidation here: seed the mutation's own
 * result (the new bucket) into the cached row for instant feedback and refetch
 * only this spec's card.
 */
function applySpecBucket(
  queryClient: ReturnType<typeof useQueryClient>,
  repoPath: string,
  specName: string,
  bucket: SpecBucket,
) {
  queryClient.setQueryData<SpecRow[]>(["specs", repoPath], (rows) =>
    rows?.map((row) => (row.name === specName ? { ...row, bucket } : row)),
  );
  queryClient.invalidateQueries({ queryKey: ["spec-card", repoPath, specName] });
}

export function useSpecActions(repoPath: string | undefined) {
  const queryClient = useQueryClient();

  const complete = useMutation({
    mutationFn: (specName: string): Promise<SpecBucket> => {
      if (!repoPath) return Promise.reject(new Error("Sem projeto selecionado"));
      return completeSpec(repoPath, specName);
    },
    onSuccess: (bucket, specName) => {
      toast.success(`Spec ${specName} concluída`);
      if (repoPath) applySpecBucket(queryClient, repoPath, specName, bucket);
    },
    onError: (err: unknown, specName) => {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error(`Falha ao concluir ${specName}: ${msg}`);
    },
  });

  const cancel = useMutation({
    mutationFn: (specName: string): Promise<SpecBucket> => {
      if (!repoPath) return Promise.reject(new Error("Sem projeto selecionado"));
      return cancelSpec(repoPath, specName);
    },
    onSuccess: (bucket, specName) => {
      toast.success(`Spec ${specName} cancelada`);
      if (repoPath) applySpecBucket(queryClient, repoPath, specName, bucket);
    },
    onError: (err: unknown, specName) => {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error(`Falha ao cancelar ${specName}: ${msg}`);
    },
  });

  const reactivate = useMutation({
    mutationFn: (specName: string): Promise<SpecBucket> => {
      if (!repoPath) return Promise.reject(new Error("Sem projeto selecionado"));
      return reactivateSpec(repoPath, specName);
    },
    onSuccess: (bucket, specName) => {
      toast.success(`Spec ${specName} reativada`);
      if (repoPath) applySpecBucket(queryClient, repoPath, specName, bucket);
    },
    onError: (err: unknown, specName) => {
      const msg = err instanceof Error ? err.message : String(err);
      toast.error(`Falha ao reativar ${specName}: ${msg}`);
    },
  });

  return { complete, cancel, reactivate };
}
