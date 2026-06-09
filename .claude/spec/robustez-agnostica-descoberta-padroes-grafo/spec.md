# Robustez agnostica da descoberta de padroes: grafo de imports, indice de termos e ancoras, cabeamento de stacks e honestidade do scan spec

<!-- drafter:tone=didactic — Write this spec narrative in didactic tone — expand abbreviations on first use (AC = Acceptance Criteria, wave = onda) and prefer plain words over jargon. -->

<!-- PRD -->

## Contexto

Corrige os 6 defeitos da descoberta de padrões mapeados pela auditoria de 2026-06-09 (workflow de 4 agentes, achados verificados com file:line; ver memória `project-mustard-pattern-discovery-audit`). **Restrição central do usuário: PHP/Laravel é apenas um caso — toda correção é no motor genérico e deve valer (e ser testada) para TODAS as linguagens parametrizadas: C#, Python, TypeScript/TSX, Go, Rust e PHP.** Nada de fix específico de linguagem; a invariante agnóstica do scanner permanece (nenhum nome de linguagem/stack em `src/`).

> Nota-prova: as "Âncoras (do scan)" que o scaffold gerou automaticamente para ESTA spec vieram erradas (arquivos do dashboard, `install_nerd_font.rs`) — é o próprio defeito de âncoras que esta spec corrige.

