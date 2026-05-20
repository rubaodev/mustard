import { useMemo } from "react";
import { useQueries, useQuery } from "@tanstack/react-query";
import { useStore } from "@/lib/store";
import {
  useProjects,
  fetchTelemetry,
  fetchSpecs,
  dashboardMetricsWaveStatus,
  type MetricsWaveStatus,
} from "@/lib/dashboard";
import { useTelemetryPhases } from "@/hooks/useTelemetryPhases";
import { usePromptEconomy } from "@/hooks/usePromptEconomy";
import { PageHeader, EmptyState } from "@/components/page";
import { EconomySection } from "@/components/telemetry/EconomySection";
import { Badge } from "@/components/ui/badge";

/**
 * Wave-4 helper: fan-out one `metrics wave-status` query per parent spec.
 * Stays inline to keep wave-4 scope to the six files in § Limites — no new
 * `hooks/use*.ts` file. Follows the dashboard's `useQueries` convention.
 */
function useWaveStatusQueries(repoPath: string | null, parentNames: string[]) {
  return useQueries({
    queries: parentNames.map((name) => ({
      queryKey: ["metrics-wave-status", repoPath, name] as const,
      queryFn: (): Promise<MetricsWaveStatus> =>
        dashboardMetricsWaveStatus(repoPath as string, name),
      enabled: !!repoPath,
      staleTime: 15_000,
    })),
  });
}

const WAVE_STATUS_COLOR: Record<string, string> = {
  completed: "bg-[--color-ok]/15 text-[--color-ok]",
  draft: "bg-muted text-muted-foreground",
  implementing: "bg-[--color-accent-mustard]/15 text-[--color-accent-mustard]",
  blocked: "bg-[--color-error]/15 text-[--color-error]",
  failed: "bg-[--color-error]/15 text-[--color-error]",
};

function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "—";
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}

