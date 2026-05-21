/**
 * Wave 4 (spec `2026-05-21-dashboard-spec-tabs-polish`): canonical color
 * palette for the 5+1 Mustard pipeline phases. The map is the single source
 * of truth for the `<PipelineTimeline>` chips and the `<PhaseChip>` pill —
 * same hue everywhere so the user builds visual association.
 *
 * Each entry is a bag of Tailwind 4 utility classes (background tint, text
 * color, border, and a ring color used to highlight the active phase). The
 * `execute` row maps to the canonical mustard accent (`--color-accent-mustard`)
 * which already drives "in progress" affordances across the app.
 *
 * Lower-case keys mirror the harness `pipeline.phase` event value
 * (`analyze`, `plan`, …); `phaseColor()` normalizes case so callers can pass
 * either `"ANALYZE"` or `"analyze"` without thinking.
 */
export const PHASE_COLORS: Record<
  string,
  { bg: string; text: string; border: string; ring: string }
> = {
  analyze: {
    bg: "bg-sky-500/15",
    text: "text-sky-400",
    border: "border-sky-500/30",
    ring: "ring-sky-500/40",
  },
  plan: {
    bg: "bg-violet-500/15",
    text: "text-violet-400",
    border: "border-violet-500/30",
    ring: "ring-violet-500/40",
  },
  // Wave 1 (spec `2026-05-21-dashboard-i18n-and-phase-unify`): EXECUTE moves
  // from mustard accent to a brighter green (`green-500`) for higher contrast
  // against the active-pipeline pulse. Tint is bumped to /20 + ring /50 so the
  // active station reads as the most energetic moment of the run.
  execute: {
    bg: "bg-green-500/20",
    text: "text-green-400",
    border: "border-green-500/40",
    ring: "ring-green-500/50",
  },
  // Wave 1: REVIEW moves from teal to amber. Two adjacent greens (teal review
  // + emerald qa) read as the same hue at a glance; amber separates the two
  // verification phases visually.
  review: {
    bg: "bg-amber-500/15",
    text: "text-amber-400",
    border: "border-amber-500/30",
    ring: "ring-amber-500/40",
  },
  qa: {
    bg: "bg-emerald-500/15",
    text: "text-emerald-400",
    border: "border-emerald-500/30",
    ring: "ring-emerald-500/40",
  },
  close: {
    bg: "bg-slate-500/15",
    text: "text-slate-400",
    border: "border-slate-500/30",
    ring: "ring-slate-500/40",
  },
};

export function phaseColor(phase: string) {
  return PHASE_COLORS[phase.toLowerCase()] ?? PHASE_COLORS.close;
}
