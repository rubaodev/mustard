import { useEffect, useMemo, useState } from "react";
import { useQuery, useQueries, useQueryClient } from "@tanstack/react-query";
import {
  Search,
  PlayCircle,
  Eye,
  AlertOctagon,
  CheckCircle2,
  CircleDashed,
  Clock,
  Ban,
  Trash2,
} from "lucide-react";
import { useStore } from "@/lib/store";
import {
  useProjects,
  fetchSpecs,
  dashboardSpecCard,
  type SpecCard,
} from "@/lib/dashboard";
import {
  SectionHeader,
  EmptyState,
} from "@/components/page";
import { SpecCard as SpecCardComponent } from "@/components/specs/SpecCard";
import { SpecTabBar, type SpecTab } from "@/components/specs/SpecTabBar";
import { SpecDetailDashboard } from "@/components/specs/SpecDetailDashboard";
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";

// ── Phase ordering for active specs ──────────────────────────────────────────
const PHASE_ORDER = ["analyze", "plan", "execute", "qa", "close"];
function phaseRank(phase: string): number {
  const i = PHASE_ORDER.indexOf(phase.toLowerCase());
  return i === -1 ? PHASE_ORDER.length : i;
}

type StatusFilter =
  | "ativas"
  | "followup"
  | "encerradas"
  | "cancelado"
  | "abandonado"
  | "todas";
type DateFilter = "today" | "7d" | "30d" | "all";

const STATUS_LABEL: Record<StatusFilter, string> = {
  ativas: "Ativas",
  followup: "Follow-up",
  encerradas: "Encerradas",
  cancelado: "Cancelado",
  abandonado: "Abandonado",
  todas: "Todas",
};

// ── Inline SpecsTopBar ────────────────────────────────────────────────────────
interface SpecsTopBarProps {
  status: StatusFilter;
  onStatus: (v: StatusFilter) => void;
  date: DateFilter;
  onDate: (v: DateFilter) => void;
  search: string;
  onSearch: (v: string) => void;
}

function SpecsTopBar({
  status,
  onStatus,
  date,
  onDate,
  search,
  onSearch,
}: SpecsTopBarProps) {
  const btnBase =
    "px-2.5 py-1 rounded text-[12px] transition-colors duration-100";
  const active = "bg-primary/10 text-primary font-medium";
  const inactive = "text-muted-foreground hover:bg-muted/40 hover:text-foreground";

  return (
    <div className="flex items-center gap-3 flex-wrap">
      {/* Status filters */}
      <div className="flex items-center gap-1">
        {(
          [
            "ativas",
            "followup",
            "encerradas",
            "cancelado",
            "abandonado",
            "todas",
          ] as StatusFilter[]
        ).map((v) => (
          <button
            key={v}
            type="button"
            onClick={() => onStatus(v)}
            aria-pressed={status === v}
            className={`${btnBase} ${status === v ? active : inactive}`}
          >
            {STATUS_LABEL[v]}
          </button>
        ))}
      </div>

      {/* Date filters */}
      <div className="flex items-center gap-1">
        {(["today", "7d", "30d", "all"] as DateFilter[]).map((v) => {
          const label = v === "today" ? "Hoje" : v === "all" ? "Todas" : v;
          return (
            <button
              key={v}
              type="button"
              onClick={() => onDate(v)}
              aria-pressed={date === v}
              className={`${btnBase} ${date === v ? active : inactive}`}
            >
              {label}
            </button>
          );
        })}
      </div>

      {/* Search */}
      <div className="relative flex-1 min-w-[160px]">
        <Search
          className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground"
          aria-hidden
        />
        <input
          value={search}
          onChange={(e) => onSearch(e.target.value)}
          placeholder="Buscar por nome…"
          aria-label="Buscar specs por nome"
          className="w-full pl-7 pr-3 py-1 bg-card border border-border rounded-md text-[12px] outline-none placeholder:text-muted-foreground focus:border-primary focus-visible:ring-2 focus-visible:ring-[--color-accent-mustard] transition-colors"
        />
      </div>
    </div>
  );
}

