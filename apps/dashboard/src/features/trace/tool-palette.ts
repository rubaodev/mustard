// Tool-type colour palette for the execution trace (claude-devtools colours
// each tool distinctly instead of one uniform amber). Single source of truth
// shared by `ExecutionTrace` (row header icon) and `ToolEventRow` (pill), keyed
// by the tool name carried in `tool.use` payloads (`payload.tool`).
//
//   Bash             → green  (--intent-success)
//   Read             → blue   (--intent-info)
//   Edit/Write/Multi → orange (--intent-warning)
//   Grep/Glob        → purple (--color-phase-qa)
//   Task/Agent       → primary (--primary)
//   Output           → blue   (--intent-info)
//
// The class strings are written out in full (not interpolated) so Tailwind's
// JIT scanner picks them up at build time — the same reason `KIND_ICON_COLOR`
// uses literal `text-[--…]` strings. Unknown tools fall back to the
// caller-supplied default so nothing regresses for shapes we don't recognise.

import {
  Terminal,
  FileText,
  Pencil,
  FilePlus,
  FilePenLine,
  Search,
  FolderSearch,
  Bot,
  Send,
  type LucideIcon,
} from "lucide-react";

/** Icon (text-colour) class per tool type — used for the trace row header icon. */
const TOOL_ICON_COLOR: Record<string, string> = {
  Bash: "text-intent-success",
  Read: "text-intent-info",
  Edit: "text-intent-warning",
  Write: "text-intent-warning",
  MultiEdit: "text-intent-warning",
  Grep: "text-violet-400",
  Glob: "text-violet-400",
  Task: "text-primary",
  Agent: "text-primary",
  Output: "text-intent-info",
};

/** Pill class (tinted bg + text) per tool type — used for the `ToolEventRow`
 *  tool-name pill. Mirrors `TOOL_ICON_COLOR` one-to-one. */
const TOOL_PILL_COLOR: Record<string, string> = {
  Bash: "bg-intent-success/15 text-intent-success",
  Read: "bg-intent-info/15 text-intent-info",
  Edit: "bg-intent-warning/15 text-intent-warning",
  Write: "bg-intent-warning/15 text-intent-warning",
  MultiEdit: "bg-intent-warning/15 text-intent-warning",
  Grep: "bg-violet-400/15 text-violet-400",
  Glob: "bg-violet-400/15 text-violet-400",
  Task: "bg-primary/15 text-primary",
  Agent: "bg-primary/15 text-primary",
  Output: "bg-intent-info/15 text-intent-info",
};

/** Lucide icon component per tool type — used so the trace row header icon
 *  conveys WHAT the tool does at a glance, not just a colour. Mirrors the
 *  colour tables above one-to-one. Unknown tools fall back to `Wrench` via
 *  `toolIcon`. */
const TOOL_ICON: Record<string, LucideIcon> = {
  Bash: Terminal,
  Read: FileText,
  Edit: Pencil,
  Write: FilePlus,
  MultiEdit: FilePenLine,
  Grep: Search,
  Glob: FolderSearch,
  Task: Bot,
  Agent: Bot,
  Output: Send,
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

/** Lucide icon component for a tool's trace row, falling back to `fallback`
 *  (e.g. the generic `Wrench`) when the tool is unknown / absent. The icon
 *  conveys the tool TYPE (Bash→Terminal, Read→FileText, …) so the tree reads
 *  at a glance instead of a wall of identical wrenches. */
export function toolIcon(
  tool: string | null | undefined,
  fallback: LucideIcon,
): LucideIcon {
  return (typeof tool === "string" && TOOL_ICON[tool]) || fallback;
}
