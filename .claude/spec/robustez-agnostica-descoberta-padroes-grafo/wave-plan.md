# Plano de Waves

## Tabela de Waves

| Wave | Spec | Papel | Depende de | Resumo |
|------|------|-------|------------|--------|
| 1 | [[wave-1-impl]] | impl | — | Grafo de imports: resolucao generica de namespace/FQCN para TODAS as linguagens parametrizadas |
| 2 | [[wave-2-impl]] | impl | — | Indice de termos (stopwords como dado + cap), samples por densidade, ancoras por raridade+co-ocorrencia, detected_stacks no QueryResult |
| 3 | [[wave-3-impl]] | impl | — | Honestidade do scan spec: --like sem match avisa (sem banner 'verified' falso) e fallback considera recorrencia/confianca |
| 4 | [[wave-4-impl]] | impl | [[wave-2-impl]] | Core: DigestQuery expoe stacks + 3a stack npm semeada no registro (generalidade alem de php/python) |
| 5 | [[wave-5-impl]] | impl | [[wave-4-impl]] | RT: payload do feature expoe stacks + facts line dos Guards ganha stacks= (gerador, parser, preserve) |

## Critérios de Aceitação
- AC-1 — Grafo resolve genericamente p/ todas as linguagens parametrizadas (edges>0 com imports internos em C#, Python, TypeScript, Go, Rust e PHP): `cargo test -p scan graph_resolution`
- AC-2 — Indice sem stopwords (dado) e sem dropar termos discriminativos; samples por densidade: `cargo test -p scan term_index`
- AC-3 — Ancoras por raridade + co-ocorrencia, deterministico: `cargo test -p scan anchor_ranking`
- AC-4 — detected_stacks flui no digest de query (QueryResult): `cargo test -p scan digest_query_stacks`
- AC-7 — scan spec honesto: --like sem match avisa sem banner falso; fallback considera recorrencia/confianca: `cargo test -p scan spec_like`
- AC-5 — DigestQuery do core expoe stacks + 3a stack npm no registro: `cargo test -p mustard-core stacks`
- AC-6 — facts line carrega stacks= (gerador+parser+preserve) e payload do feature expoe stacks: `cargo test -p mustard-rt stacks_facts`
