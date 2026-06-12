import { useMemo } from "react";
import {
  Boxes,
  Package,
  type LucideIcon,
  FileCode,
  Hexagon,
} from "lucide-react";
import { DataCard, SectionHeader, StatPill, EmptyState } from "@/components/page";
import { useProjectOverview } from "@/hooks/useProjectOverview";
import type { ProjectUnitSummary } from "@/lib/dashboard";
import { cn } from "@/lib/utils";
import { useT } from "@/lib/i18n";

interface ProjectInfoCardProps {
  repoPath: string;
}

/** Semantic intent token a language kind maps onto. Color carries meaning —
 *  each ecosystem keeps a stable hue so the eye groups projects by language. */
type LangIntent = "rust" | "node" | "go" | "python" | "java" | "accent";

interface LangFacet {
  label: string;
  icon: LucideIcon;
  intent: LangIntent;
}

/**
 * Map a project `kind` (the model's only per-unit language signal — `cargo`,
 * `npm`, `go`, …) onto a friendly language label + icon + semantic color so the
 * card answers "which language?" at a glance. Unknown kinds fall back to the
 * raw kind (capitalised) with the neutral accent hue rather than inventing a
 * label or a new palette.
 */
const KIND_FACETS: Record<string, LangFacet> = {
  cargo: { label: "Rust", icon: FileCode, intent: "rust" },
  npm: { label: "Node/TS", icon: Hexagon, intent: "node" },
  pnpm: { label: "Node/TS", icon: Hexagon, intent: "node" },
  yarn: { label: "Node/TS", icon: Hexagon, intent: "node" },
  go: { label: "Go", icon: FileCode, intent: "go" },
  pip: { label: "Python", icon: FileCode, intent: "python" },
  poetry: { label: "Python", icon: FileCode, intent: "python" },
  uv: { label: "Python", icon: FileCode, intent: "python" },
  maven: { label: "Java", icon: FileCode, intent: "java" },
  gradle: { label: "Java/Kotlin", icon: FileCode, intent: "java" },
  composer: { label: "PHP", icon: FileCode, intent: "accent" },
  bundler: { label: "Ruby", icon: FileCode, intent: "accent" },
  pub: { label: "Dart/Flutter", icon: FileCode, intent: "accent" },
  dotnet: { label: ".NET", icon: FileCode, intent: "accent" },
  swift: { label: "Swift", icon: FileCode, intent: "accent" },
};

function langFacet(kind: string): LangFacet {
  const known = KIND_FACETS[kind.toLowerCase()];
  if (known) return known;
  return {
    label: kind.charAt(0).toUpperCase() + kind.slice(1),
    icon: Package,
    intent: "accent",
  };
}

/**
 * Tonal classes per language intent. `cargo→Rust` orange, `npm→Node/TS`
 * cyan, `go` cyan-blue, `python` amber, `java` red, default accent. Mapped onto
 * the design-system intent variables (no new palette): orange/amber reuse
 * `--intent-warning`, cyan/blue `--intent-info`, red `--intent-error`, default
 * `--accent`. Each pairs a dessaturated `/10` tonal fill with the matching text
 * color so the icon container reads as a label, not decoration.
 */
const LANG_TONE: Record<LangIntent, { box: string; text: string }> = {
  rust: { box: "bg-[--intent-warning]/10", text: "text-[--intent-warning]" },
  node: { box: "bg-[--intent-info]/10", text: "text-[--intent-info]" },
  go: { box: "bg-[--intent-info]/10", text: "text-[--intent-info]" },
  python: { box: "bg-[--intent-warning]/10", text: "text-[--intent-warning]" },
  java: { box: "bg-[--intent-error]/10", text: "text-[--intent-error]" },
  accent: { box: "bg-[--accent]/15", text: "text-foreground/70" },
};

/** Tonal icon container — `h-7 w-7 rounded-md` with a subtle intent fill and
 *  matching foreground, the shared treatment for every overview-card icon. */
