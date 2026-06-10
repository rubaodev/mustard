# wave-3-ranking

## Resumo

P3+P5 — BM25 de ponto-fixo nos samples + âncoras match-first com fan-in log-amortecido + stop-file por inverse import frequency

## Rede

- Pai: [[redesenho-agnostico-indice-termos-digest]]
- Depende de: [[wave-1-grammars]], [[wave-2-classifier]]

## Tarefas

- [ ] Substituir a densidade bruta de build_terms por BM25: tf = contagem do termo no módulo, |D| = declarations.len(), avgdl = média sobre módulos, k1/b como dado em TOML; aritmética de ponto-fixo inteiro (score ×1024) e desempate por path ascendente para preservar a saída byte-estável
- [ ] Persistir o fan-in por módulo no modelo (o degree_map completo já é computado em graph.rs) — 1 inteiro aditivo por módulo
- [ ] Âncoras match-first: score = match (BM25 + co-ocorrência existente) + α·log2(1+fanin) com α pequeno — fan-in vira desempate, nunca dominante; hub só entra com match de termo nas declarações do módulo, não apenas no path
- [ ] Stop-file estrutural: módulo com fan-in acima de x% do total de módulos (x em TOML) sai da elegibilidade de âncoras — estatística do próprio repositório, sem nomes de tipo
- [ ] SOLID: se a soma amostragem+ranking estourar a coesão de digest.rs, extrair módulo próprio (ex.: rank.rs) mantendo digest.rs como face de orquestração — espelhar a decomposição existente do crate; sem fachadas
- [ ] Atualizar term_index.rs e anchor_ranking.rs (densidade→BM25; manter os testes de determinismo byte-a-byte)

## Arquivos

- `apps/scan/src/digest.rs`
- `apps/scan/src/graph.rs`
- `apps/scan/src/model.rs`
- `apps/scan/tests/term_index.rs`
- `apps/scan/tests/anchor_ranking.rs`