Âncoras reais (da auditoria, verificadas):
- `apps/scan/src/graph.rs:191-226` — resolução de import: FQCN com `\` (PHP) nunca resolve (ns_index é por namespace; ramo de path exige `/`) → 0 edges, sem hubs/touchpoints/camadas.
- `apps/scan/src/digest.rs:268-323` — `build_terms`/`tokenize`: sem stopwords; `MAX_TERMS=120` por frequência DESC seleciona a favor do glue (and=193) e dropa termos discriminativos (`php`/`grammar`/`stack` fora do índice — verificado).
- `apps/scan/src/digest.rs:276` — samples por termo = 3 primeiros módulos na ordem da walk (viés alfabético: apps/cli monopoliza).
- `apps/scan/src/digest.rs:170-209` — `query()`: âncoras herdam ordem frequência-desc; sem raridade nem co-ocorrência; `QueryResult` (135-156) omite `detected_stacks`.
- `apps/scan/src/spec.rs:504-520` — `pick_slice`: `--like` sem match cai SILENCIOSAMENTE no slice mais "rico" (richness ignora confiança); `spec.rs:208-210` imprime "Mirrored... (verified in the model)" sem verificação; `spec.rs:38` suprime o banner NO PRECEDENT.
- `packages/core/src/domain/scan.rs:66-88` — `DigestQuery` omite stacks → `mustard-rt run feature` nunca vê.
- `apps/rt/src/commands/scan_claude.rs:163,418` — facts line dos Guards usa deps cruas (`frameworks=`), não `detected_stacks`; parser em `apps/rt/src/commands/scan_guards/list.rs:133-166`, preserve em `apply.rs:151-154`; payload do feature em `apps/rt/src/commands/feature.rs:45-65`.

Por que agora: o motor de stacks (spec `motor-deteccao-stack-por-evidencia`, fechada) entregou detecção que hoje é JSON sem leitor, e o objetivo "funcionar para Laravel e para as linguagens existentes" esbarra nesses 6 defeitos do aparelho de descoberta.

## Usuários/Stakeholders

O orquestrador (`/feature` ganha âncoras e termos relevantes + stacks visíveis no digest), os agentes de pipeline (facts line dos Guards com stack nomeada em vez de deps cruas) e qualquer repo escaneado em qualquer linguagem parametrizada (grafo de imports vivo → hubs/camadas/touchpoints).

## Métrica de sucesso

Num scan do próprio repo mustard: termos discriminativos entram no índice (ex.: `grammar`, `stack`), "and"/"from" saem, e as âncoras de uma query "scan detection" apontam para `apps/scan/*`. Num repo com imports internos em QUALQUER das 6 linguagens: `graph.edges > 0`. O digest de query e a facts line carregam as stacks detectadas. `scan spec` com `--like` sem match avisa em vez de fingir verificação.

## Não-Objetivos

- Mexer nos thresholds do miner (`MIN_ROLE_PARTNERS`/`MIN_CLUSTER`) ou filtrar pseudo-entidades (`use*`/`dashboard*`) — risco de desestabilizar modelos existentes; exige corpus de validação próprio (spec futura).
- Reinstalar binários (`~/.mustard`) — disruptivo; fica como passo manual pós-spec.
- IA/heurística não-determinística; catálogo curado de frameworks (a lista `frameworks` crua permanece como está).
- Tocar a mineração de convenções em si (`mine.rs`) — os miners de papéis/slices ficam intactos.

## Critérios de Aceitação

- **AC-1** — Grafo de imports resolve genericamente para TODAS as linguagens parametrizadas: fixtures com imports internos em C#, Python, TypeScript, Go, Rust e PHP produzem `graph.edges > 0`
  Command: `cargo test -p scan graph_resolution`
- **AC-2** — Índice de termos sem stopwords (dado, não hardcode) e sem dropar termos discriminativos; samples por densidade do termo no módulo
  Command: `cargo test -p scan term_index`
- **AC-3** — Âncoras ranqueadas por raridade (count asc) + co-ocorrência de múltiplos termos da query, determinístico
  Command: `cargo test -p scan anchor_ranking`
- **AC-4** — `detected_stacks` flui no digest de query do scan (`QueryResult`)
  Command: `cargo test -p scan digest_query_stacks`
- **AC-5** — `DigestQuery` do core expõe as stacks + registro com 3ª stack de ecossistema npm provando generalidade
  Command: `cargo test -p mustard-core stacks`
- **AC-6** — facts line dos Guards carrega `stacks=` (gerador + parser + preserve) e o payload de `mustard-rt run feature` expõe as stacks
  Command: `cargo test -p mustard-rt stacks_facts`
- **AC-7** — `scan spec` honesto: `--like` sem match gera aviso explícito no draft (sem banner "verified" falso) e o fallback considera recorrência/confiança
  Command: `cargo test -p scan spec_like`
- **AC-8** — Suíte completa do scan verde (regressões de digest/facts/spec/fixtures cobertas)
  Command: `cargo test -p scan`

<!-- PLAN -->

## Arquivos

Panorâmico (detalhe por onda nas sub-specs):

**Onda 1 — grafo (apps/scan):**
- `apps/scan/src/graph.rs` — resolução genérica de import: normalizar separadores de namespace (`\`, `.`, `::`, `/`) e, quando o import for FQCN de tipo (não casa namespace direto), retentar com o último segmento removido. Sem nome de linguagem na lógica.
- `apps/scan/tests/` — fixtures mínimas com imports internos em C#, Python, TypeScript, Go, Rust e PHP + teste `graph_resolution_*` assertando `edges > 0` por linguagem.

**Onda 2 — termos/âncoras/stacks-no-query (apps/scan):**
- `apps/scan/src/digest.rs` — stopwords aplicadas em `tokenize`/`build_terms`; estratégia do cap (indexar tudo, capar só a resposta — ou descartar a banda glue); samples por densidade do termo no módulo; ranking de âncora por raridade + co-ocorrência; `QueryResult` ganha `detected_stacks` (copiado do modelo).
- `apps/scan/stopwords.toml` (ou nome equivalente) — **novo**, lista de stopwords como DADO (mantém o guard agnóstico).
- `apps/scan/tests/` — testes `term_index_*`, `anchor_ranking_*`, `digest_query_stacks_*`.

**Onda 3 — honestidade do scan spec (apps/scan):**
- `apps/scan/src/spec.rs` — `pick_slice`: `--like` sem match emite nota explícita no draft e reativa o banner NO PRECEDENT; suprimir "Mirrored ... (verified in the model)" quando não verificado; richness passa a considerar recorrência/confiança antes da contagem de papéis.
- `apps/scan/tests/` — testes `spec_like_*`.

**Onda 4 — core (packages/core):**
- `packages/core/src/domain/scan.rs` — `DigestQuery` ganha o campo de stacks (serde default) parseado do `QueryResult`.
- `packages/core/src/domain/vocabulary/stacks.toml` — 3ª stack semeada de ecossistema npm (ex.: nextjs: dep `next` + marker `next.config.*` + assinatura) como DADO, provando generalidade além de php/python.

**Onda 5 — rt (apps/rt):**
- `apps/rt/src/commands/feature.rs` — payload de `mustard-rt run feature` expõe `stacks`.
- `apps/rt/src/commands/scan_claude.rs` — facts line ganha `stacks=name(conf)` a partir de `detected_stacks` (mantém `frameworks=` por compat).
- `apps/rt/src/commands/scan_guards/list.rs` + `apply.rs` — parser e preserve estendidos para `stacks=`.

## Dependências

- Ondas 1, 2 e 3 são independentes (arquivos disjuntos em apps/scan) — nível 0, paralelas.
- Onda 4 depende da 2 (contrato do campo no `QueryResult`).
- Onda 5 depende da 4 (consome o `DigestQuery` estendido).

## Limites

IN: resolução genérica do grafo, qualidade do índice de termos/âncoras (stopwords como dado), cabeamento de `detected_stacks` até o digest de query/facts line/payload do feature, honestidade do `--like`, fixtures multi-linguagem.
OUT: thresholds do miner e filtragem de pseudo-entidades; reinstall de binários; catálogo curado de frameworks; `mine.rs`.