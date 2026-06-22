<!-- mustard:generated -->
# use — examples in this codebase

<!-- mustard:enrich hash=3c219f59296b -->
## Purpose

Worked examples of the `use`-hook convention. `useActivityFeed.ts` fans `useQueries` across projects, flattens each `fetchRecentEvents` result into `ActivityFeedRow[]`, and sorts newest-first. `useAggregate.ts` is the heavier reducer — it runs parallel specs and events queries, then derives counters (active specs, executing, completed in 7 days, events today) plus active-pipeline and timeline lists. `useArtifactDrift.ts` shows the drift fan-out keyed by project path with `retry: false` and a 60s `staleTime`, returning a per-path map, plus a single-project `useIsMustardRepo` lookup.
<!-- /mustard:enrich -->

- Ref: apps/dashboard/src/hooks/useActivityFeed.ts
- Ref: apps/dashboard/src/hooks/useAggregate.ts
- Ref: apps/dashboard/src/hooks/useArtifactDrift.ts

