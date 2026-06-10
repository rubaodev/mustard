# wave-2-frontend

## Resumo

Superficializar a adesão no dashboard: projetar digest_used + source_reads_before_digest no SpecCard a partir do analyze.digest.summary escopado ao spec, espelhar no tipo TS, registrar tema/i18n dos eventos analyze.digest.*, e renderizar um badge no card consumidor.

## Rede

- Pai: [[instrumentar-adesao-ao-digest-no]]
- Depende de: [[wave-1-backend]]

## Tarefas

- [ ] Em apps/dashboard/src-tauri/src/spec_views.rs: adicionar digest_used: bool e source_reads_before_digest: i64 ao struct SpecCard (#[serde(default)] para compat); em spec_card_v2_with_counts, dobrar o analyze.digest.summary do slice events (filtrar por event=="analyze.digest.summary" e spec == spec, pegar o mais recente por ts) e setar os dois campos; default digest_used=false / source_reads_before_digest=0 quando ausente.
- [ ] Em apps/dashboard/src-tauri/src/spec_views.rs: adicionar braços em feed_payload_summary para analyze.digest.summary (ex.: "digest usado · N reads antes") e analyze.digest.used.
- [ ] Em apps/dashboard/src/lib/types/specs.ts: adicionar digest_used?: boolean e source_reads_before_digest?: number à interface SpecCard.
- [ ] Em apps/dashboard/src/lib/phaseTheme.ts: adicionar entradas EVENT_THEME para analyze.digest.summary e analyze.digest.used com detailKey apontando para as novas chaves i18n.
- [ ] Em apps/dashboard/src/lib/i18n.ts: adicionar eventTheme.analyzeDigestSummary.detail e eventTheme.analyzeDigestUsed.detail (pt + en) e os rótulos do badge.
- [ ] Localizar o componente consumidor de SpecCard (grep por tools_used/children_count em apps/dashboard/src) e renderizar um badge pequeno de adesão (ex.: "digest ✓ · 0 reads antes"), com empty state quando os campos estiverem ausentes.

## Arquivos

- `apps/dashboard/src-tauri/src/spec_views.rs`
- `apps/dashboard/src/lib/types/specs.ts`
- `apps/dashboard/src/lib/phaseTheme.ts`
- `apps/dashboard/src/lib/i18n.ts`
