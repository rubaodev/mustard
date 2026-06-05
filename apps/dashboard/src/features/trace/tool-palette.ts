// Tool-type colour palette for the execution trace (claude-devtools colours
// each tool distinctly instead of one uniform amber). Single source of truth
// shared by `ExecutionTrace` (row header icon) and `ToolEventRow` (pill), keyed
// by the tool name carried in `tool.use` payloads (`payload.tool`).
//
//   Bash             → green  (--ds-intent-success)
//   Read             → blue   (--ds-intent-info)
//   Edit/Write/Multi → orange (--ds-intent-warning)
//   Grep/Glob        → purple (--color-phase-qa)
//   Task/Agent       → primary (--ds-accent-primary)
//   Output           → blue   (--ds-intent-info)
//
// The class strings are written out in full (not interpolated) so Tailwind's
// JIT scanner picks them up at build time — the same reason `KIND_ICON_COLOR`
// uses literal `text-[--ds-…]` strings. Unknown tools fall back to the
// caller-supplied default so nothing regresses for shapes we don't recognise.

/** Icon (text-colour) class per tool type — used for the trace row header icon. */
const TOOL_ICON_COLOR: Record<string, string> = {
  Bash: "text-[--ds-intent-success]",
  Read: "text-[--ds-intent-info]",
  Edit: "text-[--ds-intent-warning]",
  Write: "text-[--ds-intent-warning]",
  MultiEdit: "text-[--ds-intent-warning]",
  Grep: "text-[--color-phase-qa]",
  Glob: "text-[--color-phase-qa]",
  Task: "text-[--ds-accent-primary]",
  Agent: "text-[--ds-accent-primary]",
  Output: "text-[--ds-intent-info]",
};

/** Pill class (tinted bg + text) per tool type — used for the `ToolEventRow`
 *  tool-name pill. Mirrors `TOOL_ICON_COLOR` one-to-one. */
const TOOL_PILL_COLOR: Record<string, string> = {
  Bash: "bg-[--ds-intent-success]/15 text-[--ds-intent-success]",
  Read: "bg-[--ds-intent-info]/15 text-[--ds-intent-info]",
  Edit: "bg-[--ds-intent-warning]/15 text-[--ds-intent-warning]",
  Write: "bg-[--ds-intent-warning]/15 text-[--ds-intent-warning]",
  MultiEdit: "bg-[--ds-intent-warning]/15 text-[--ds-intent-warning]",
  Grep: "bg-[--color-phase-qa]/15 text-[--color-phase-qa]",
  Glob: "bg-[--color-phase-qa]/15 text-[--color-phase-qa]",
  Task: "bg-[--ds-accent-primary]/15 text-[--ds-accent-primary]",
  Agent: "bg-[--ds-accent-primary]/15 text-[--ds-accent-primary]",
  Output: "bg-[--ds-intent-info]/15 text-[--ds-intent-info]",
};

/** Text-colour class for a tool's trace icon, falling back to `fallback`
 *  (the per-kind tool colour) when the tool is unknown / absent. */
export function toolIconColorClass(
  tool: string | null | undefined,
  fallback: string,
): string {
  return (typeof tool === "string" && TOOL_ICON_COLOR[tool]) || fallback;
}

/** Pill classes (tinted bg + text) for the `ToolEventRow` tool-name pill,
 *  falling back to `fallback` (the legacy accent pill) for unknown tools. */
export function toolPillColorClass(
  tool: string | null | undefined,
  fallback: string,
): string {
  return (typeof tool === "string" && TOOL_PILL_COLOR[tool]) || fallback;
}
