# wave-1-grammars

## Resumo

P1+P8 — cobertura de membros nas tags.scm de todas as linguagens + manifesto de kinds + teste de paridade + proveniência upstream

## Rede

- Pai: [[redesenho-agnostico-indice-termos-digest]]

## Tarefas

- [ ] Auditar TODAS as linguagens declaradas em languages.toml contra o checklist de kinds de membro (method, property, field, enum_member) e completar os patterns faltantes em cada queries/<lang>/tags.scm — nenhuma linguagem é caso especial; a auditoria atual aponta as maiores lacunas em csharp e typescript, mas a autoridade é o teste de paridade, não a lista
- [ ] Criar apps/scan/queries/kinds-manifest.toml declarando os kinds suportados por linguagem (dado, nunca lógica): linguagem nova no futuro = 1 entrada em languages.toml + tags.scm + fixture, e o teste de paridade acusa lacuna sozinho — regra geral, não correção pontual
- [ ] Partir das tags.scm upstream (MIT) onde cobrirem mais e registrar proveniência/licença (README no diretório queries/)
- [ ] Ampliar as fixtures apps/scan/tests/fixtures/graph_<lang>/ com classe+método+propriedade+enum member por linguagem
- [ ] Criar o teste de paridade genérico (kinds_parity.rs): todo kind declarado no manifesto produz ≥1 declaração na fixture da linguagem — espelhar o padrão scan_fixture_labeled de graph_resolution.rs; o teste itera o manifesto, sem nomes de linguagem no corpo
- [ ] Acrescentar ao stopwords.toml os sufixos-cola de membro que inundariam o índice (ex.: async), com justificativa no header
- [ ] Verificar que a allowlist is_significant (mine.rs) permanece intocada — kinds de membro alimentam o índice de termos, não a mineração de papéis

## Arquivos

- `apps/scan/queries/csharp/tags.scm`
- `apps/scan/queries/typescript/tags.scm`
- `apps/scan/queries/go/tags.scm`
- `apps/scan/queries/python/tags.scm`
- `apps/scan/queries/rust/tags.scm`
- `apps/scan/queries/php/tags.scm`
- `apps/scan/queries/kinds-manifest.toml`
- `apps/scan/queries/README.md`
- `apps/scan/tests/kinds_parity.rs`
- `apps/scan/tests/fixtures/graph_csharp/`
- `apps/scan/tests/fixtures/graph_typescript/`
- `apps/scan/tests/fixtures/graph_go/`
- `apps/scan/tests/fixtures/graph_python/`
- `apps/scan/tests/fixtures/graph_rust/`
- `apps/scan/tests/fixtures/graph_php/`
- `apps/scan/stopwords.toml`
