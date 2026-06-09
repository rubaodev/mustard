# wave-3-impl

## Resumo

Honestidade do scan spec: --like sem match avisa (sem banner 'verified' falso) e fallback considera recorrencia/confianca

## Rede

- Pai: [[robustez-agnostica-descoberta-padroes-grafo]]

## Tarefas

- [ ] - [ ] Ler apps/scan/src/spec.rs: pick_slice (504-520, filtro --like por substring nas entidades do slice; fallback silencioso p/ max_by_key(richness)), richness (507, ordena por n_papeis ANTES de recorrencia, ignora confianca), banner 'Mirrored on the REAL files of <like> (verified in the model)' (208-210) impresso mesmo sem match, e a supressao do banner NO PRECEDENT (38: novel = like.is_empty() && !has_precedent).
- [ ] - [ ] Quando --like NAO casa nenhuma entidade de slice: (a) o draft deve carregar uma nota explicita (ex. 'like "<x>" not found in the model — fallback pattern below, treat as UNVERIFIED'); (b) NAO imprimir o banner Mirrored/verified; (c) o calculo de novel/NO PRECEDENT volta a valer como se --like nao tivesse sido passado.
- [ ] - [ ] Richness do fallback: priorizar recorrencia e confianca antes da contagem de papeis (ex. ordenar por (recorrencia, confianca, n_papeis, nome) ou incluir conf no criterio) — o caso real: convencao com recorrencia 11/conf 0.94 deve vencer pseudo-convencao de 2 familias de wrappers com 7 papeis/conf 0.82. Mudanca de ranking altera saidas existentes: atualize testes de spec deliberadamente, com comentario justificando.
- [ ] - [ ] Testes (nome exato p/ QA): `spec_like_*` — (1) --like sem match produz a nota e NAO produz 'verified in the model'; (2) --like com match real continua imprimindo Mirrored; (3) fallback prefere alta recorrencia/confianca.
- [ ] - [ ] Rodar `cargo test -p scan` completo.

## Arquivos

- `apps/scan/src/spec.rs`
- `apps/scan/tests/spec_like.rs`