function formatDurationMs(n: number): string {
  if (!Number.isFinite(n) || n <= 0) return "—";
  const s = Math.round(n / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  const rem = s % 60;
  return rem ? `${m}m ${rem}s` : `${m}m`;
}

export function Economia() {
  const projectsRoot = useStore((s) => s.projectsRoot);
  const activeWorkspaceId = useStore((s) => s.activeWorkspaceId);
  const projects = useProjects();
  const activeProject = projects.find((p) => p.id === activeWorkspaceId) ?? null;
  const repoPath = activeProject?.path ?? null;

  const telemetry = useQuery({
    queryKey: ["telemetry", repoPath],
    queryFn: () => fetchTelemetry(repoPath!),
    enabled: !!repoPath,
    staleTime: 30_000,
    refetchInterval: 30_000,
  });

  const phases = useTelemetryPhases(repoPath, "all");
  const promptEconomy = usePromptEconomy(repoPath);

  // Wave 4 (2026-05-20, spec mustard-wave-network-standard) — list specs that
  // own a wave-plan and resolve `metrics wave-status` for each. The Tauri
  // command is fail-soft (always resolves), so the React Query layer never
  // surfaces a hard error. `parents` is derived: a spec is a parent when
  // another row references it via `row.parent`.
  const specs = useQuery({
    queryKey: ["specs", repoPath],
    queryFn: () => fetchSpecs(repoPath!),
    enabled: !!repoPath,
    staleTime: 30_000,
  });

  const parentSpecs = useMemo(() => {
    const rows = specs.data ?? [];
    const parentNames = new Set<string>();
    for (const r of rows) {
      if (r.parent) parentNames.add(r.parent);
    }
    // Only surface parents that are still active (Economia is workspace-oriented).
    return rows.filter((r) => parentNames.has(r.name) && r.bucket === "active");
  }, [specs.data]);

  // One useQuery per parent — small N (waves of waves), so fan-out is cheap.
  // We use `useQueries` from TanStack Query v5 to stay aligned with the
  // dashboard's per-project fan-out convention.
  const waveStatusQueries = useWaveStatusQueries(repoPath, parentSpecs.map((p) => p.name));

  if (!projectsRoot) {
    return (
      <div className="flex flex-col gap-6 w-full">
        <PageHeader
          breadcrumb={[{ label: "Workspace" }, { label: "Economia" }]}
          title="Economia"
          subtitle="Tokens, cache, e economia agregada"
        />
        <EmptyState
          title="Diretório de projetos não configurado"
          description="Vá em Configurações e aponte para a pasta onde estão seus repos."
        />
      </div>
    );
  }

  if (!activeWorkspaceId) {
    return (
      <div className="flex flex-col gap-6 w-full">
        <PageHeader
          breadcrumb={[{ label: "Workspace" }, { label: "Economia" }]}
          title="Economia"
          subtitle="Tokens, cache, e economia agregada"
        />
        <EmptyState
          title="Selecione um workspace"
          description="Use o seletor na sidebar para escolher um projeto."
        />
      </div>
    );
  }

  // AC-14: explicit empty state when `telemetry.data?.rtk?.available === false`.
  // Renders at the top so the other channels (measured/prevention/routing/phases/
  // promptEconomy) keep showing normally below — we never gate the whole page on
  // RTK availability. Phrase contains "RTK" and "indispon" to match the spec regex.
  const rtkUnavailable = telemetry.data?.rtk?.available === false;

  return (
    <div className="flex flex-col gap-6 w-full">
      <PageHeader
        breadcrumb={[{ label: "Workspace" }, { label: "Economia" }]}
        title="Economia"
        subtitle="Tokens, cache, e economia agregada"
      />
      {rtkUnavailable && (
        <EmptyState
          variant="warning"
          title="RTK não está disponível neste sistema"
          description={
            <>
              O processo do dashboard não encontrou o binário <code className="font-mono">rtk</code>{" "}
              no PATH — por isso a economia medida pelo RTK aparece como indisponível.
              Outros canais (custo medido, hooks, roteamento, economia de prompts) continuam
              funcionando abaixo.{" "}
              <a
                href="https://github.com/rust-token-killer/rtk"
                target="_blank"
                rel="noreferrer"
                className="underline hover:text-foreground"
              >
                Como instalar o RTK
              </a>
              .
            </>
          }
        />
      )}
      <EconomySection
        rtk={
          telemetry.data?.rtk ?? {
            available: false,
            total_commands: null,
            input_tokens: null,
            output_tokens: null,
            tokens_saved: null,
            savings_pct: null,
            total_exec_time_ms: null,
            daily: [],
          }
        }
        measured={telemetry.data?.measured ?? { tokens_total: 0, tokens_today: 0 }}
        prevention={telemetry.data?.prevention ?? []}
        routing={
          telemetry.data?.routing ?? {
            blocks: 0,
            allows: 0,
            by_intent: [],
            by_note: [],
            session_blocks: 0,
            session_allows: 0,
          }
        }
        phases={phases.data ?? []}
        promptEconomy={promptEconomy.data}
      />

      {/* Wave-4 (2026-05-20, spec mustard-wave-network-standard): per-parent
       *  wave-status. Renders one expandable block per active parent spec;
       *  each row is a wave with status pill + tokens_saved + duration +
       *  retries + cross_wave_memory bytes. Empty parents are hidden so the
       *  section disappears entirely when no wave-plan specs are active. */}
      {parentSpecs.length > 0 && (
        <section className="flex flex-col gap-3">
          <div className="flex items-center justify-between">
            <h2 className="text-sm font-medium">Ondas por parent</h2>
            <span className="text-[11px] text-muted-foreground">
              fonte: <code className="font-mono">metrics_wave_status</code>
            </span>
          </div>
          <div className="flex flex-col gap-2">
            {parentSpecs.map((parent, idx) => {
              const q = waveStatusQueries[idx];
              const data = q?.data;
              const waves = data?.waves ?? [];
              return (
                <details
                  key={parent.name}
                  open
                  className="rounded-lg border border-border bg-card/20 px-3 py-2"
                >
                  <summary className="cursor-pointer text-[13px] font-mono font-medium flex items-center gap-2">
                    <span className="truncate">{parent.name}</span>
                    <Badge variant="outline" className="text-[10px] py-0">
                      {waves.length} ondas
                    </Badge>
                  </summary>
                  {q?.isLoading ? (
                    <p className="text-[11px] text-muted-foreground mt-2">carregando…</p>
                  ) : waves.length === 0 ? (
                    <p className="text-[11px] text-muted-foreground mt-2">
                      sem dados de wave-status para este parent.
                    </p>
                  ) : (
                    <ul className="mt-2 flex flex-col gap-1 text-[12px]">
                      {waves.map((w) => {
                        const statusKey = w.status ?? "draft";
                        const cls =
                          WAVE_STATUS_COLOR[statusKey] ??
                          "bg-muted text-muted-foreground";
                        return (
                          <li
                            key={w.name}
                            className="flex items-center gap-3 px-2 py-1 rounded hover:bg-muted/40"
                          >
                            <span className="font-mono truncate flex-1 min-w-0">
                              {w.name}
                            </span>
                            <span
                              className={`text-[10px] font-medium px-1.5 py-0.5 rounded uppercase tracking-wide ${cls}`}
                              title={statusKey}
                            >
                              {statusKey}
                            </span>
                            <span
                              className="text-muted-foreground tabular-nums"
                              style={{ fontVariantNumeric: "tabular-nums" }}
                              title="tokens economizados"
                            >
                              tokens {w.tokens_saved}
                            </span>
                            <span
                              className="text-muted-foreground tabular-nums"
                              style={{ fontVariantNumeric: "tabular-nums" }}
                              title="duração"
                            >
                              {formatDurationMs(w.duration_ms)}
                            </span>
                            <span
                              className="text-muted-foreground tabular-nums"
                              style={{ fontVariantNumeric: "tabular-nums" }}
                              title="retries"
                            >
                              retries {w.retries}
                            </span>
                            <span
                              className="text-muted-foreground tabular-nums"
                              style={{ fontVariantNumeric: "tabular-nums" }}
                              title="cross-wave memory"
                            >
                              cw-mem {formatBytes(w.cross_wave_memory_bytes)}
                            </span>
                            {w.model && (
                              <span className="font-mono text-[10px] text-muted-foreground/50">
                                {w.model}
                              </span>
                            )}
                          </li>
                        );
                      })}
                    </ul>
                  )}
                </details>
              );
            })}
          </div>
        </section>
      )}
    </div>
  );
}
