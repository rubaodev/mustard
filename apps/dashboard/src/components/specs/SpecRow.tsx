import { ChevronRight, ChevronDown, ArrowRight } from "lucide-react";
import { cn } from "@/lib/utils";
import { StageBullet } from "./StageBullet";
import { stateFromStatus } from "./stage-from-status";
import type { SpecCard } from "@/lib/types/specs";

interface SpecRowProps {
  data: SpecCard;
  /** Whether the expandable children tree is open for this spec. */
  expanded: boolean;
  /** Toggle the children tree (chevron click). */
  onToggle: (slug: string) => void;
  /** Open the spec in a new tab / drill-down (row click). */
  onOpen: (slug: string) => void;
}

function formatDuration(ms: number | null): string {
  if (ms == null) return "—";
  const s = Math.round(ms / 1000);
  if (s < 60) return `${s}s`;
  const m = Math.floor(s / 60);
  const rem = s % 60;
  return rem > 0 ? `${m}m ${rem}s` : `${m}m`;
}

/**
 * SpecRow — a dense (32px) Linear-style list row replacing the ~150px
 * `SpecCard`. The leading cluster pairs an expand chevron with a `StageBullet`;
 * the row body shows the spec name (mono), model, wave + AC counters and the
 * duration. Clicking the row (anywhere but the chevron) opens the drill-down;
 * clicking the chevron toggles the inline children tree.
 */
export function SpecRow({ data, expanded, onToggle, onOpen }: SpecRowProps) {
  const state = stateFromStatus(data.status);
  const Chevron = expanded ? ChevronDown : ChevronRight;

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
        "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[--color-accent-mustard]",
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
        className="font-mono text-[12px] text-foreground/90 truncate flex-1 min-w-0"
        title={data.spec}
      >
        {data.spec}
      </span>

      {/* Quantitative columns. Hidden on the narrowest widths so the name keeps
          priority; tabular-nums so counters align column-to-column. */}
      <div
        className="hidden sm:flex items-center gap-4 shrink-0 text-[11px] text-muted-foreground tabular-nums"
        style={{ fontVariantNumeric: "tabular-nums" }}
      >
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

      {/* Trailing affordance — appears on hover/focus. */}
      <ArrowRight
        className="shrink-0 h-3.5 w-3.5 text-muted-foreground/40 opacity-0 transition-opacity group-hover/specrow:opacity-100"
        aria-hidden
      />
    </div>
  );
}
