import { ChevronRight, ChevronDown, ArrowRight, RefreshCw } from "lucide-react";
import { cn } from "@/lib/utils";
import { useT } from "@/lib/i18n";
import { StageBullet } from "../StageBullet";
import { SpecBadge } from "../SpecBadge";
import { stateFromStatus } from "../_shared/stage-from-status";
import { useSpecPlanStaleness } from "@/hooks/useSpecPlanStaleness";
import type { SpecCard } from "@/lib/types/specs";

/** Which set of quantitative columns a row (and its group header) renders. */
export type SpecRowVariant = "planning" | "default";

interface SpecRowProps {
  data: SpecCard;
  /** Whether the expandable children tree is open for this spec. */
  expanded: boolean;
  /** Toggle the children tree (chevron click). */
  onToggle: (slug: string) => void;
  /** Open the spec in a new tab / drill-down (row click). */
  onOpen: (slug: string) => void;
  /**
   * Column set for this row (spec `melhorias-pagina-specs`, item 3). `"default"`
   * shows model / waves / ACs / duration. `"planning"` swaps those for
   * "Criada em" + "Parada há" (a planning spec ran nothing, so the metric
   * columns would always be empty) and exposes the "Reanalisar" affordance.
   */
  variant?: SpecRowVariant;
  /**
   * Active project path — only consumed by the `"planning"` variant for the
   * on-demand staleness check. `null` disables the "Reanalisar" button.
   */
  repoPath?: string | null;
  /**
   * Wave-6: set of spec slugs flagged as suspects by the hygiene hook
   * (`hygiene.detected` in the last 7 days, still active). When provided,
   * matching rows get a `suspect` badge. Passed down from `Specs.tsx` which
   * holds the `workspace_health` query result.
   */
  suspectSpecs?: ReadonlySet<string>;
  /**
   * Wave-6: set of spec slugs that were auto-closed today (`hygiene.autoclose`
   * in the last 24h). Used to render the `auto-closed` badge in the
   * "Encerradas" bucket.
   */
  autoClosedSpecs?: ReadonlySet<string>;
}

function formatDuration(ms: number | null): string {
  if (ms == null) return "—";
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  const rem = s % 60;
  return rem > 0 ? `${m}m ${rem}s` : `${m}m`;
}

/** Short local date `dd/MM/yyyy` from an ISO timestamp; `—` for null/unparseable. */
function formatShortDate(iso: string | null): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "—";
  const dd = String(d.getDate()).padStart(2, "0");
  const mm = String(d.getMonth() + 1).padStart(2, "0");
  return `${dd}/${mm}/${d.getFullYear()}`;
}

/** Whole days between `iso` and now as `Xd`; `hoje` when under a day; `—` for
 *  null/unparseable. Uses `new Date()` at render time (frontend, no fixed clock). */
function formatStalledFor(iso: string | null): string {
  if (!iso) return "—";
  const t = Date.parse(iso);
  if (!Number.isFinite(t)) return "—";
  const days = Math.floor((Date.now() - t) / (24 * 60 * 60 * 1000));
  if (days < 1) return "hoje";
  return `${days}d`;
}

/**
 * Discreet column-header line for a group of `SpecRow`s (spec
 * `melhorias-pagina-specs`, item 2). Mirrors the row's quantitative-column
 * layout (same widths / alignment / `tabular-nums`) so the labels sit directly
 * over the values, and follows the row `variant` so the labels match the
 * columns actually shown. Rendered once per group by `Specs.tsx`, indented to
 * clear the row's leading chevron + stage bullet.
 */
export function SpecRowColumnsHeader({
  variant = "default",
}: {
  variant?: SpecRowVariant;
}) {
  const t = useT();
  return (
    <div
      className="hidden sm:flex items-center gap-2 h-5 px-4 select-none"
      aria-hidden
    >
      {/* Spacer matching the row's chevron (h-5 w-5) + gap + stage bullet. */}
      <span className="shrink-0 w-5" />
      <span className="shrink-0 w-2" />
      {/* Spacer for the spec-name column (flex-1). */}
      <span className="flex-1 min-w-0" />
      <div
        className="flex items-center gap-4 shrink-0 text-[10px] uppercase tracking-wide text-muted-foreground/60 tabular-nums"
        style={{ fontVariantNumeric: "tabular-nums" }}
      >
        {variant === "planning" ? (
          <>
            <span className="w-20 text-right">{t("specs.col.created")}</span>
            <span className="w-16 text-right">{t("specs.col.stalledFor")}</span>
          </>
        ) : (
          <>
            <span className="w-20 text-right">{t("specs.col.created")}</span>
            <span className="hidden md:inline max-w-[110px]">
              {t("specs.col.model")}
            </span>
            <span className="w-12 text-right">{t("specs.col.waves")}</span>
            <span className="w-12 text-right">{t("specs.col.ac")}</span>
            <span className="w-12 text-right">{t("specs.col.duration")}</span>
          </>
        )}
      </div>
      {/* Spacer matching the row's trailing ArrowRight (h-3.5 w-3.5). */}
      <span className="shrink-0 w-3.5" />
    </div>
  );
}

