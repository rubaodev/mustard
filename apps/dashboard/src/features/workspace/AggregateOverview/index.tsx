import type { Project } from "@/api/discovery";
import { useStore } from "@/lib/store";
import { SectionHeader, EmptyState } from "@/components/page";
import { Separator } from "@/components/ui/separator";
import { useT } from "@/lib/i18n";
import { SpecStatusCards } from "@/features/workspace/SpecStatusCards";
import { SpecAlertsBand } from "@/features/workspace/SpecAlertsBand";
import { ProjectInfoCard } from "@/features/workspace/ProjectInfoCard";
import { GitInfoCard } from "@/features/workspace/GitInfoCard";
import { WorkspaceFilesRanking } from "@/features/workspace/WorkspaceFilesRanking";

/**
 * Visão Geral — redesign (spec `redesenho-rota-visao-geral-dashboard`). Two
 * purpose-built sections replace the old ROI scoreboard / aggregated
 * Consumption & Savings block / loose KPIs / activity timeline (consumption
 * detail still lives on the Economia page):
 *
 *   - Specs   — stage cards (Planejando/Executando/Finalizadas) + an Alerts
 *               band (Suspeitas, Specs paradas), each deep-linking to `/specs`.
 *   - Projetos — project identity (monorepo, languages, stacks), local git
 *               state, and the reused most-touched-files ranking.
 *
 * The cards are single-repo. The overview targets the selected workspace; when
 * none is selected (portfolio entry), it falls back to the first discovered
 * project so the page is never blank with data on disk.
 */
export function AggregateOverview({ projects }: { projects: Project[] }) {
  const t = useT();
  const activeWorkspaceId = useStore((s) => s.activeWorkspaceId);

  const activeProject =
    projects.find((p) => p.id === activeWorkspaceId) ?? projects[0] ?? null;
  const repoPath = activeProject?.path ?? null;

  if (!repoPath) {
    return (
      <EmptyState
        title={t("overview.empty.title", "Nenhum projeto selecionado")}
        description={t(
          "overview.empty.description",
          "Escolha um workspace na sidebar para ver suas specs e a identidade do projeto.",
        )}
      />
    );
  }

  return (
    <div className="flex flex-col gap-6">
      {/* ── Specs ──────────────────────────────────────────────────────── */}
      <section className="flex flex-col gap-3">
        <SpecStatusCards repoPath={repoPath} />
        <SpecAlertsBand repoPath={repoPath} />
      </section>

      <Separator />

      {/* ── Projetos ───────────────────────────────────────────────────── */}
      <section className="flex flex-col gap-3">
        <SectionHeader title={t("overview.projects.title", "Projetos")} />
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-3">
          <ProjectInfoCard repoPath={repoPath} />
          <GitInfoCard repoPath={repoPath} />
        </div>
        <WorkspaceFilesRanking repoPath={repoPath} />
      </section>
    </div>
  );
}
