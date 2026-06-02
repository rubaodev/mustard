<!-- mustard:generated -->
# Stack — `apps/dashboard`

<!-- mustard:enrich hash=d9d86f9676fb -->
## Purpose

The `dashboard` subproject is a Tauri desktop app: a React 19.1 + TypeScript 5.8 frontend (124 `.tsx`, 79 `.ts`) over a Rust backend (26 `.rs`), styled with Tailwind 4.3. The dependency set confirms the shape — `@tauri-apps/api` plus plugins (store, dialog, log, updater, window-state) bridge to the native side, `@tanstack/react-query` drives data fetching in the `use` hooks, `react-router` handles navigation, and `radix-ui` / `shadcn` / `lucide-react` / `cmdk` / `sonner` make up the component layer. Source splits into 2 clusters (`use`, `tauri`) across 254 files.
<!-- /mustard:enrich -->

## Manifests
- `package.json`

## Dependencies
- `@fontsource-variable/geist` (from `package.json`)
- `@fontsource/ibm-plex-mono` (from `package.json`)
- `@fontsource-variable/inter` (from `package.json`)
- `@fontsource/inter` (from `package.json`)
- `@tanstack/react-query` (from `package.json`)
- `@tauri-apps/api` (from `package.json`)
- `@tauri-apps/plugin-dialog` (from `package.json`)
- `@tauri-apps/plugin-log` (from `package.json`)
- `@tauri-apps/plugin-opener` (from `package.json`)
- `@tauri-apps/plugin-store` (from `package.json`)
- `@tauri-apps/plugin-updater` (from `package.json`)
- `@tauri-apps/plugin-window-state` (from `package.json`)
- `class-variance-authority` (from `package.json`)
- `clsx` (from `package.json`)
- `cmdk` (from `package.json`)
- `dayjs` (from `package.json`)
- `i18next` (from `package.json`)
- `lucide-react` (from `package.json`)
- `radix-ui` (from `package.json`)
- `react` (from `package.json`)
- `react-dom` (from `package.json`)
- `react-i18next` (from `package.json`)
- `react-markdown` (from `package.json`)
- `react-router` (from `package.json`)
- `react-virtuoso` (from `package.json`)
- `remark-gfm` (from `package.json`)
- `shadcn` (from `package.json`)
- `sonner` (from `package.json`)
- `tailwind-merge` (from `package.json`)
- `tw-animate-css` (from `package.json`)
- ... 17 more

## Source extensions
- `.tsx` — 124
- `.ts` — 79
- `.rs` — 26
- `.json` — 11
- `.md` — 4
- `.css` — 2

## Clusters
- 2 clusters across 254 source files
- `use`