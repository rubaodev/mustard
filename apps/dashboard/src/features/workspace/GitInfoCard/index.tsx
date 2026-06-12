import {
  GitBranch,
  GitCommitHorizontal,
  FileDiff,
  FileQuestion,
  Check,
  type LucideIcon,
} from "lucide-react";
import { DataCard, SectionHeader, StatPill, EmptyState } from "@/components/page";
import { useGitInfo } from "@/hooks/useGitInfo";
import type { GitInfo } from "@/lib/dashboard";
import { relativeTime } from "@/lib/time";
import { cn } from "@/lib/utils";
import { useT } from "@/lib/i18n";

interface GitInfoCardProps {
  repoPath: string;
}

/** How many recent commits the history shows at most (backend may cap lower). */
const MAX_COMMITS = 10;
/** How many branch chips render before the rest collapse into a "+N" pill. */
const MAX_BRANCH_CHIPS = 12;

/** Semantic tones for git facets — color carries meaning: branch=violet/accent,
 *  ahead=success, behind=warning, staged=success, unstaged=warning,
 *  untracked/commit=muted. Mapped onto design-system intent variables. */
type GitTone = "branch" | "success" | "warning" | "muted";

const TONE: Record<GitTone, { box: string; text: string }> = {
  branch: { box: "bg-[--accent]/15", text: "text-[--accent]" },
  success: { box: "bg-[--intent-success]/10", text: "text-[--intent-success]" },
  warning: { box: "bg-[--intent-warning]/10", text: "text-[--intent-warning]" },
  muted: { box: "bg-muted/40", text: "text-muted-foreground" },
};

/** Tonal icon container — `h-7 w-7 rounded-md`, the shared overview-card icon
 *  treatment (subtle intent fill + matching foreground). */
function TonalIcon({ icon: Icon, tone }: { icon: LucideIcon; tone: GitTone }) {
  const c = TONE[tone];
  return (
    <span
      aria-hidden
      className={cn(
        "inline-flex h-7 w-7 shrink-0 items-center justify-center rounded-md",
        c.box,
      )}
    >
      <Icon className={cn("h-3.5 w-3.5", c.text)} />
    </span>
  );
}

/** Strip the `.git` suffix and protocol/host noise from a remote URL so the
 *  card shows a compact `owner/repo` style identifier when possible. */
function shortRemote(url: string): string {
  if (!url) return "";
  const s = url.replace(/\.git$/, "");
  // git@host:owner/repo  →  owner/repo
  const scp = s.match(/^[^@]+@[^:]+:(.+)$/);
  if (scp) return scp[1];
  // https://host/owner/repo  →  owner/repo
  const http = s.match(/^https?:\/\/[^/]+\/(.+)$/);
  if (http) return http[1];
  return s;
}

/** One pending-status row (staged / unstaged / untracked) with a tonal icon. */
function PendingItem({
  icon,
  tone,
  count,
  label,
}: {
  icon: LucideIcon;
  tone: GitTone;
  count: number;
  label: string;
}) {
  const active = count > 0;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 text-[12px]",
        active ? "text-foreground/80" : "text-muted-foreground/60",
      )}
      title={label}
    >
      <TonalIcon icon={icon} tone={active ? tone : "muted"} />
      <span className="font-mono tabular-nums">{count}</span>
      <span>{label}</span>
    </span>
  );
}

