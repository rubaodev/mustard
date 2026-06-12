import { useMemo } from "react";
import { useNavigate } from "react-router";
import { useQuery } from "@tanstack/react-query";
import { AlertTriangle, PauseCircle } from "lucide-react";
import type { LucideIcon } from "lucide-react";
import { cn } from "@/lib/utils";
import { DataCard, SectionHeader } from "@/components/page";
import {
  fetchSpecCards,
  fetchWorkspaceHealth,
  type SpecCard,
} from "@/lib/dashboard";
import { stateFromStatus } from "@/features/specs/_shared/stage-from-status";
import { useT } from "@/lib/i18n";

interface SpecAlertsBandProps {
  repoPath: string;
}

/** A spec is "parada" (stale) when it is still active but has not emitted an
 *  event in this many days. Constant, revisable later (spec non-goal: no
 *  persisted threshold). */
const STALE_DAYS = 7;
const STALE_CUTOFF_MS = STALE_DAYS * 24 * 60 * 60 * 1000;

/** Severity hue an alert takes when it is "hot" (count > 0). Suspeitas are an
 *  error signal (vermelho), specs paradas a warning (âmbar). When cold the pill
 *  falls back to muted/structural grey — cor = significado, cinza = estrutura. */
type AlertTone = "error" | "warning";

interface AlertDef {
  /** `/specs?filter=` target. */
  filterKey: string;
  labelKey: string;
  labelFallback: string;
  icon: LucideIcon;
  tone: AlertTone;
}

const SUSPECTS: AlertDef = {
  filterKey: "suspeitas",
  labelKey: "overview.alerts.suspects",
  labelFallback: "Suspeitas",
  icon: AlertTriangle,
  tone: "error",
};

const STALE: AlertDef = {
  filterKey: "stale",
  labelKey: "overview.alerts.stale",
  labelFallback: "Specs paradas",
  icon: PauseCircle,
  tone: "warning",
};

const HOT_BORDER: Record<AlertTone, string> = {
  error: "border-[--intent-error]/40 bg-[--intent-error]/5 hover:bg-[--intent-error]/10",
  warning: "border-[--intent-warning]/40 bg-[--intent-warning]/5 hover:bg-[--intent-warning]/10",
};
const HOT_BOX: Record<AlertTone, string> = {
  error: "bg-[--intent-error]/10",
  warning: "bg-[--intent-warning]/10",
};
const HOT_TEXT: Record<AlertTone, string> = {
  error: "text-[--intent-error]",
  warning: "text-[--intent-warning]",
};

function AlertPill({
  label,
  count,
  icon: Icon,
  tone,
  onClick,
}: {
  label: string;
  count: number;
  icon: LucideIcon;
  tone: AlertTone;
  onClick: () => void;
}) {
  const hot = count > 0;
  return (
    <button
      type="button"
      onClick={onClick}
      title={label}
      className={cn(
        "flex items-center gap-2 px-3 py-2 rounded-lg border text-left transition-colors",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[--primary]",
        hot ? HOT_BORDER[tone] : "border-border bg-card/40 hover:bg-muted/40",
      )}
    >
      <span
        aria-hidden
        className={cn(
          "inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md",
          hot ? HOT_BOX[tone] : "bg-muted/40",
        )}
      >
        <Icon
          className={cn("h-3.5 w-3.5", hot ? HOT_TEXT[tone] : "text-muted-foreground")}
        />
      </span>
      <span
        className={cn(
          "text-lg font-mono font-medium tabular-nums",
          hot ? HOT_TEXT[tone] : "text-foreground/80",
        )}
      >
        {count}
      </span>
      <span className="text-[12px] text-muted-foreground">{label}</span>
    </button>
  );
}

/**
 * Alerts band for the workspace overview — the attention signals split out of
 * the stage cards (a suspect is an alert, not a stage):
 *   - Suspeitas — active specs with a hygiene flag, from `workspace_health`.
 *   - Specs paradas — active specs with no event in >= 7 days, derived
 *     front-side from each card's `last_event_at` (no new backend).
 * Each pill deep-links to the matching `/specs` filter.
 */
export function SpecAlertsBand({ repoPath }: SpecAlertsBandProps) {
  const t = useT();
  const navigate = useNavigate();

  const { data: health } = useQuery({
    queryKey: ["workspace-health", repoPath],
    queryFn: () => fetchWorkspaceHealth(repoPath),
    enabled: !!repoPath,
    staleTime: 10_000,
  });

  const { data: cards } = useQuery<SpecCard[]>({
    queryKey: ["spec-cards", repoPath],
    queryFn: () => fetchSpecCards(repoPath),
    enabled: !!repoPath,
    staleTime: 10_000,
  });

  const suspectsCount = health?.suspects ?? health?.suspect_specs.length ?? 0;

  // Stale: active specs whose latest activity is older than the cutoff. A card
  // with no timestamp at all is treated as not-stale (no signal to act on).
  const staleCount = useMemo<number>(() => {
    const cutoff = Date.now() - STALE_CUTOFF_MS;
    let n = 0;
    for (const card of cards ?? []) {
      if (stateFromStatus(card.status).outcome !== "active") continue;
      const ts = card.last_event_at ?? card.started_at;
      if (!ts) continue;
      const ms = Date.parse(ts);
      if (Number.isFinite(ms) && ms < cutoff) n += 1;
    }
    return n;
  }, [cards]);

  return (
    <DataCard padded>
      <SectionHeader title={t("overview.alerts.title", "Alertas")} />
      <div className="mt-3 flex flex-wrap gap-2">
        <AlertPill
          label={t(SUSPECTS.labelKey, SUSPECTS.labelFallback)}
          count={suspectsCount}
          icon={SUSPECTS.icon}
          tone={SUSPECTS.tone}
          onClick={() => navigate(`/specs?filter=${SUSPECTS.filterKey}`)}
        />
        <AlertPill
          label={t(STALE.labelKey, STALE.labelFallback)}
          count={staleCount}
          icon={STALE.icon}
          tone={STALE.tone}
          onClick={() => navigate(`/specs?filter=${STALE.filterKey}`)}
        />
      </div>
    </DataCard>
  );
}
