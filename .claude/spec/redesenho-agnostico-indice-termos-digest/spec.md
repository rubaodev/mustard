# Redesenho agnostico do indice de termos e digest do scan: cobertura de membros nas tags.scm com teste de paridade, classificacao de arquivos gerados, BM25 nos samples, ancoras match-first, escada de match multilingue e relatorio matched k/n no lugar do miss booleano

<!-- drafter:tone=didactic — Write this spec narrative in didactic tone — expand abbreviations on first use (AC = Acceptance Criteria, wave = onda) and prefer plain words over jargon. -->

<!-- PRD -->

## Contexto

Redesenho agnostico do indice de termos e digest do scan: cobertura de membros nas tags.scm com teste de paridade, classificacao de arquivos gerados, BM25 nos samples, ancoras match-first, escada de match multilingue e O scan minera o repositório num modelo durável e o digest responde, de forma determinística, "onde no código vive este pedido" — é a bússola que evita ler o repositório inteiro a cada feature. Uma execução real num monorepo com frontend TypeScript e backend C# mostrou que essa bússola falha de quatro formas: o índice só enxerga nomes de tipos (métodos e propriedades ficam invisíveis), arquivos gerados por ferramentas monopolizam as amostras, tipos compartilhados muito importados dominam as âncoras, e um pedido escrito em português casa por falso cognato com o vocabulário em inglês do código — devolvendo confiança falsa ("encontrei precedente") em cima de ruído. O resultado prático: o agente recebe âncoras erradas e o diagnóstico certo só sai por exploração manual. Este redesenho torna a descoberta confiável para qualquer linguagem de programação e qualquer idioma do pedido, por construção: cobertura de membros vira regra verificada por teste, gerados são reconhecidos e demovidos, o ranking prioriza correspondência real, e a resposta declara honestamente o que casou e o que não casou.

Âncoras (do scan):
- packages/core/src/domain/ast/entity.rs
- packages/core/src/domain/vocabulary/language_caps.rs
- packages/core/src/domain/scan.rs
- apps/rt/src/commands/review/dependency_precheck.rs
- apps/rt/src/commands/doctor/language_audit.rs
- apps/rt/src/commands/spec/approve_spec.rs
- packages/core/src/domain/ast/loader.rs
- apps/dashboard/src-tauri/src/spec_views.rs
- apps/rt/src/commands/wave/wave_files.rs
- apps/dashboard/src/features/trace/tool-palette.ts
- apps/dashboard/src/features/workspace/WorkspaceDigest/index.tsx
- apps/rt/src/util/sha256.rs

Fatias recorrentes (precedente a espelhar): args (×2)

Por que agora: a falha foi comprovada em uso real e cada feature nova consultada em português paga o mesmo custo — âncoras ruins desperdiçam a exploração do agente e corroem a confiança no pipeline.

## Usuários/Stakeholders

Os orquestradores de pipeline (a inteligência artificial que conduz /feature e /bugfix consumindo o digest) e, por consequência, todos os times que usam o Mustard em seus projetos — o caso que motivou o redesenho veio de um time com backend C# e pedidos escritos em português. Solicitante: Rubens.

## Métrica de sucesso

Numa consulta de feature sobre um repositório de teste, as âncoras devolvidas apontam o código-fonte relevante (não arquivos gerados nem tipos-cola muito importados), e quando algum termo do pedido não tem correspondência o resultado declara explicitamente quanto casou e o que ficou de fora — tanto para pedido em português quanto em inglês.

## Não-Objetivos

Tradução automática de propósito geral; qualquer técnica neural ou de embeddings; detecção automática de idioma; PageRank personalizado (candidato a uma segunda versão); alinhar o compilador de spec à nova escada de match (fase própria); re-escanear os projetos consumidores (operação pós-instalação, não código desta spec).

## Critérios de Aceitação

- **AC-1** — Paridade de kinds: toda linguagem extrai os kinds de membro declarados no manifesto (fixture por linguagem).
  Command: `cargo test -p scan --test kinds_parity`
- **AC-2** — Classificação de gerados: fixture com marcador vira file_class=generated, fica fora de samples/âncoras e a consulta responde generated_only quando só gerado casa.
  Command: `cargo test -p scan --test generated_class`
- **AC-3** — Ranking: samples por BM25 de ponto-fixo e âncoras match-first com fan-in amortecido, determinismo byte-a-byte.
  Command: `cargo test -p scan --test term_index --test anchor_ranking`
- **AC-4** — Escada de tiers: "cores" não casa "core", "cancelado" não casa "cancel" sem glossário, "parentid" casa exato e "cancelado" casa via glossário com tier reportado.
  Command: `cargo test -p scan --test match_tiers`
- **AC-5** — Contrato novo no core: report com matched k/n e razão desserializa a saída real do digest.
  Command: `cargo test -p mustard-core`
- **AC-6** — Consumidor atualizado: o payload do run feature expõe o report por termo.
  Command: `cargo test -p mustard-rt feature`
- **AC-7** — Estratificação: com dois subprojetos casando, cada estrato garante ao menos uma vaga nos samples.
  Command: `cargo test -p scan --test stratified_samples`
- **AC-8** — Workspace inteiro verde.
  Command: `cargo test --workspace`

<!-- PLAN -->

## Resumo