/**
 * Inline "Reanalisar" affordance + verdict badge for a planning row (spec
 * `melhorias-pagina-specs`, item 4). The click runs the deterministic
 * `dashboard_spec_plan_staleness` command and renders the verdict inline:
 * `obsoleto` (stale, tooltip lists missing/changed), `ok` (fresh), `?`
 * (unknown). The button stops propagation so it never opens the spec.
 */
function PlanStalenessControl({
  repoPath,
  data,
}: {
  repoPath: string | null;
  data: SpecCard;
}) {
  const t = useT();
  const { mutate, data: result, isPending } = useSpecPlanStaleness();
  const disabled = !repoPath || isPending;

  // Plan-date / age context shared by every verdict tooltip — `age_days` and
  // `plan_date` are reported as evidence even though age no longer drives the
  // `stale` verdict (Rust side).
  const contextLines = (() => {
    if (!result) return [] as string[];
    const lines: string[] = [];
    if (result.plan_date) {
      lines.push(`${t("specs.staleness.planDate")}: ${formatShortDate(result.plan_date)}`);
    }
    lines.push(
      `${t("specs.staleness.age")}: ${result.age_days} ${t("specs.staleness.days")}`,
    );
    return lines;
  })();

  const verdictBadge = (() => {
    if (!result) return null;
    if (result.verdict === "stale") {
      // Tooltip lists the evidence that drove the verdict: which census files
      // went missing and which changed on disk (the only triggers now), plus
      // the plan date / age as context.
      const sections: string[] = [];
      if (result.missing.length > 0) {
        sections.push(
          `${t("specs.staleness.missing")}:\n${result.missing.map((f) => `  ${f}`).join("\n")}`,
        );
      }
      if (result.changed.length > 0) {
        sections.push(
          `${t("specs.staleness.changed")}:\n${result.changed.map((f) => `  ${f}`).join("\n")}`,
        );
      }
      const detail = [...sections, ...contextLines].join("\n");
      return (
        <span
          className="inline-flex items-center shrink-0 rounded-sm text-[11px] font-medium leading-none px-1.5 py-[3px] bg-amber-950/60 text-amber-400 border border-amber-800/50"
          title={detail || t("specs.staleness.stale")}
        >
          {t("specs.staleness.stale")}
        </span>
      );
    }
    if (result.verdict === "fresh") {
      return (
        <span
          className="inline-flex items-center shrink-0 rounded-sm text-[11px] font-medium leading-none px-1.5 py-[3px] bg-emerald-950/60 text-emerald-400 border border-emerald-800/50"
          title={contextLines.join("\n") || t("specs.staleness.fresh")}
        >
          {t("specs.staleness.fresh")}
        </span>
      );
    }
    return (
      <span
        className="inline-flex items-center shrink-0 rounded-sm text-[11px] font-medium leading-none px-1.5 py-[3px] bg-slate-800/60 text-slate-400 border border-slate-700/50"
        title={result.reason || t("specs.staleness.unknown")}
      >
        {t("specs.staleness.unknown")}
      </span>
    );
  })();

  return (
    <div className="hidden sm:flex items-center gap-1.5 shrink-0">
      {verdictBadge}
      <button
        type="button"
        disabled={disabled}
        onClick={(e) => {
          e.stopPropagation();
          if (!repoPath) return;
          mutate({ repoPath, spec: data.spec, startedAt: data.started_at });
        }}
        title={t("specs.staleness.button")}
        aria-label={t("specs.staleness.button")}
        className={cn(
          "inline-flex items-center gap-1 rounded px-1.5 py-[3px] text-[11px]",
          "text-muted-foreground hover:text-foreground hover:bg-muted/40 transition-colors",
          "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[--primary]",
          disabled && "opacity-50 pointer-events-none",
        )}
      >
        <RefreshCw
          className={cn("h-3 w-3", isPending && "animate-spin")}
          aria-hidden
        />
        <span>
          {isPending
            ? t("specs.staleness.checking")
            : t("specs.staleness.button")}
        </span>
      </button>
    </div>
  );
}

/**
 * SpecRow — a dense (32px) Linear-style list row replacing the ~150px
 * `SpecCard`. The leading cluster pairs an expand chevron with a `StageBullet`;
 * the row body shows the spec name (mono), then a `variant`-dependent column
 * cluster: `"default"` → model, wave + AC counters, duration; `"planning"` →
 * Criada em + Parada há + the "Reanalisar" affordance. Clicking the row
 * (anywhere but the chevron / Reanalisar button) opens the drill-down;
 * clicking the chevron toggles the inline children tree.
 */
