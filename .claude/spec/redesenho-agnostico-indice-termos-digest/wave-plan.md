# Plano de Waves

## Tabela de Waves

| Wave | Spec | Papel | Depende de | Resumo |
|------|------|-------|------------|--------|
| 1 | [[wave-1-grammars]] | grammars | — | P1+P8 — cobertura de membros nas tags.scm de todas as linguagens + manifesto de kinds + teste de paridade + proveniência upstream |
| 2 | [[wave-2-classifier]] | classifier | — | P4 — classificação generated/vendored/lockfile/minified no scan via catálogo de marcadores + demoção (não exclusão) no digest |
| 3 | [[wave-3-ranking]] | ranking | [[wave-1-grammars]], [[wave-2-classifier]] | P3+P5 — BM25 de ponto-fixo nos samples + âncoras match-first com fan-in log-amortecido + stop-file por inverse import frequency |
| 4 | [[wave-4-matching]] | matching | [[wave-3-ranking]] | P2 — escada de match por tiers (exato > fold > stem same-language > glossário) + novo contrato matched k/n no QueryResult/DigestQuery + consumidores |
| 5 | [[wave-5-polish]] | polish | [[wave-4-matching]] | P6+P7 — estratificação de samples por subprojeto + diversidade MMR + peso por classe de kind no catálogo publicado |

## Critérios de Aceitação
- **AC-1** — Paridade de kinds: toda linguagem extrai os kinds de membro declarados no manifesto (fixture por linguagem). Command: `cargo test -p scan --test kinds_parity`
- **AC-2** — Classificação de gerados: fixture com marcador vira file_class=generated, fica fora de samples/âncoras e a consulta responde generated_only quando só gerado casa. Command: `cargo test -p scan --test generated_class`
- **AC-3** — Ranking: samples por BM25 de ponto-fixo e âncoras match-first com fan-in amortecido, determinismo byte-a-byte. Command: `cargo test -p scan --test term_index --test anchor_ranking`
- **AC-4** — Escada de tiers: "cores" não casa "core", "cancelado" não casa "cancel" sem glossário, "parentid" casa exato e "cancelado" casa via glossário com tier reportado. Command: `cargo test -p scan --test match_tiers`
- **AC-5** — Contrato novo no core: report com matched k/n e razão desserializa a saída real do digest. Command: `cargo test -p mustard-core`
- **AC-6** — Consumidor atualizado: o payload do run feature expõe o report por termo. Command: `cargo test -p mustard-rt feature`
- **AC-7** — Estratificação: com dois subprojetos casando, cada estrato garante ao menos uma vaga nos samples. Command: `cargo test -p scan --test stratified_samples`
- **AC-8** — Workspace inteiro verde. Command: `cargo test --workspace`
