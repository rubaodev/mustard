# Redesenho agnóstico do índice de termos e do digest

> Status: proposta pesquisada e adversarialmente verificada — **não aprovada, não executada**.
> Data: 2026-06-10. Origem: run real de feature no sialia (payables) + 3 experimentos empíricos + pesquisa web (aider/tree-sitter, Zoekt/Sourcegraph, GitHub Blackbird, linguist, literatura CLIR/BM25/MMR).
> Invariantes respeitados: determinístico, byte-estável, sem IA/rede no scan, zero nomes de linguagem-de-programação em `src/` (dado em TOML/.scm), agnóstico a linguagem de programação E a idioma natural (intent PT ou EN; identificadores EN ou PT). C# aparece neste documento apenas como **exemplo e evidência do caso motivador** — nenhum componente cita C# em código; toda menção por-linguagem ou por-ferramenta vira **entrada de dado** (tags.scm, manifesto de kinds, catálogo de marcadores), e linguagem nova entra só com arquivos de dado, cobrada automaticamente pelo teste de paridade.
> Princípios de implementação: SOLID — um módulo coeso por responsabilidade nova (classificação, escada de match, stemmer espelho-de-dado), espelhando a decomposição existente do crate (`graph.rs`, `extract.rs`, `mine.rs`); `digest.rs` permanece a face de orquestração; sem wrappers/fachadas, chamadas módulo-qualificadas diretas.

## 1. Contexto e evidência

Uma feature real (badges + hierarquia de desdobramento de títulos) num monorepo Next.js + submodule backend C# expôs cinco defeitos do digest, todos verificados com reprodução:

| # | Defeito | Evidência |
|---|---------|-----------|
| 1 | Cobertura de membros desigual entre linguagens: a `tags.scm` de C# captura só tipos (class/interface/record/struct/enum) | `SplitAsync` = 0 ocorrências no grain inteiro; `ParentId` backend = 0 (`queries/csharp/tags.scm:13-17`) |
| 2 | Samples por densidade bruta (top-3) favorecem arquivos gerados com dezenas de decls | `PayableService.cs` é 48º/107 sob `payable`; zero dos 48 .cs do módulo Payables viram sample |
| 3 | Hubs por fan-in puro | 3 enums compartilhados grau-474 dominaram as âncoras |
| 4 | Intent PT casa por falso cognato via prefix-match ≥4 | `cancelado→cancel` (145), `cores→core`; 3/16 termos casados |
| 5 | `miss` booleano gera falsa confiança | `miss=false` + "espelhe as slices" apontando fluxos de Cancel irrelevantes |

Fixes só de consulta (stopwords/tri-state/ranking/glossário) foram **refutados** para o núcleo do problema: operam acima de um índice cujo recall estrutural exclui o local do bug. O redesenho abaixo ataca recall (mineração) e precisão (consulta) juntos.

## 2. Desenho final, por prioridade

### Decisivos

**P1 — Cobertura de membros nas tags.scm + teste de paridade data-driven** [defeito 1] [100% dado] [custo S]
- Estender cada `queries/<lang>/tags.scm` contra um **checklist fixo de kinds**: `definition.{method,property,field,enum_member}` (constructor fica fora: repete o nome do tipo). Exemplo C# (+6 patterns): `method_declaration`, `property_declaration`, `field_declaration > variable_declarator`, `enum_member_declaration`. TypeScript é o segundo pior (`method_definition`, `public_field_definition`, `property_signature`).
- O motor já copia o sufixo de kind verbatim (`extract.rs:100-111`) e descarta pattern inválido individualmente — a extensão é colar dado, zero código.
- **Decisão deliberada**: kinds novos NÃO entram na allowlist `is_significant` (`mine.rs:576-581`) — membros alimentam o índice de termos, mas ficam fora da mineração de roles (roles continuam cegos a ruído de membro).
- **Teste de paridade em CI**: fixture mínima por linguagem (classe+método+propriedade+enum member) + manifesto de kinds suportados por linguagem (dado) + asserção genérica "todo kind declarado produz ≥1 decl na fixture". Esse teste teria pego o buraco do C# automaticamente — e pega o da próxima linguagem.
- `stopwords.toml` ganha os sufixos-cola de membro que inundam o índice (`async`, etc.), mesma política declarada do get/set.

**P2 — Escada de match por tiers; morte do prefix-match ≥4** [defeitos 4 e 5] [lógica agnóstica + dados por idioma] [custo M]
- `token_match` (`digest.rs:174-176`) vira escada com igualdade EXATA em todo tier: **T1** token exato OU identificador-inteiro exato (indexar também o ident lowercased: `splitasync`, `parentid`) > **T2** accent-fold > **T3** stem same-language (rust-stemmers; stem **nunca** atravessa idioma) > **T4** glossário de domínio bilíngue estilo Pirkola (traduções como sinônimos OR, nunca substituição; `lexicons/<src>-<dst>.toml` = dado curado, extensível por projeto).
- Idiomas da consulta = `dedup([lang do mustard.json raiz, "en"])` — **zero detecção de idioma**. Índice ganha colunas por token: cru, fold, stem_en, stem_lang-config (simétrico: cobre identificador PT + intent EN).
- Pesos com hierarquia Zoekt (exato ≫ fold ≫ stem ≫ glossário, ~10× por degrau). `cores→core` e `cancelado→cancel`-por-prefixo morrem por construção; "cancelado" casa honestamente via T4 (cancelar→cancel).
- **Saída substitui `miss: bool`**: por termo `{term, tier, lang, files}` + agregado `matched k/n` + razão `none|generated_only|weak|strong`. Termo sem match = miss nomeado.
- Stoplists naturais: vendorizar Snowball `stop.txt` PT/EN como dado. **Rejeitado stopwords-iso** (contém "estado", "valor", "tipo" — mataria intents de domínio).

