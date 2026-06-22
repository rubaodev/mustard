# wave-2-impl

## Resumo

Indice de termos (stopwords como dado + cap), samples por densidade, ancoras por raridade+co-ocorrencia, detected_stacks no QueryResult

## Rede

- Pai: [[robustez-agnostica-descoberta-padroes-grafo]]

## Tarefas

- [ ] - [ ] Ler apps/scan/src/digest.rs: build_terms (268-286, MAX_TERMS=120 por frequencia DESC), tokenize (294-323, so corta <3 chars), samples (276, primeiros-3 modulos na ordem da walk), query()/anchors (170-209), QueryResult (135-156).
- [ ] - [ ] Criar a lista de stopwords como DADO (novo arquivo .toml ao lado de languages.toml, ex. apps/scan/stopwords.toml, embutido via include_str! ou pelo build.rs como as queries): glue ingles comum em identificadores (and, from, when, with, the, for, into, that, this, get, set...). Justifique a lista no cabecalho do arquivo. NADA hardcoded em src/ alem do include do arquivo de dado.
- [ ] - [ ] Aplicar stopwords em tokenize/build_terms. Mudar a estrategia do cap: NAO 'top-120 por frequencia' (mantem glue, dropa dominio) — indexar todos os termos e capar apenas a RESPOSTA da query, OU descartar a banda mais frequente antes do cap; escolha a opcao mais simples que faca `grammar`/`stack` entrarem e `and`/`from` sairem (prove no teste). Atencao a memoria/latencia: o indice e construido por chamada de digest, nao persiste no modelo.
- [ ] - [ ] Samples por termo = top-3 modulos POR CONTAGEM do termo no modulo (densidade), desempate estavel (ordem alfabetica do path) — elimina o vies alfabetico apps/cli.
- [ ] - [ ] Ancoras em query(): ranquear termos casados por RARIDADE (count asc) e dar bonus de co-ocorrencia (arquivo presente nos samples de >=2 termos da query sobe). Deterministico, desempates estaveis.
- [ ] - [ ] QueryResult ganha `detected_stacks` (mesmo shape do CapabilityDigest, copiado do modelo em query()).
- [ ] - [ ] Testes (nomes exatos p/ QA): `term_index_*` (stopwords fora; termo raro discriminativo entra; termo de teste 'and' NAO casa), `anchor_ranking_*` (raridade + co-ocorrencia + determinismo), `digest_query_stacks_*` (QueryResult carrega as stacks da fixture laravel).
- [ ] - [ ] Rodar `cargo test -p scan` completo — consumidores do digest (spec-draft context, glossary-coverage no rt) leem terms/anchors: se algum teste de scan quebrar por ordem nova de ancoras (melhoria intencional), atualizar com justificativa.

## Arquivos

- `apps/scan/src/digest.rs`
- `apps/scan/stopwords.toml`
- `apps/scan/tests/term_index.rs`
- `apps/scan/tests/anchor_ranking.rs`