function GitBody({ data }: { data: GitInfo }) {
  const t = useT();
  const pending = data.pending;
  const cleanTree =
    pending.staged === 0 && pending.unstaged === 0 && pending.untracked === 0;
  const commits = data.recent_commits.slice(0, MAX_COMMITS);
  const branches = data.branches;
  const extraBranches = Math.max(0, branches.length - MAX_BRANCH_CHIPS);

  return (
    <div className="mt-3 flex flex-col gap-4">
      {/* Branch + ahead/behind */}
      <div className="flex flex-wrap items-center gap-2">
        {data.branch ? (
          <span className="inline-flex items-center gap-2 text-[13px] text-foreground/90">
            <TonalIcon icon={GitBranch} tone="branch" />
            <span className="font-mono font-medium">{data.branch}</span>
          </span>
        ) : (
          <span className="inline-flex items-center gap-2 text-[13px] text-muted-foreground/70">
            <TonalIcon icon={GitBranch} tone="muted" />
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

      {/* Pending — working-tree state */}
      <div className="flex flex-col gap-1.5">
        <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
          {t("overview.git.pending", "Pendências")}
        </span>
        {cleanTree ? (
          <span className="inline-flex items-center gap-2 text-[12.5px] text-[--intent-success]">
            <TonalIcon icon={Check} tone="success" />
            {t("overview.git.clean", "working tree limpo")}
          </span>
        ) : (
          <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
            <PendingItem
              icon={Check}
              tone="success"
              count={pending.staged}
              label={t("overview.git.staged", "staged")}
            />
            <PendingItem
              icon={FileDiff}
              tone="warning"
              count={pending.unstaged}
              label={t("overview.git.unstaged", "modificados")}
            />
            <PendingItem
              icon={FileQuestion}
              tone="muted"
              count={pending.untracked}
              label={t("overview.git.untracked", "não rastreados")}
            />
          </div>
        )}
      </div>

      {/* Branches */}
      {branches.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
            {t("overview.git.branches", "Branches")}
          </span>
          <div className="flex flex-wrap gap-1.5">
            {branches.slice(0, MAX_BRANCH_CHIPS).map((b) => {
              const current = b === data.branch;
              return (
                <span
                  key={b}
                  className={cn(
                    "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 font-mono text-[11px]",
                    current
                      ? "border-[--accent]/60 bg-[--accent]/15 text-[--accent]"
                      : "border-border bg-card text-muted-foreground",
                  )}
                >
                  {current && <GitBranch className="h-3 w-3" aria-hidden />}
                  {b}
                </span>
              );
            })}
            {extraBranches > 0 && (
              <span className="inline-flex items-center rounded-full border border-border bg-card px-2 py-0.5 font-mono text-[11px] text-muted-foreground/70">
                +{extraBranches}
              </span>
            )}
          </div>
        </div>
      )}

      {/* History */}
      {commits.length > 0 && (
        <div className="flex flex-col gap-1.5">
          <span className="text-[11px] uppercase tracking-wider text-muted-foreground">
            {t("overview.git.history", "Histórico")}
          </span>
          <ul className="flex flex-col gap-2">
            {commits.map((c) => (
              <li key={c.hash} className="flex items-start gap-2 text-[12.5px]">
                <GitCommitHorizontal
                  className="mt-0.5 h-3.5 w-3.5 shrink-0 text-muted-foreground"
                  aria-hidden
                />
                <div className="flex min-w-0 flex-col gap-0.5">
                  <span className="truncate text-foreground/85" title={c.subject}>
                    <span className="font-mono text-[--accent]">{c.hash}</span>{" "}
                    {c.subject}
                  </span>
                  <span className="text-[11px] text-muted-foreground/70">
                    {c.author}
                    {c.date ? ` · ${relativeTime(c.date)}` : ""}
                  </span>
                </div>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

/**
 * Local git state card for the workspace overview — a compact git client:
 * current branch + ahead/behind vs upstream, working-tree pending counts
 * (staged / unstaged / untracked, or "working tree limpo"), the local branch
 * list (current marked) and the recent-commit history. Backed by `useGitInfo`
 * (local `git`, no network / `gh`). Fail-open — a non-repo path resolves to
 * `is_repo: false`, rendering an empty state instead of an error.
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

  return (
    <DataCard padded>
      <SectionHeader
        title={t("overview.git.title", "Git")}
        right={
          remote ? (
            <span
              className="font-mono text-[11px] text-muted-foreground truncate max-w-[220px]"
              title={data.remote_url}
            >
              {remote}
            </span>
          ) : (
            <span className="text-[11px] text-muted-foreground/70">
              {t("overview.git.noRemote", "sem remote")}
            </span>
          )
        }
      />
      <GitBody data={data} />
    </DataCard>
  );
}
