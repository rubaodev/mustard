import { useMemo } from "react";
import { useNavigate } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { Layers, Play, CheckCircle2 } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { DataCard, SectionHeader } from "@/components/page";
import { fetchSpecCards, type SpecCard } from "@/lib/dashboard";
import { stateFromStatus } from "@/features/specs/_shared/stage-from-status";
import { useT } from "@/lib/i18n";

interface SpecStatusCardsProps {
  repoPath: string;
}

/** Lifecycle-stage bucket the overview groups specs into. Maps onto the
 *  `/specs?filter=` sub-filter param the Specs page reads. */
type StageBucket = "planejando" | "executando" | "finalizadas";

/** Semantic tone per stage — color carries meaning: Planejando→info (azul),
 *  Executando→warning (âmbar), Finalizadas→success (verde). Mapped onto the
 *  design-system intent variables, dessaturated `/10` tonal fill. */
type StageTone = "info" | "warning" | "success";

const TONE: Record<StageTone, { box: string; text: string }> = {
  info: { box: "bg-[--intent-info]/10", text: "text-[--intent-info]" },
  warning: { box: "bg-[--intent-warning]/10", text: "text-[--intent-warning]" },
  success: { box: "bg-[--intent-success]/10", text: "text-[--intent-success]" },
};

interface StageDef {
  bucket: StageBucket;
  labelKey: string;
  labelFallback: string;
  icon: LucideIcon;
  tone: StageTone;
  /** Whether the icon should pulse (Executando = work in flight). */
  pulse?: boolean;
}

const STAGES: StageDef[] = [
  { bucket: "planejando", labelKey: "overview.specStage.planning", labelFallback: "Planejando", icon: Layers, tone: "info" },
  { bucket: "executando", labelKey: "overview.specStage.executing", labelFallback: "Executando", icon: Play, tone: "warning", pulse: true },
  { bucket: "finalizadas", labelKey: "overview.specStage.finished", labelFallback: "Finalizadas", icon: CheckCircle2, tone: "success" },
];

/**
 * Project one spec card onto its overview stage bucket. Uses the same
 * `stateFromStatus` lift the Specs page uses, so a card lands in the same
 * stage the list would group it under:
 *   - terminal outcome (completed/cancelled/…)  → finalizadas
 *   - active + execute stage                    → executando
 *   - active + analyze/plan/qa-review/close     → planejando
 */
function bucketForCard(card: SpecCard): StageBucket {
  const state = stateFromStatus(card.status);
  if (state.outcome !== "active") return "finalizadas";
  if (state.stage === "execute") return "executando";
  return "planejando";
}

function StageCard({
  label,
  count,
  icon: Icon,
  tone,
  pulse,
  onClick,
}: {
  label: string;
  count: number;
  icon: LucideIcon;
  tone: StageTone;
  pulse?: boolean;
  onClick: () => void;
}) {
  const c = TONE[tone];
  return (
    <button
      type="button"
      onClick={onClick}
      title={label}
      className={cn(
        "flex flex-col gap-1.5 px-3 py-2.5 rounded-lg border border-border bg-card/40 text-left",
        "transition-colors hover:bg-muted/40",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[--primary]",
      )}
    >
      <div className="flex items-center gap-2 text-muted-foreground">
        <span
          aria-hidden
          className={cn(
            "inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md",
            c.box,
          )}
        >
          <Icon
            className={cn("h-3.5 w-3.5", c.text, pulse && "animate-pulse")}
          />
        </span>
        <span className="text-[11px] uppercase tracking-wider">{label}</span>
      </div>
      <span className="text-2xl font-mono font-medium text-foreground tabular-nums">{count}</span>
    </button>
  );
}

/**
 * Three stage cards (Planejando · Executando · Finalizadas) for the workspace
 * overview. Counts are derived front-side from the batch `fetchSpecCards`
 * projection (no new backend); each card deep-links to `/specs?filter=<stage>`.
 */
export function SpecStatusCards({ repoPath }: SpecStatusCardsProps) {
  const t = useT();
  const navigate = useNavigate();

  const { data } = useQuery<SpecCard[]>({
    queryKey: ["spec-cards", repoPath],
    queryFn: () => fetchSpecCards(repoPath),
    enabled: !!repoPath,
    staleTime: 10_000,
  });

  const counts = useMemo<Record<StageBucket, number>>(() => {
    const acc: Record<StageBucket, number> = { planejando: 0, executando: 0, finalizadas: 0 };
    for (const card of data ?? []) acc[bucketForCard(card)] += 1;
    return acc;
  }, [data]);

  return (
    <DataCard padded>
      <SectionHeader title={t("overview.specs.title", "Specs")} />
      <div className="mt-3 grid grid-cols-3 gap-2">
        {STAGES.map((s) => (
          <StageCard
            key={s.bucket}
            label={t(s.labelKey, s.labelFallback)}
            count={counts[s.bucket]}
            icon={s.icon}
            tone={s.tone}
            pulse={s.pulse}
            onClick={() => navigate(`/specs?filter=${s.bucket}`)}
          />
        ))}
      </div>
    </DataCard>
  );
}
