import { useEffect, useState, useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import { Search, AlertTriangle } from "lucide-react";
import { useStore } from "@/lib/store";
import {
  useProjects,
  fetchKnowledgeBrowse,
  fetchSearchKnowledge,
  fetchFriction,
  type KnowledgeBrowseRow,
  type KnowledgeRow,
  type FrictionEntry,
} from "@/lib/dashboard";
import { Badge } from "@/components/ui/badge";
import { KnowledgeCard } from "@/components/KnowledgeCard";
import {
  PageHeader,
  SectionHeader,
  EmptyState,
  DataCard,
} from "@/components/page";
import { relativeTime } from "@/lib/time";

/**
 * Knowledge type labels. Only `convention` is rendered as "CONVENÇÃO" — and
 * only for rows whose backend type is literally `convention`. Friction signals
 * (hook-retry, heavy pipeline) are NOT knowledge: they come from a separate
 * source (friction.json) and render in their own section below.
 */
const TYPE_LABELS: Record<string, string> = {
  "entity-cluster": "Cluster de entidade",
  "naming-pattern": "Padrão de nomenclatura",
  decision: "Decisão",
  lesson: "Lição",
  recipe: "Receita",
  convention: "Convenção",
  pattern: "Padrão",
};
function labelType(t: string): string {
  return TYPE_LABELS[t] ?? t;
}

/** Sort order so "real knowledge" types lead and noisier ones trail. */
const TYPE_ORDER = [
  "decision",
  "pattern",
  "naming-pattern",
  "entity-cluster",
  "convention",
  "recipe",
  "lesson",
];
function typeRank(t: string): number {
  const i = TYPE_ORDER.indexOf(t);
  return i === -1 ? TYPE_ORDER.length : i;
}

function truncate(s: string, n: number): string {
  return s.length > n ? s.slice(0, n - 1) + "…" : s;
}

export function Knowledge() {
  const projectsRoot = useStore((s) => s.projectsRoot);
  const activeWorkspaceId = useStore((s) => s.activeWorkspaceId);
  const persistedQuery = useStore((s) => s.knowledgeQuery);
  const setKnowledgeQuery = useStore((s) => s.setKnowledgeQuery);

  const projects = useProjects();
  const activeProject = projects.find((p) => p.id === activeWorkspaceId) ?? null;

  const [query, setQuery] = useState(persistedQuery);
  const [debouncedQuery, setDebouncedQuery] = useState(persistedQuery);

  useEffect(() => {
    const t = setTimeout(() => {
      setDebouncedQuery(query);
      setKnowledgeQuery(query);
    }, 300);
    return () => clearTimeout(t);
  }, [query, setKnowledgeQuery]);

  const trimmed = debouncedQuery.trim();
  const hasQuery = trimmed.length >= 2;

  // Browse: all knowledge rows for the active workspace.
  const { data: browseRows, isLoading: browseLoading } = useQuery({
    queryKey: ["knowledge-browse", activeProject?.path],
    queryFn: () => fetchKnowledgeBrowse(activeProject!.path, 500),
    enabled: !!activeProject && !hasQuery,
    staleTime: 60_000,
  });

  // Search: when query >= 2 chars.
  const { data: searchRows, isLoading: searchLoading } = useQuery({
    queryKey: ["knowledge-search", activeProject?.path, trimmed],
    queryFn: () => fetchSearchKnowledge(activeProject!.path, trimmed, 200),
    enabled: !!activeProject && hasQuery,
    staleTime: 30_000,
  });

  // Friction: measured atrito — separate source, separate section.
  const { data: friction } = useQuery({
    queryKey: ["friction", activeProject?.path],
    queryFn: () => fetchFriction(activeProject!.path),
    enabled: !!activeProject,
    staleTime: 60_000,
  });

  // Instant in-memory refinement of the browse list when a query is typed.
  const refinedBrowse = useMemo<KnowledgeBrowseRow[]>(() => {
    if (!browseRows || !hasQuery) return browseRows ?? [];
    const q = trimmed.toLowerCase();
    return browseRows.filter(
      (r) =>
        r.name.toLowerCase().includes(q) ||
        r.description?.toLowerCase().includes(q) ||
        r.type.toLowerCase().includes(q),
    );
  }, [browseRows, hasQuery, trimmed]);

  // Group browse results by type, real-knowledge types first.
  const grouped = useMemo<[string, KnowledgeBrowseRow[]][]>(() => {
    const source = hasQuery ? refinedBrowse : (browseRows ?? []);
    const map = source.reduce<Record<string, KnowledgeBrowseRow[]>>((acc, row) => {
      (acc[row.type] ??= []).push(row);
      return acc;
    }, {});
    return Object.entries(map).sort(([a], [b]) => typeRank(a) - typeRank(b));
  }, [browseRows, refinedBrowse, hasQuery]);

  const searchResults: KnowledgeRow[] = hasQuery
    ? (searchRows ?? refinedBrowse)
    : [];

  return (
    <div className="flex flex-col gap-6 w-full">
      <PageHeader
        breadcrumb={[
          "Mustard",
          "Knowledge",
          ...(activeProject ? [{ label: activeProject.name, mono: true }] : []),
        ]}
        title="Knowledge"
        subtitle={activeProject?.name}
        description={
          <>
            O que o Mustard aprendeu rodando pipelines neste workspace, dividido
            em duas naturezas. <strong className="text-foreground/80">Padrões e
            decisões</strong> são conhecimento reutilizável — convenções de
            código, decisões de arquitetura e lições. <strong className="text-foreground/80">
            Atrito</strong> é o oposto: telemetria de fricção (retries de hook,
            pipelines pesadas) que indica onde o processo emperrou. Use a busca
            para localizar uma entrada específica.
          </>
        }
      />

      {/* Search */}
      <div className="relative w-full">
        <Search
          className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground"
          aria-hidden
        />
        <label htmlFor="knowledge-search" className="sr-only">
          Buscar conhecimento
        </label>
        <input
          id="knowledge-search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Buscar padrões, convenções, decisões, lições…"
          className="w-full pl-9 pr-3 py-2 bg-card border border-border rounded-md text-sm outline-none placeholder:text-muted-foreground focus:border-primary transition-colors"
        />
      </div>

      {/* Gate states */}
      {!projectsRoot ? (
        <EmptyState
          title="Diretório de projetos não configurado"
          description="Vá em Settings e aponte para a pasta onde estão seus repos."
        />
      ) : !activeWorkspaceId ? (
        <EmptyState
          title="Selecione um workspace"
          description="Use o seletor no topo da sidebar para escolher um projeto e ver o que ele aprendeu."
        />
      ) : !activeProject ? (
        <p className="text-[13px] text-muted-foreground">Carregando…</p>
      ) : hasQuery ? (
        // ── Search mode ─────────────────────────────────────────────────────
        searchLoading ? (
          <ul className="flex flex-col gap-1">
            {[0, 1, 2].map((i) => (
              <li key={i} className="h-8 bg-muted/40 rounded animate-pulse" />
            ))}
          </ul>
        ) : searchResults.length === 0 ? (
          <EmptyState
            title={`Nenhum resultado para "${trimmed}"`}
            description="Tente um termo mais curto, ou limpe a busca para ver tudo agrupado por tipo."
          />
        ) : (
          <section className="flex flex-col gap-2">
            <SectionHeader title="Resultados" right={`${searchResults.length}`} />
            <DataCard padded>
              <ul className="flex flex-col gap-0.5 text-sm">
                {searchResults.map((row) => (
                  <li
                    key={row.id}
                    className="flex items-baseline gap-2 flex-wrap px-2 py-1.5 rounded hover:bg-muted/40"
                  >
                    <Badge variant="secondary" className="text-[11px] py-0">
                      {labelType(row.type)}
                    </Badge>
                    <span className="font-mono font-medium text-[13px]">{row.name}</span>
                    {row.description && (
                      <span className="text-muted-foreground text-[12.5px] basis-full pl-1">
                        {truncate(row.description, 160)}
                      </span>
                    )}
                  </li>
                ))}
              </ul>
            </DataCard>
          </section>
        )
      ) : (
        // ── Browse mode ─────────────────────────────────────────────────────
        <div className="flex flex-col gap-8">
          {/* Padrões & decisões */}
          <section className="flex flex-col gap-3">
            <SectionHeader
              title="Padrões e decisões"
              description="Conhecimento reutilizável extraído das pipelines: convenções de código, decisões de arquitetura, padrões de nomenclatura e lições. O rótulo CONVENÇÃO aparece só para convenções de código de verdade."
              right={browseRows ? `${browseRows.length}` : undefined}
            />
            {browseLoading ? (
              <ul className="flex flex-col gap-1">
                {[0, 1, 2].map((i) => (
                  <li key={i} className="h-8 bg-muted/40 rounded animate-pulse" />
                ))}
              </ul>
            ) : !browseRows || browseRows.length === 0 ? (
              <EmptyState
                title="Nenhum padrão capturado ainda"
                description={
                  <>
                    O Mustard extrai padrões automaticamente ao final de cada
                    pipeline. Rode um <code className="font-mono">/mustard:feature</code>{" "}
                    ou <code className="font-mono">/mustard:bugfix</code>, ou
                    invoque <code className="font-mono">/mustard:knowledge</code>{" "}
                    para forçar uma extração.
                  </>
                }
              />
            ) : (
              <div className="flex flex-col gap-6">
                {grouped.map(([type, rows]) => (
                  <div key={type} className="flex flex-col gap-2">
                    <div className="flex items-baseline gap-2">
                      <h3 className="text-[11px] uppercase tracking-wider font-medium text-muted-foreground">
                        {labelType(type)}
                      </h3>
                      <span className="text-[11px] font-mono text-muted-foreground/50">
                        {rows.length}
                      </span>
                    </div>
                    <div className="grid grid-cols-1 lg:grid-cols-2 gap-2">
                      {rows.map((row) => (
                        <KnowledgeCard key={row.id} row={row} />
                      ))}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>

          {/* Atrito */}
          <FrictionSection friction={friction} />
        </div>
      )}
    </div>
  );
}

/**
 * Friction section — measured atrito, kept strictly separate from real
 * knowledge. Each entry shows its honest measured count (retries OR API calls)
 * and, when available, a prescriptive hint. Never labelled CONVENÇÃO.
 */
function FrictionSection({ friction }: { friction: FrictionEntry[] | undefined }) {
  return (
    <section className="flex flex-col gap-3">
      <SectionHeader
        title="Atrito"
        description="Sinais de fricção medidos durante as pipelines — não é conhecimento, é diagnóstico. Cada linha mostra o número real observado (retries de hook ou chamadas de API). É normal estar vazio: atrito alto é raro."
        right={friction ? `${friction.length}` : undefined}
      />
      {!friction || friction.length === 0 ? (
        <EmptyState
          title="Nenhum atrito registrado"
          description="As pipelines deste workspace rodaram sem fricção acima do limite (mais de 2 retries de hook ou mais de 50 chamadas de API por pipeline). Isso é bom — é o estado esperado."
        />
      ) : (
        <DataCard padded>
          <ul className="flex flex-col gap-2">
            {friction.map((f) => (
              <li
                key={f.name}
                className="flex flex-col gap-1 border-b border-border/40 last:border-b-0 pb-2 last:pb-0"
              >
                <div className="flex items-baseline gap-2 flex-wrap">
                  <AlertTriangle
                    className="h-3.5 w-3.5 text-amber-400 self-center shrink-0"
                    aria-hidden
                  />
                  <span className="font-mono font-medium text-[13px]">{f.name}</span>
                  {f.retry_count != null && (
                    <Badge
                      variant="outline"
                      className="text-[10px] border-amber-500/40 text-amber-300"
                      title="Retries de hook medidos nesta pipeline (sandbox/stash/re-prompt — não redespacho de agente)."
                    >
                      {f.retry_count} retries
                    </Badge>
                  )}
                  {f.api_calls != null && (
                    <Badge
                      variant="outline"
                      className="text-[10px] border-amber-500/40 text-amber-300"
                      title="Total de chamadas de API medidas nesta pipeline."
                    >
                      {f.api_calls} chamadas
                    </Badge>
                  )}
                  {f.updated_at && (
                    <span className="text-[11px] text-muted-foreground/60 ml-auto">
                      {relativeTime(f.updated_at)}
                    </span>
                  )}
                </div>
                <p className="text-[12.5px] text-muted-foreground leading-relaxed pl-6">
                  {f.description}
                </p>
                {f.prescription && (
                  <p className="text-[12px] text-emerald-400/90 leading-relaxed pl-6">
                    Sugestão: {f.prescription}
                  </p>
                )}
              </li>
            ))}
          </ul>
        </DataCard>
      )}
    </section>
  );
}