export function SpecRow({
  data,
  expanded,
  onToggle,
  onOpen,
  variant = "default",
  repoPath,
  suspectSpecs,
  autoClosedSpecs,
}: SpecRowProps) {
  const state = stateFromStatus(data.status);
  const Chevron = expanded ? ChevronDown : ChevronRight;

  // Compute which badges to render for this row (right of the name).
  // Order: blocked → wave-failed → followup → suspect → auto-closed.
  const badges: Array<"blocked" | "wave-failed" | "followup" | "suspect" | "auto-closed"> = [];
  if (state.flags.blocked) badges.push("blocked");
  if (state.flags.wave_failed) badges.push("wave-failed");
  if (state.flags.followup_open) badges.push("followup");
  if (suspectSpecs?.has(data.spec)) badges.push("suspect");
  if (autoClosedSpecs?.has(data.spec)) badges.push("auto-closed");

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => onOpen(data.spec)}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onOpen(data.spec);
        }
      }}
      className={cn(
        "group/specrow flex items-center gap-2 h-8 px-4 rounded-md",
        "cursor-pointer transition-colors hover:bg-muted/30",
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[--primary]",
      )}
    >
      {/* Leading: chevron + stage bullet. */}
      <button
        type="button"
        onClick={(e) => {
          e.stopPropagation();
          onToggle(data.spec);
        }}
        aria-label={expanded ? "Recolher" : "Expandir"}
        aria-expanded={expanded}
        className="shrink-0 grid place-items-center h-5 w-5 rounded text-muted-foreground/50 hover:text-muted-foreground hover:bg-muted/40 transition-colors"
      >
        <Chevron className="h-3.5 w-3.5" aria-hidden />
      </button>
      <StageBullet
        stage={state.stage}
        outcome={state.outcome}
        flags={state.flags}
      />

      {/* Spec name — mono, truncates at the end. */}
      <span
        className="font-mono text-[12px] text-foreground/90 truncate min-w-0"
        style={{ flex: "1 1 0%" }}
        title={data.spec}
      >
        {data.spec}
      </span>

      {/* Wave-6 hygiene badges — right of the name, before metric columns. */}
      {badges.length > 0 && (
        <div className="hidden sm:flex items-center gap-1 shrink-0">
          {badges.map((badgeVariant) => (
            <SpecBadge key={badgeVariant} variant={badgeVariant} />
          ))}
        </div>
      )}

      {variant === "planning" ? (
        <>
          {/* Planning columns: Criada em + Parada há. A planning spec ran
              nothing, so model/waves/ACs/duration would always be empty. */}
          <div
            className="hidden sm:flex items-center gap-4 shrink-0 text-[11px] text-muted-foreground tabular-nums"
            style={{ fontVariantNumeric: "tabular-nums" }}
          >
            <span title="Criada em" className="w-20 text-right">
              {formatShortDate(data.started_at)}
            </span>
            <span title="Parada há" className="w-16 text-right">
              {formatStalledFor(data.last_event_at ?? data.started_at)}
            </span>
          </div>
          <PlanStalenessControl repoPath={repoPath ?? null} data={data} />
        </>
      ) : (
        /* Quantitative columns. Hidden on the narrowest widths so the name keeps
           priority; tabular-nums so counters align column-to-column. The
           "Criada" date leads the cluster (shown for every spec, not just the
           planning variant). */
        <div
          className="hidden sm:flex items-center gap-4 shrink-0 text-[11px] text-muted-foreground tabular-nums"
          style={{ fontVariantNumeric: "tabular-nums" }}
        >
          <span title="Criada" className="w-20 text-right">
            {formatShortDate(data.started_at)}
          </span>
          <span
            className="hidden md:inline font-mono text-foreground/60 truncate max-w-[110px]"
            title={data.model ?? "modelo desconhecido"}
          >
            {data.model ?? "—"}
          </span>
          <span title="Ondas" className="w-12 text-right">
            {data.current_wave ?? "—"}/{data.total_waves ?? "—"}
          </span>
          <span title="Critérios de aceitação" className="w-12 text-right">
            {data.ac_passed}/{data.ac_total}
          </span>
          <span title="Duração" className="w-12 text-right">
            {formatDuration(data.duration_ms)}
          </span>
        </div>
      )}

      {/* Trailing affordance — appears on hover/focus. */}
      <ArrowRight
        className="shrink-0 h-3.5 w-3.5 text-muted-foreground/40 opacity-0 transition-opacity group-hover/specrow:opacity-100"
        aria-hidden
      />
    </div>
  );
}