Oito componentes (P1-P8) do desenho pesquisado e validado adversarialmente em docs/REDESENHO-INDICE-DIGEST-AGNOSTICO.md, distribuídos em cinco ondas: (1) cobertura de membros nas tags.scm com manifesto de kinds e teste de paridade; (2) classificação de arquivos gerados/vendorizados com catálogo de marcadores e demoção; (3) BM25 de ponto-fixo nos samples e âncoras match-first com fan-in amortecido; (4) escada de match por tiers com stoplists/stemmer/glossário e novo contrato matched k/n; (5) estratificação por subprojeto + diversidade MMR e proteção do catálogo publicado.

## Arquivos

- apps/scan/queries/{csharp,typescript,go,python,rust,php}/tags.scm — patterns de membro (onda 1)
- apps/scan/queries/kinds-manifest.toml (novo) + apps/scan/queries/README.md (proveniência) — onda 1
- apps/scan/tests/kinds_parity.rs (novo) + apps/scan/tests/fixtures/graph_*/ (ampliar) — onda 1
- apps/scan/stopwords.toml — sufixos-cola de membro (onda 1)
- apps/scan/generated-markers.toml (novo) + apps/scan/src/{ingest,model,main}.rs + apps/scan/tests/generated_class.rs (novo) — onda 2
- apps/scan/src/digest.rs — elegibilidade/demoção (onda 2), BM25 + âncoras match-first (onda 3), escada de tiers (onda 4), estratificação/MMR + peso por kind (onda 5)
- apps/scan/src/graph.rs + persistência de fan-in no modelo — onda 3
- apps/scan/tests/{term_index,anchor_ranking}.rs — atualização (onda 3)
- apps/scan/src/stemmers.rs (novo) + apps/scan/stoplists/{pt,en}.txt (novo) + apps/scan/lexicons/pt-en.toml (novo) + apps/scan/Cargo.toml (dep rust-stemmers) — onda 4
- packages/core/src/domain/scan.rs — contrato DigestQuery (onda 4)
- apps/rt/src/commands/feature.rs — payload do consumidor (onda 4)
- apps/scan/tests/match_tiers.rs (novo, onda 4) + apps/scan/tests/stratified_samples.rs (novo, onda 5)

## Limites

IN: índice de termos, ranking e extração do scan (apps/scan); contrato DigestQuery no core (packages/core); consumidor feature.rs no rt (apps/rt); dados novos (manifesto de kinds, catálogo de gerados, stoplists, léxicos).
OUT: compilador de spec (token_seq_in) — alinhamento em fase própria; dashboard; prosa de skills; re-scan de projetos externos e reinstalação de binários (pós-deploy); conteúdo de glossário CONTEXT.md por projeto (só a infraestrutura entra).

## Dependências

- rust-stemmers (MIT, Rust puro) — única dependência nova de crate.
- Dados vendorizados: Snowball stop.txt (pt/en); tags.scm upstream como ponto de partida (MIT), com proveniência registrada.

## Decisões não-óbvias

- `scan spec --entity` não foi usado na lapidação: as unidades são infraestrutura da própria ferramenta (sem precedente de entidade de produto); o desenho lapidado vem do documento de pesquisa em docs/.
- Kinds de membro ficam fora da mineração de papéis (allowlist `is_significant` intocada) — papéis continuam derivados só de tipos.
- BM25 com aritmética de ponto-fixo inteiro (score ×1024) — floats de libm variam entre plataformas e quebrariam a saída byte-estável.
- Carve-out único de agnosticismo: módulo espelho-de-dado mapeando código de idioma→algoritmo de stemmer (idiomas naturais, nunca linguagens de programação).
- Marcadores gravados no catálogo com escape unicode para o auto-scan não classificar o próprio catálogo como gerado.
- C# e TypeScript aparecem no plano apenas como evidência do caso motivador e como as maiores lacunas da auditoria — nunca como caso especial: nenhuma linguagem de programação entra em src/ (guard do scan); toda menção por-linguagem vive em dado (languages.toml, tags.scm, kinds-manifest.toml, generated-markers.toml), e linguagem nova = só arquivos de dado + o teste de paridade cobra sozinho.
- SOLID e padrão do crate: cada responsabilidade nova vira módulo coeso próprio (classificação de arquivos, escada de match, stemmer espelho-de-dado), espelhando o padrão um-módulo-por-responsabilidade já existente (graph.rs, extract.rs, mine.rs); digest.rs permanece face de orquestração — não pode virar god-file; sem wrappers/fachadas, chamadas módulo-qualificadas diretas.

## Preocupações

- A troca do `miss: bool` pelo report estruturado muda o contrato do QueryResult — consumidores no rt (feature.rs; glossary_coverage lê só nomes de termo) atualizam na mesma onda 4.
- Crescimento do modelo (~×3-5 nas declarações) e rebaseline único de goldens — esperado, não é flakiness.
- Specs ativas pré-existentes no diretório de specs não bloquearam esta abertura (decisão explícita do usuário).
- Pós-merge: rebuild dos binários + re-scan dos projetos consumidores para o grain novo valer.
- Observação de ferramenta (follow-up, fora do escopo): o `wave-dependency` instalado só aceita a forma de derivação `{files: [...]}`; o modo de validação de plano descrito em refs/feature/wave-decomposition.md respondeu `empty-input` para a forma `{waves: [...]}` — deriva entre prosa e binário. O DAG desta spec foi autorado no formato canônico `wave-N-role` e validado manualmente.