// ── Quick-open dialog ────────────────────────────────────────────────────────
interface SpecQuickOpenDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  cards: SpecCard[];
  onPick: (slug: string) => void;
}

function SpecQuickOpenDialog({
  open,
  onOpenChange,
  cards,
  onPick,
}: SpecQuickOpenDialogProps) {
  const [query, setQuery] = useState("");

  // Reset query each time the dialog opens so the previous search does not
  // leak across opens.
  useEffect(() => {
    if (open) setQuery("");
  }, [open]);

  const q = query.trim().toLowerCase();
  const filtered = q
    ? cards.filter((c) => c.spec.toLowerCase().includes(q))
    : cards;

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-lg">
        <DialogHeader>
          <DialogTitle>Abrir spec em nova aba</DialogTitle>
        </DialogHeader>
        <div className="flex flex-col gap-2">
          <div className="relative">
            <Search
              className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3 w-3 text-muted-foreground"
              aria-hidden
            />
            <input
              autoFocus
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder="Buscar por nome…"
              aria-label="Buscar specs"
              className="w-full pl-7 pr-3 py-1.5 bg-card border border-border rounded-md text-[12px] outline-none placeholder:text-muted-foreground focus:border-primary focus-visible:ring-2 focus-visible:ring-[--color-accent-mustard] transition-colors"
            />
          </div>
          <div className="max-h-[360px] overflow-y-auto flex flex-col gap-0.5">
            {filtered.length === 0 ? (
              <p className="px-2 py-4 text-center text-[12px] text-muted-foreground">
                Nenhuma spec encontrada.
              </p>
            ) : (
              filtered.map((c) => (
                <button
                  key={c.spec}
                  type="button"
                  onClick={() => {
                    onPick(c.spec);
                    onOpenChange(false);
                  }}
                  className="flex items-center justify-between gap-2 px-2 py-1.5 rounded text-left text-[12px] hover:bg-muted/60 focus-visible:bg-muted/60 outline-none transition-colors"
                >
                  <span className="font-mono truncate flex-1 min-w-0" title={c.spec}>
                    {c.spec}
                  </span>
                  <span
                    className="text-[10px] uppercase tracking-wide text-muted-foreground shrink-0"
                    title={c.status}
                  >
                    {c.status}
                  </span>
                </button>
              ))
            )}
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}