function TonalIcon({
  icon: Icon,
  intent,
}: {
  icon: LucideIcon;
  intent: LangIntent;
}) {
  const tone = LANG_TONE[intent];
  return (
    <span
      aria-hidden
      className={cn(
        "inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md",
        tone.box,
      )}
    >
      <Icon className={cn("h-3.5 w-3.5", tone.text)} />
    </span>
  );
}

/** One project row inside the per-project list. */
function UnitRow({ unit }: { unit: ProjectUnitSummary }) {
  const facet = langFacet(unit.language);
  const tone = LANG_TONE[facet.intent];
  return (
    <li className="flex items-start gap-2.5 rounded-md border border-border bg-card/30 px-2.5 py-2">
      <TonalIcon icon={facet.icon} intent={facet.intent} />
      <div className="flex min-w-0 flex-col gap-1">
        <div className="flex items-baseline gap-2">
          <span className="truncate text-[13px] font-medium text-foreground">
            {unit.name}
          </span>
          <span className={cn("shrink-0 text-[11px] font-medium", tone.text)}>
            {facet.label}
          </span>
        </div>
        <span
          className="truncate font-mono text-[11px] text-muted-foreground"
          title={unit.dir}
        >
          {unit.dir || "."}
        </span>
        {unit.frameworks.length > 0 && (
          <div className="mt-0.5 flex flex-wrap gap-1">
            {unit.frameworks.slice(0, 8).map((fw) => (
              <StatPill key={fw} value={fw} />
            ))}
          </div>
        )}
      </div>
    </li>
  );
}

/**
 * Project identity card for the workspace overview. Header: monorepo flag +
 * project count. Top: an aggregate detected-stacks summary. Body: one row per
 * project from `units` — name, directory (mono/muted), language (kind→label +
 * colored icon) and that project's frameworks as badges. The list scrolls past
 * a threshold so a large monorepo never overflows the page. Empty-state
 * tolerant — an unscanned workspace resolves to an empty overview (the Tauri
 * command is fail-open).
 */
export function ProjectInfoCard({ repoPath }: ProjectInfoCardProps) {
  const t = useT();
  const { data } = useProjectOverview(repoPath);

  const units = useMemo<ProjectUnitSummary[]>(() => data?.units ?? [], [data?.units]);
  const hasData = !!data && data.project_count > 0;

  return (
    <DataCard padded>
      <SectionHeader
        title={t("overview.project.title", "Projeto")}
        right={
          hasData ? (
            <span className="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground">
              <Boxes className="h-3.5 w-3.5 text-[--accent]" aria-hidden />
              {data.is_monorepo
                ? t("overview.project.monorepo", "monorepo")
                : t("overview.project.single", "projeto único")}
              {" · "}
              <span className="tabular-nums">{data.project_count}</span>
            </span>
          ) : undefined
        }
      />

      {!hasData ? (
        <EmptyState
          className="mt-3"
          title={t("overview.project.empty.title", "Sem modelo do projeto")}
          description={t(
            "overview.project.empty.description",
            "Rode /mustard:scan para minerar linguagens e stacks deste workspace.",
          )}
        />
      ) : (
        <div className="mt-3 flex flex-col gap-3">
          {data.detected_stacks.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
                {t("overview.project.stacks", "Stacks")}
              </span>
              <ul className="flex flex-wrap gap-x-3 gap-y-0.5">
                {data.detected_stacks.map((stack) => (
                  <li
                    key={stack.name}
                    className="flex items-center gap-1.5 text-[12.5px] text-foreground/80"
                  >
                    <Package
                      className="h-3.5 w-3.5 shrink-0 text-[--accent]"
                      aria-hidden
                    />
                    <span className="font-mono">{stack.name}</span>
                    <span className="text-[11px] text-muted-foreground tabular-nums">
                      {Math.round(stack.confidence * 100)}%
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {units.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
                {t("overview.project.units", "Projetos")}
              </span>
              <ul className="flex max-h-[320px] flex-col gap-1.5 overflow-y-auto pr-1">
                {units.map((unit) => (
                  <UnitRow key={`${unit.dir}:${unit.name}`} unit={unit} />
                ))}
              </ul>
            </div>
          )}
        </div>
      )}
    </DataCard>
  );
}