**P3 — BM25 no lugar de densidade bruta nos samples** [defeito 2] [lógica agnóstica] [custo S]
- Em `build_terms` (`digest.rs:367-371`): score = BM25 com `tf` = contagem do termo no módulo, `|D|` = `declarations.len()`, `avgdl` = média, `k1=1.2`, `b=0.75`. Saturação mata a monopolização (52 ocorrências ≠ 26× 2 ocorrências); normalização por tamanho derruba o arquivo gerado gigante.
- **Requisito de byte-estabilidade**: ponto-fixo inteiro (score ×1024) OU crate `libm` puro-Rust; empates por path asc. Sem isso, o invariante quebra entre plataformas.

**P4 — Classificação gerado/vendorizado no scan + demoção no digest** [defeitos 2, 3, 5] [motor genérico ~30 linhas + `generated-markers.toml` 100% dado] [custo M]
- Campo aditivo `file_class = normal|generated|vendored|lockfile|minified` + `marker`, persistido por módulo no grain (modelo Zoekt: classificar no indexador, nunca em query-time). Motor: "procure string/regex nas N primeiras / 2 últimas linhas / primeiro bloco de comentário, filtrado por glob" — zero nome de linguagem em src/.
- Catálogo cross-ecossistema: `@generated` (Meta/Phabricator), regex oficial do Go (`^// Code generated .* DO NOT EDIT\.$`), `<auto-generated` (Roslyn — crítico para C#), "Generated by Kubb", orval, OpenAPI Generator, protoc, paths `__generated__/`, `src/gen/`, `*.Designer.cs`, minificado por média de linha >110, lockfiles por basename. Overrides `.gitattributes` (`linguist-generated`) e `.editorconfig` (`generated_code`) sempre vencem.
- Política: `generated|vendored` inelegíveis para samples/âncoras/hubs mas PERMANECEM no índice com multiplicador (0.25, TOML) — termo que casa só em gerado responde `generated_only`, nunca silêncio. `lockfile|minified` saem do índice.
- Gotcha resolvido: markers gravados no TOML com escape unicode (`@generated`) para o self-scan não classificar o próprio catálogo como gerado.

**P5 — Âncoras match-first; fan-in subordinado e amortecido** [defeito 3] [lógica agnóstica] [custo M]
- Inverter para o modelo Zoekt (match 7000 : rank estático 10): âncora_score = score de match (BM25/tier + co-ocorrência) + `α·log2(1+fanin)` — fan-in vira desempate (474→8.9, 30→5.0: colapsa).
- **Stop-file estrutural**: módulo com fan-in > x% do total (TOML, ex. 20%) vira stopword de grafo nas âncoras ("inverse import frequency") — descoberto por estatística do próprio repo, não por nome de tipo.
- Encaixe: filtro de elegibilidade (file_class + iif) ANTES do `truncate(8)` em `graph.rs:125-127`; modelo persiste fan-in por módulo (`degree_map` já existe em `graph.rs:192`); hub passa a exigir match de termo nas decls, não só no path (`digest.rs:207`).

### Complementos

**P6 — Estratificação por subprojeto + diversidade MMR** [defeitos 2 e 3] [lógica agnóstica] [custo M]
- Estrato = `projects[].dir` (o grain já conhece). Se ≥2 estratos têm match, cada um garante ≥1 vaga nos samples. MMR greedy (λ=0.7) nas demais vagas (Jaccard de subtokens + diretório + vizinhança de imports). Em repo de 1 projeto, degenera limpo para ranking global.

**P7 — Proteção do catálogo publicado (MAX_TERMS) com peso por classe de kind** [regressão de P1] [lógica agnóstica] [custo S]
- Com membros, ocorrências ×~9. Catálogo publicado pondera kind tipo ×2.5, membro ×1 (vocabulário de kind genérico do motor, não nome de linguagem). `query` já busca o índice sem cap — nada se perde em lookup.

**P8 — Vendorização de queries upstream com proveniência** [defeito 1, sustentação] [dado] [custo S]
- Partir das tags.scm upstream (MIT) onde cobrirem mais, com README de licença (precedente aider). Atenção: a upstream de C# tem `method` mas NÃO tem property/struct/record/enum — **o checklist do P1 é a verdade, não a upstream**.

### Rejeitados (com motivo)

| Proposta | Motivo |
|---|---|
| Detecção de idioma (whatlang/lingua) | Intent curto e code-mixed = pior caso; idioma já existe por config root-wins |
| stopwords-iso | Contém substantivos de domínio ("estado", "valor", "tipo") |
| Stem cross-língua / dual-stemming | AMPLIFICA o falso cognato (`stem_en("cores")→"core"` recria o defeito); cross-língua é papel exclusivo do glossário |
| Dicionário PT-EN geral | Ambiguidade explode (~50% da eficácia monolíngue na literatura CLIR); glossário curado de domínio no lugar |
| PageRank personalizado (aider) | P5 resolve mais barato (modelo Zoekt); iteração float por query ameaça byte-estabilidade. Candidato a v2 |
| Backfill léxico (indexar todo identificador) | Cobertura se resolve na raiz (.scm + paridade); pressão em MAX_TERMS |
| Splitter Samurai/Spiral | Baseline camelCase + ident-inteiro (T1) cobre os casos reais; v2 opcional |
| Embeddings/neural | Invariante do produto |

## 3. Validação adversarial

**Guards do scan**: P1/P4/P8 são dado puro; P3/P5/P6/P7 aritmética agnóstica. Roles intocados. Furo real achado e tratado: o mapeamento código-ISO→stemmer do rust-stemmers cita idiomas **naturais** em src/ — carve-out explícito num módulo isolado espelho-de-dado (1 linha por idioma novo).

**Caso sialia ponto a ponto**: `SplitAsync` indexável (P1, + ident inteiro via T1); `ParentId` indexável (token `parent` + `parentid`); `PayableService.cs` alcançável por **três mecanismos independentes** (P4 demove Kubb, P3 normaliza, P6 garante vaga ao estrato do submodule); enums-474 fora do topo (P4 ou iif+log-damp+match-em-decls); intent PT degrada honesto com miss nomeado.

**Casos-espelho**: Python puro PASSA (paridade só exige kinds que a linguagem tem); identificadores PT + intent EN PASSA (índice com stem das duas línguas + glossário bidirecional); repo sem gerados PASSA (P4 no-op); monorepo de 1 projeto PASSA (estratificação degenera).

**Regressões tratadas**: inundação de verbos (P7+stopwords); modelo ×3-5 bytes (aceito); rebaseline único de goldens; float (requisito ponto-fixo); `spec::token_seq_in` mantém filosofia antiga nesta fase (alinhamento em spec própria).

## 4. Limitações assumidas

1. Carve-out do mapeamento de stemmer (idiomas naturais, módulo isolado).
2. Crescimento do modelo (×3-5) e do índice (colunas fold/stem).
3. Cognato verdadeiro fora do glossário vira miss honesto — preferível ao falso cognato; lexicon extensível por projeto.
4. `spec::token_seq_in` não alinhado nesta fase.
5. Gerado sem marcador e fora de path conhecido escapa do P4 — mitigação via `.gitattributes` do usuário.
6. Same-case identifiers (`splitline`) sem split — só ident-inteiro casa.
7. Miss honesto sobe a taxa de "não sei" reportada — consumidores em apps/rt precisam tratar `generated_only` e `matched k/n` (mudança de contrato do QueryResult).

## 5. Ordem de implementação

P1+P8 (destrava tudo: nenhum ranking recupera termo com 0 ocorrências) → P4 (classificação no modelo antes do ranking usar) → P3+P5 (ranking) → P2 (matching/saída) → P6+P7 (polimento). Exige rebuild dos binários + re-scan dos projetos.

## 6. Fontes

- P1/P8: tree-sitter-c-sharp `queries/tags.scm` · tree-sitter code-navigation docs · aider `repomap.py` + `aider/queries/tree-sitter-languages`
- P3: Okapi BM25 · Sourcegraph "Keeping it boring and relevant with BM25F"
- P5: zoekt `contentprovider.go` (match 7000 : rank 10) · Sourcegraph indexed-ranking
- P2: GitHub Blackbird ("a brief history of code search at GitHub": exact > partial) · rust-stemmers (MIT, puro-Rust) · Snowball `stop.txt` PT · Pirkola 1998 (structured queries CLIR) · InformationR 19-1 paper605 (truncamento N-chars = baseline inferior — fundamenta matar o prefix≥4)
- P4: zoekt `file_category.go`/`score.go` · linguist `generated.rb` · Roslyn `GeneratedCodeUtilities.cs` · Kubb `defineResolver.ts` · Go `^// Code generated .* DO NOT EDIT\.$`
- P6: Carbonell & Goldstein 1998 (MMR)

## 7. Encaixes locais

`apps/scan/queries/csharp/tags.scm:13-17` (P1, exemplo) · `apps/scan/src/digest.rs:174-176,242,351-377` (P2/P3) · `apps/scan/src/graph.rs:111-128,192` (P5) · `apps/scan/src/mine.rs:576-581` (gate de roles — NÃO mexer) · `apps/scan/stopwords.toml` (dado) · `apps/scan/src/extract.rs:100-111,122-139` (kind verbatim + tolerância a pattern inválido — viabiliza P1 sem código novo).
