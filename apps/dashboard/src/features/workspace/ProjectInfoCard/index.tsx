import { useMemo } from "react";
import { Boxes, Package } from "lucide-react";
import { DataCard, SectionHeader, StatPill, EmptyState } from "@/components/page";
import { useProjectOverview } from "@/hooks/useProjectOverview";
import { useT } from "@/lib/i18n";

interface ProjectInfoCardProps {
  repoPath: string;
}

/**
 * Map a project `kind` (the model's only per-unit language signal — `cargo`,
 * `npm`, `go`, …) onto a friendly language label so the card answers "which
 * language?". Unknown kinds fall back to the raw kind (capitalised) rather than
 * inventing a label.
 */
const KIND_LABELS: Record<string, string> = {
  cargo: "Rust",
  npm: "Node/TS",
  pnpm: "Node/TS",
  yarn: "Node/TS",
  go: "Go",
  pip: "Python",
  poetry: "Python",
  uv: "Python",
  maven: "Java",
  gradle: "Java/Kotlin",
  composer: "PHP",
  bundler: "Ruby",
  pub: "Dart/Flutter",
  dotnet: ".NET",
  swift: "Swift",
};

function kindLabel(kind: string): string {
  const known = KIND_LABELS[kind.toLowerCase()];
  if (known) return known;
  return kind.charAt(0).toUpperCase() + kind.slice(1);
}

/**
 * Project identity card for the workspace overview: monorepo flag, project
 * count and the languages (kind→label) / detected stacks mined from the grain
 * model. Empty-state tolerant — an unscanned workspace resolves to an empty
 * overview (the Tauri command is fail-open).
 */
export function ProjectInfoCard({ repoPath }: ProjectInfoCardProps) {
  const t = useT();
  const { data } = useProjectOverview(repoPath);

  const languages = useMemo(
    () => (data?.languages ?? []).map(kindLabel),
    [data?.languages],
  );

  const hasData = !!data && data.project_count > 0;

  return (
    <DataCard padded>
      <SectionHeader
        title={t("overview.project.title", "Projeto")}
        right={
          hasData ? (
            <span className="inline-flex items-center gap-1.5 text-[11px] text-muted-foreground">
              <Boxes className="h-3.5 w-3.5" aria-hidden />
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
          {languages.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
                {t("overview.project.languages", "Linguagens")}
              </span>
              <div className="flex flex-wrap gap-1.5">
                {languages.map((lang) => (
                  <StatPill key={lang} value={lang} intent="info" />
                ))}
              </div>
            </div>
          )}

          {data.detected_stacks.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
                {t("overview.project.stacks", "Stacks")}
              </span>
              <ul className="flex flex-col gap-0.5">
                {data.detected_stacks.map((stack) => (
                  <li
                    key={stack.name}
                    className="flex items-center gap-2 text-[12.5px] text-foreground/80"
                  >
                    <Package className="h-3.5 w-3.5 shrink-0 text-muted-foreground" aria-hidden />
                    <span className="font-mono">{stack.name}</span>
                    <span className="ml-auto text-[11px] text-muted-foreground tabular-nums">
                      {Math.round(stack.confidence * 100)}%
                    </span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {data.frameworks.length > 0 && (
            <div className="flex flex-col gap-1.5">
              <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
                {t("overview.project.frameworks", "Frameworks")}
              </span>
              <div className="flex flex-wrap gap-1.5">
                {data.frameworks.slice(0, 12).map((fw) => (
                  <StatPill key={fw} value={fw} />
                ))}
              </div>
            </div>
          )}
        </div>
      )}
    </DataCard>
  );
}