// ── Main page ─────────────────────────────────────────────────────────────────
export function Specs() {
  const projectsRoot = useStore((s) => s.projectsRoot);
  const activeWorkspaceId = useStore((s) => s.activeWorkspaceId);
  const projects = useProjects();
  const activeProject = projects.find((p) => p.id === activeWorkspaceId) ?? null;
  const queryClient = useQueryClient();

  // Default to "ativas" — the primary use-case is "what's running now",
  // not "everything ever". The legacy default of "todas" buried current
  // work under closed history (spec 2026-05-20-dashboard-ux-honest).
  const [statusFilter, setStatusFilter] = useState<StatusFilter>("ativas");
  const [dateFilter, setDateFilter] = useState<DateFilter>("all");
  const [search, setSearch] = useState("");

  // Wave-1 (spec `2026-05-21-dashboard-spec-tabs`): route-local tab state.
  // Sair da rota = unmount = state limpo. No persistence in zustand.
  const [tabs, setTabs] = useState<SpecTab[]>([{ id: "list", kind: "list" }]);
  const [activeTabId, setActiveTabId] = useState<string>("list");
  const [quickOpenOpen, setQuickOpenOpen] = useState(false);

  function openSpec(slug: string) {
    setTabs((prev) => {
      const exists = prev.some((t) => t.kind === "spec" && t.specName === slug);
      if (exists) return prev;
      return [...prev, { id: slug, kind: "spec", specName: slug }];
    });
    setActiveTabId(slug);
  }

  function closeSpec(id: string) {
    if (id === "list") return; // never closable
    setTabs((prev) => {
      const idx = prev.findIndex((t) => t.id === id);
      if (idx === -1) return prev;
      const next = prev.filter((t) => t.id !== id);
      // If the closed tab was active, focus the tab immediately to the left
      // (falls back to "list" when no spec tabs remain to the left).
      if (activeTabId === id) {
        const leftIdx = Math.max(0, idx - 1);
        const leftTab = next[leftIdx] ?? next[0] ?? { id: "list" };
        setActiveTabId(leftTab.id);
      }
      return next;
    });
  }

  function onRefresh() {
    const active = tabs.find((t) => t.id === activeTabId);
    if (!active || active.kind === "list") {
      queryClient.invalidateQueries({ queryKey: ["specs"] });
      queryClient.invalidateQueries({ queryKey: ["spec-card"] });
      return;
    }
    const slug = active.specName;
    queryClient.invalidateQueries({ queryKey: ["spec-card", undefined, slug] });
    queryClient.invalidateQueries({ queryKey: ["spec-card"] });
    queryClient.invalidateQueries({ queryKey: ["spec-waves", slug] });
    queryClient.invalidateQueries({ queryKey: ["spec-quality", slug] });
    queryClient.invalidateQueries({ queryKey: ["spec-children", slug] });
    queryClient.invalidateQueries({ queryKey: ["spec-events", slug] });
  }

  // Hash deep-link: auto-open spec on mount only when the hash looks like a
  // spec slug (date-prefixed). HashRouter paths like `#/specs` would otherwise
  // be treated as a slug and open a phantom tab.
  useEffect(() => {
    const hash = window.location.hash.replace(/^#/, "");
    if (hash && /^\d{4}-\d{2}-\d{2}-/.test(hash)) openSpec(hash);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Fetch spec list (SpecRow[])
  const { data: specRows, isLoading: listLoading } = useQuery({
    queryKey: ["specs", activeProject?.path],
    queryFn: () => fetchSpecs(activeProject!.path),
    enabled: !!activeProject,
    staleTime: 10_000,
    refetchInterval: 15_000,
  });

  // Fan-out: fetch SpecCard for each spec. Wave 5 fix (2026-05-20): every
  // card polls on a 5-second cadence so an active pipeline animates without
  // the user having to refocus the window.
  const cardQueries = useQueries({
    queries: (specRows ?? []).map((row) => ({
      queryKey: ["spec-card", activeProject?.path, row.name] as const,
      queryFn: (): Promise<SpecCard> =>
        dashboardSpecCard(activeProject!.path, row.name),
      enabled: !!activeProject,
      staleTime: 5_000,
      refetchInterval: 5_000,
      refetchIntervalInBackground: false,
    })),
  });

  const cards = useMemo<SpecCard[]>(() => {
    return cardQueries
      .map((q) => q.data)
      .filter((d): d is SpecCard => d != null);
  }, [cardQueries]);

  // Bug 1 fix (spec `2026-05-21-dashboard-spec-tabs-polish` W1): the `cards`
  // fan-out is governed by `useQueries`, which takes one tick to populate
  // even when the list query is `success`. Treat the page as still loading
  // while ANY card query is mid-flight so we never flash the "Nenhuma spec
  // encontrada" empty state on first mount. Two-stage cascade: list loading
  // OR per-card fan-out loading → render the skeleton row block.
  const cardsLoading = cardQueries.some((q) => q.isLoading);
  const specsLoading = listLoading || cardsLoading;

  // Date cutoff
  const dateCutoff = useMemo<number>(() => {
    const now = Date.now();
    if (dateFilter === "today") return now - 24 * 60 * 60 * 1000;
    if (dateFilter === "7d") return now - 7 * 24 * 60 * 60 * 1000;
    if (dateFilter === "30d") return now - 30 * 24 * 60 * 60 * 1000;
    return 0;
  }, [dateFilter]);

  const FOLLOWUP_STATUS = "closed-followup";
  const COMPLETED_STATUSES = new Set(["completed", "closed", "no-events"]);
  const CANCELLED_STATUSES = new Set(["cancelled"]);
  const ABANDONED_STATUSES = new Set(["abandoned"]);
  const TERMINAL_STATUSES = new Set([
    ...COMPLETED_STATUSES,
    ...CANCELLED_STATUSES,
    ...ABANDONED_STATUSES,
  ]);
  const isFollowup = (c: SpecCard) => c.status === FOLLOWUP_STATUS;
  const isCancelled = (c: SpecCard) => CANCELLED_STATUSES.has(c.status);
  const isAbandoned = (c: SpecCard) => ABANDONED_STATUSES.has(c.status);
  const isTerminal = (c: SpecCard) => TERMINAL_STATUSES.has(c.status);
  const isActive = (c: SpecCard) => !isTerminal(c) && !isFollowup(c);

  type GroupKey =
    | "ativas"
    | "revisao"
    | "bloqueadas"
    | "followup"
    | "concluidas"
    | "cancelados"
    | "abandonados"
    | "sem-eventos";
  const GROUP_ORDER: GroupKey[] = [
    "ativas",
    "revisao",
    "bloqueadas",
    "followup",
    "concluidas",
    "cancelados",
    "abandonados",
    "sem-eventos",
  ];
  const GROUP_META: Record<
    GroupKey,
    { label: string; Icon: typeof PlayCircle }
  > = {
    ativas: { label: "Ativas", Icon: PlayCircle },
    revisao: { label: "Em revisão", Icon: Eye },
    bloqueadas: { label: "Bloqueadas", Icon: AlertOctagon },
    followup: { label: "Follow-up", Icon: Clock },
    concluidas: { label: "Concluídas", Icon: CheckCircle2 },
    cancelados: { label: "Cancelados", Icon: Ban },
    abandonados: { label: "Abandonados", Icon: Trash2 },
    "sem-eventos": { label: "Sem eventos", Icon: CircleDashed },
  };
  function groupKeyForStatus(status: string): GroupKey {
    if (status === "no-events") return "sem-eventos";
    if (status === "blocked" || status === "wave-failed") return "bloqueadas";
    if (status === "reviewing" || status === "qa") return "revisao";
    if (status === FOLLOWUP_STATUS) return "followup";
    if (ABANDONED_STATUSES.has(status)) return "abandonados";
    if (CANCELLED_STATUSES.has(status)) return "cancelados";
    if (COMPLETED_STATUSES.has(status)) return "concluidas";
    return "ativas";
  }

  const filteredSpecs = useMemo<SpecCard[]>(() => {
    return cards
      .filter((c) => {
        if (statusFilter === "ativas" && !isActive(c)) return false;
        if (statusFilter === "followup" && !isFollowup(c)) return false;
        if (statusFilter === "encerradas" && !isTerminal(c)) return false;
        if (statusFilter === "cancelado" && !isCancelled(c)) return false;
        if (statusFilter === "abandonado" && !isAbandoned(c)) return false;
        return true;
      })
      .filter((c) => {
        if (dateCutoff === 0) return true;
        const ts = c.last_event_at ?? c.started_at;
        if (!ts) return true;
        return new Date(ts).getTime() >= dateCutoff;
      })
      .filter((c) => {
        if (!search.trim()) return true;
        return c.spec.toLowerCase().includes(search.trim().toLowerCase());
      })
      .sort((a, b) => {
        const rank = (c: SpecCard) => (isActive(c) ? 0 : isFollowup(c) ? 1 : 2);
        const ra = rank(a);
        const rb = rank(b);
        if (ra !== rb) return ra - rb;
        if (ra === 0) return phaseRank(a.phase) - phaseRank(b.phase);
        const ta = a.last_event_at ? new Date(a.last_event_at).getTime() : 0;
        const tb = b.last_event_at ? new Date(b.last_event_at).getTime() : 0;
        return tb - ta;
      });
  }, [cards, statusFilter, dateCutoff, search]);

  const groupedByStatus = useMemo<[GroupKey, SpecCard[]][]>(() => {
    if (statusFilter !== "todas") return [];
    const map = new Map<GroupKey, SpecCard[]>();
    for (const key of GROUP_ORDER) map.set(key, []);
    for (const c of filteredSpecs) {
      const key = groupKeyForStatus(c.status);
      map.get(key)!.push(c);
    }
    return GROUP_ORDER.map((k) => [k, map.get(k) ?? []] as [GroupKey, SpecCard[]])
      .filter(([, list]) => list.length > 0);
  }, [statusFilter, filteredSpecs]);

  // ── Gate cascade ─────────────────────────────────────────────────────────
  if (!projectsRoot) {
    return (
      <div className="flex flex-col gap-6 w-full">
        <EmptyState
          title="Diretório de projetos não configurado"
          description="Vá em Configurações e aponte para a pasta onde estão seus repos."
        />
      </div>
    );
  }

  if (!activeWorkspaceId) {
    return (
      <div className="flex flex-col gap-6 w-full">
        <EmptyState
          title="Selecione um workspace"
          description="Use o seletor na sidebar para escolher um projeto."
        />
      </div>
    );
  }

  const activeTab = tabs.find((t) => t.id === activeTabId) ?? tabs[0];
  const repoPath = activeProject?.path ?? null;

  return (
    <div className="flex flex-col gap-4 w-full">
      <SpecTabBar
        tabs={tabs}
        activeId={activeTabId}
        onActivate={setActiveTabId}
        onClose={closeSpec}
        onAddRequest={() => setQuickOpenOpen(true)}
        onRefresh={onRefresh}
      />

      <SpecQuickOpenDialog
        open={quickOpenOpen}
        onOpenChange={setQuickOpenOpen}
        cards={cards}
        onPick={openSpec}
      />

      {activeTab.kind === "list" ? (
        <div className="flex flex-col gap-6">
          <SpecsTopBar
            status={statusFilter}
            onStatus={setStatusFilter}
            date={dateFilter}
            onDate={setDateFilter}
            search={search}
            onSearch={setSearch}
          />

          <section className="flex flex-col gap-3">
            <SectionHeader
              title="Specs"
              right={specsLoading ? undefined : String(filteredSpecs.length)}
            />

            {specsLoading ? (
              <ul className="flex flex-col gap-2">
                {[0, 1, 2].map((i) => (
                  <li
                    key={i}
                    className="h-20 bg-muted/40 rounded-lg animate-pulse"
                  />
                ))}
              </ul>
            ) : filteredSpecs.length === 0 ? (
              <EmptyState
                title="Nenhuma spec encontrada"
                description="Ajuste os filtros ou rode uma pipeline com /mustard:feature."
              />
            ) : statusFilter === "todas" ? (
              <div className="flex flex-col gap-5">
                {groupedByStatus.map(([key, list]) => {
                  const meta = GROUP_META[key];
                  const Icon = meta.Icon;
                  return (
                    <section key={key} className="flex flex-col gap-2">
                      <header className="flex items-center gap-2 text-[11px] uppercase tracking-wide text-muted-foreground">
                        <Icon className="h-3.5 w-3.5" aria-hidden />
                        <span className="font-medium">{meta.label}</span>
                        <span
                          className="tabular-nums text-muted-foreground/60"
                          style={{ fontVariantNumeric: "tabular-nums" }}
                        >
                          {list.length}
                        </span>
                      </header>
                      <div className="flex flex-col gap-2">
                        {list.map((s) => (
                          <SpecCardComponent
                            key={s.spec}
                            data={s}
                            repoPath={repoPath}
                            onOpenSpec={openSpec}
                          />
                        ))}
                      </div>
                    </section>
                  );
                })}
              </div>
            ) : (
              <div className="flex flex-col gap-2">
                {filteredSpecs.map((s) => (
                  <SpecCardComponent
                    key={s.spec}
                    data={s}
                    repoPath={repoPath}
                    onOpenSpec={openSpec}
                  />
                ))}
              </div>
            )}
          </section>
        </div>
      ) : (
        <SpecDetailDashboard repoPath={repoPath} spec={activeTab.specName} />
      )}
    </div>
  );
}
