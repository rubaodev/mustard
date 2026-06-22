/**
 * Heuristics to extract a likely test-file path from an Acceptance-Criterion
 * command. Used by `SpecQualityTab` (Wave 4 of spec
 * `2026-05-21-dashboard-spec-tabs`) to render an "abrir arquivo de teste"
 * button next to each AC. Best-effort — when no heuristic matches we return
 * `null` and the UI omits the link.
 *
 * Examples that match:
 *  - `cargo test -p mustard-rt --lib hooks::bash_guard`
 *      -> `apps/rt/src/hooks/bash_guard.rs`
 *  - `cargo test -p mustard-core --lib reader`
 *      -> `packages/core/src/reader.rs`
 *  - `pnpm --filter mustard-dashboard test src/lib/quality-link.ts`
 *      -> `src/lib/quality-link.ts`
 *  - `node -e "require('./apps/dashboard/src/foo.ts')"`
 *      -> `apps/dashboard/src/foo.ts`
 *  - `bash -c 'cat apps/rt/src/main.rs'`
 *      -> `apps/rt/src/main.rs`
 *
 * Examples that do NOT match (return null):
 *  - `pnpm --filter mustard-dashboard build`
 *  - `node -e "process.exit(0)"`
 *  - `echo ok`
 */

/** Workspace crate -> directory map. Falls back to apps/<crate> or
 *  packages/<crate> when the crate is unknown. */
const CRATE_DIRS: Record<string, string> = {
  "mustard-rt":        "apps/rt",
  "mustard-cli":       "apps/cli",
  "mustard-dashboard": "apps/dashboard",
  "mustard-core":      "packages/core",
};

const TEST_FILE_EXT_RE = /\.(rs|ts|tsx|js|jsx|mjs|cjs)\b/;

function resolveCrateDir(crate: string): string {
  if (CRATE_DIRS[crate]) return CRATE_DIRS[crate];
  // Fallback: strip a `mustard-` prefix if present, then guess apps/<name>.
  // This won't always be right; the caller treats a non-existent path as a
  // silent failure (the link just won't open).
  const bare = crate.replace(/^mustard-/, "");
  return `apps/${bare}`;
}

/** Extract `<mod>::<sub>` style cargo test paths into a relative file path
 *  under the crate's `src/` directory. */
function cargoModuleToPath(crateDir: string, modPath: string): string {
  const parts = modPath.split("::").filter(Boolean);
  if (parts.length === 0) return `${crateDir}/src/lib.rs`;
  // Prefer the deepest segment as the filename. The fallback is the parent
  // module file (`<crate>/src/<mod>.rs`) which exists in single-file modules.
  if (parts.length === 1) return `${crateDir}/src/${parts[0]}.rs`;
  const file = parts[parts.length - 1];
  const dir = parts.slice(0, -1).join("/");
  return `${crateDir}/src/${dir}/${file}.rs`;
}

/**
 * Try to extract a single repo-relative path that the command appears to
 * exercise. Returns `null` when no heuristic produced a confident match.
 */
export function extractTestLink(command: string | null): string | null {
  if (!command) return null;
  const cmd = command.trim();
  if (cmd.length === 0) return null;

  // 1. cargo test -p <crate> [--lib|--bin <name>] <mod::sub>
  //    We accept any of: --lib | --bin <x> | --test <x> | (omitted).
  const cargoRe =
    /cargo\s+test\s+(?:--\s+)?(?:[^\s]+\s+)*?-p\s+([\w-]+)(?:\s+(?:--lib|--bin\s+\S+|--test\s+\S+))?\s+([\w:]+)/;
  const cargoMatch = cmd.match(cargoRe);
  if (cargoMatch) {
    const [, crate, modPath] = cargoMatch;
    return cargoModuleToPath(resolveCrateDir(crate), modPath);
  }

  // 2. pnpm --filter <pkg> [test|exec|run] <file-with-ext>
  const pnpmRe =
    /pnpm\s+(?:--filter\s+\S+\s+)?(?:test|exec|run)?\s*(\S+\.(?:rs|ts|tsx|js|jsx|mjs|cjs))/;
  const pnpmMatch = cmd.match(pnpmRe);
  if (pnpmMatch) {
    return pnpmMatch[1];
  }

  // 3. `node -e "...require('./apps/x')..."` or `node -e "...'packages/y'..."`
  //    Extract anything inside quotes that looks like a repo path with ext.
  const quotedPathRe =
    /['"`]\.{0,2}\/?((?:apps|packages|src)\/[\w./@-]+\.(?:rs|ts|tsx|js|jsx|mjs|cjs))['"`]/;
  const quotedMatch = cmd.match(quotedPathRe);
  if (quotedMatch) {
    return quotedMatch[1];
  }

  // 4. Last resort: a standalone path token with a known source extension.
  //    First match wins. Skips paths that look like URLs.
  const standaloneRe =
    /(?:^|\s|=|["'`(])((?:\.{0,2}\/)?(?:[\w@-]+\/)+[\w@.-]+\.(?:rs|ts|tsx|js|jsx|mjs|cjs))(?=$|\s|["'`):;,])/;
  const standaloneMatch = cmd.match(standaloneRe);
  if (standaloneMatch) {
    const p = standaloneMatch[1].replace(/^\.\//, "");
    if (!/^https?:/.test(p) && TEST_FILE_EXT_RE.test(p)) return p;
  }

  return null;
}
