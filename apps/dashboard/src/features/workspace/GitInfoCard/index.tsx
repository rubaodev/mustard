import { GitBranch, GitCommitHorizontal } from "lucide-react";
import { DataCard, SectionHeader, StatPill, EmptyState } from "@/components/page";
import { useGitInfo } from "@/hooks/useGitInfo";
import { relativeTime } from "@/lib/time";
import { useT } from "@/lib/i18n";

interface GitInfoCardProps {
  repoPath: string;
}

/** Strip the `.git` suffix and protocol/host noise from a remote URL so the
 *  card shows a compact `owner/repo` style identifier when possible. */
function shortRemote(url: string): string {
  if (!url) return "";
  let s = url.replace(/\.git$/, "");
  // git@host:owner/repo  →  owner/repo
  const scp = s.match(/^[^@]+@[^:]+:(.+)$/);
  if (scp) return scp[1];
  // https://host/owner/repo  →  owner/repo
  const http = s.match(/^https?:\/\/[^/]+\/(.+)$/);
  if (http) return http[1];
  return s;
}

/**
 * Local git state card for the workspace overview: remote, branch, ahead/behind
 * vs upstream and the last commit. Backed by `useGitInfo` (local `git`, no
 * network / `gh`). Fail-open — a non-repo path resolves to `is_repo: false`,
 * rendering an empty state instead of an error.
 */
export function GitInfoCard({ repoPath }: GitInfoCardProps) {
  const t = useT();
  const { data } = useGitInfo(repoPath);

  if (!data || !data.is_repo) {
    return (
      <DataCard padded>
        <SectionHeader title={t("overview.git.title", "Git")} />
        <EmptyState
          className="mt-3"
          title={t("overview.git.empty.title", "Sem repositório git")}
          description={t(
            "overview.git.empty.description",
            "Este workspace não está dentro de um repositório git.",
          )}
        />
      </DataCard>
    );
  }

  const remote = shortRemote(data.remote_url);
  const commitDate = data.last_commit_date ? relativeTime(data.last_commit_date) : null;

  return (
    <DataCard padded>
      <SectionHeader
        title={t("overview.git.title", "Git")}
        right={
          remote ? (
            <span className="font-mono text-[11px] text-muted-foreground truncate max-w-[220px]" title={data.remote_url}>
              {remote}
            </span>
          ) : (
            <span className="text-[11px] text-muted-foreground/70">
              {t("overview.git.noRemote", "sem remote")}
            </span>
          )
        }
      />

      <div className="mt-3 flex flex-col gap-3">
        <div className="flex flex-wrap items-center gap-2">
          {data.branch ? (
            <span className="inline-flex items-center gap-1.5 text-[12.5px] text-foreground/80">
              <GitBranch className="h-3.5 w-3.5 text-muted-foreground" aria-hidden />
              <span className="font-mono">{data.branch}</span>
            </span>
          ) : (
            <span className="inline-flex items-center gap-1.5 text-[12.5px] text-muted-foreground/70">
              <GitBranch className="h-3.5 w-3.5" aria-hidden />
              {t("overview.git.detached", "HEAD destacado")}
            </span>
          )}
          {data.ahead > 0 && (
            <StatPill
              value={data.ahead}
              unit="↑"
              intent="success"
              tooltip={t("overview.git.aheadTooltip", "commits à frente do upstream")}
            />
          )}
          {data.behind > 0 && (
            <StatPill
              value={data.behind}
              unit="↓"
              intent="warning"
              tooltip={t("overview.git.behindTooltip", "commits atrás do upstream")}
            />
          )}
          {data.ahead === 0 && data.behind === 0 && data.branch && (
            <span className="text-[11px] text-muted-foreground/70">
              {t("overview.git.upToDate", "em dia")}
            </span>
          )}
        </div>

        {data.last_commit_hash && (
          <div className="flex items-start gap-2 text-[12.5px]">
            <GitCommitHorizontal className="h-3.5 w-3.5 shrink-0 mt-0.5 text-muted-foreground" aria-hidden />
            <div className="flex flex-col gap-0.5 min-w-0">
              <span className="text-foreground/80 truncate" title={data.last_commit_message}>
                <span className="font-mono text-muted-foreground">{data.last_commit_hash}</span>{" "}
                {data.last_commit_message}
              </span>
              <span className="text-[11px] text-muted-foreground/70">
                {data.last_commit_author}
                {commitDate ? ` · ${commitDate}` : ""}
              </span>
            </div>
          </div>
        )}
      </div>
    </DataCard>
  );
}
