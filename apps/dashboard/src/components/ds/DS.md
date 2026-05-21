# DS — Design System primitives

Token-driven, presentational-only primitives consumed by Wave 6 (trace viewer)
and Wave 7 (Economia page). No business logic, no data fetching, no Tauri
`invoke()`. All colors flow through `var(--ds-*)` declared in
`apps/dashboard/src/styles/theme.css`.

Storybook is intentionally not used — this file is the spec.

## Tokens

Declared in `theme.css`:

- **Status dots** — `--ds-status-{draft,implementing,awaiting-qa,completed,archived}`
- **Intents** — `--ds-intent-{success,warning,error,info}`
- **Surface tiers** — `--ds-surface-{base,elevated,hover,sunken}`
- **Text ladder** — `--ds-text-{primary,secondary,tertiary,disabled}`
- **Accent** — `--ds-accent-{primary,secondary}` (indigo/violet)
- **Radius** — `--ds-radius-{sm,md,lg}`
- **Spacing** — `--ds-spacing-{1..8}` (4px base)
- **Type** — `--ds-font-{sans,mono}` (Inter / JetBrains Mono — already loaded)

Light values live under `:root`; dark values under `.dark` (matches the
existing `useTheme.ts` switch which toggles `.dark` on `<html>`).

## DiffViewer

```ts
<DiffViewer
  before={"line1\nline2\nline3"}
  after={"line1\nline2 changed\nline3"}
  mode="unified" | "split"   // default: unified
  maxLines={500}              // optional truncation
/>
```

Runs an in-house LCS (O(m·n)) over the two line arrays — no third-party diff
library. Renders `+`/`-`/` ` lines with `--ds-intent-success`/`error` tints.
Algorithm adapted from claude-devtools (MIT, see `NOTICE.md`).

## CodeBlock

```ts
<CodeBlock
  code="fn main() { println!(\"hi\"); }"
  lang="rust" | "ts" | "tsx" | "json" | "sql" | "plain"   // default: plain
  showLineNumbers
/>
```

Regex tokenizer with a small per-language keyword map (~20 each for rust/ts,
JSON literals, SQL clauses). Strings, comments, numbers, and keywords are
colored; everything else falls back to `--ds-text-primary`.

## TreeNode

```ts
<TreeNode
  node={{ label: "root", children: [{ label: "child" }] }}
  defaultExpanded
  onSelect={(node) => console.log(node)}
/>
```

Recursive. Uses native `<details>`/`<summary>` so keyboard / a11y come for
free. Connector lines are CSS-only (`border-l border-dashed`) — no SVG
overlay.

## MetricsPill

```ts
<MetricsPill value="1.2k" unit="tok" intent="success" tooltip="breakdown" />
```

Compact monospace pill. `intent` colors the border only (fill stays at
`--ds-surface-elevated`) so dense lists stay calm.

## BaseRow

```ts
<BaseRow
  icon={<FileText size={14} />}
  label="spec-name"
  summary="3 waves · 12 tasks"
  tokens={1240}
  status="implementing"
  chevron
  onClick={() => navigate("/spec/x")}
/>
```

The atom every list view should compose. Renders icon · status dot · label/summary
· `MetricsPill` · chevron. Hover state via `--ds-surface-hover`. Keyboard-activatable
when `onClick` is passed.
