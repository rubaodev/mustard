// Barrel for the Design System primitives. Page-level consumers should
// import from `@/components/ds` rather than the individual files.

export { DiffViewer } from "./DiffViewer";
export type { DiffViewerProps, DiffMode } from "./DiffViewer";

export { CodeBlock } from "./CodeBlock";
export type { CodeBlockProps, CodeLang } from "./CodeBlock";

export { TreeNode } from "./TreeNode";
export type { TreeNodeProps, TreeNodeData } from "./TreeNode";

export { MetricsPill } from "./MetricsPill";
export type { MetricsPillProps, Intent as MetricsIntent } from "./MetricsPill";

export { BaseRow } from "./BaseRow";
export type { BaseRowProps, RowStatus } from "./BaseRow";
