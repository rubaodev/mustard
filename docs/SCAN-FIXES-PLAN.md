# Plano-mestre: correções do `/scan` (agnóstico + SOLID)

> Origem: auditoria do `/scan` rodado sobre o próprio mustard (2026-05-29). Este documento é o registro
> de design das correções. Detalhe completo (file:line) das investigações em dois workflows de
> entendimento; este doc consolida os 7 vetores + 1 achado extra, as abstrações compartilhadas e a
> ordem de execução.

## Invariantes (constraints do usuário)

- **Agnóstico**: nenhum nome de linguagem/framework/arquitetura hardcoded; toda regra é predicado geral
  (convenção de path, forma estrutural, contagem) sobre dados que a detecção produz.
- **SOLID + DRY**: quando dois lugares precisam concordar, extrai-se uma função única (SSOT). Sem facade.
- **min-IA / max-Rust**: o Rust extrai o máximo e entrega um prompt **auto-contido**; a IA não lê arquivo.
  Quando o Rust detecta um campo incompleto, o **prompt** carrega a instrução de completar aquele ponto.
- **Skill no padrão canônico**: `SKILL.md` com `description` = *trigger* real (não stub, não escondido no corpo).

## Descoberta de fundo (amarra os pontos 1, 2, 7)

Vários artefatos **derivados** (promoção do `<!--desc-->` → `description:`; seções geradas do CLAUDE.md)
só são produzidos pelo **caminho de render**, que é *gated* pelo hash do código-fonte
(`scan_orchestrate.rs` classify → `dispatch` vs `skipped`). Num run **só-enrich** (fast-path, hash igual)
o render não roda, então a IA preenche o `<!--desc-->` no corpo mas **a promoção nunca acontece** — o
resolvedor segue lendo o stub genérico. Correção transversal: **separar** "regenerar esqueleto"
(legítimo gated por hash) de "aplicar artefatos derivados sobre a prosa enriquecida" (deve rodar sempre
que houver enrich novo) — via `enrich_block::reapply_derived` chamado no `finalize`.

---

## Os 7 vetores

### V1 — Falso-positivo de framework/arquitetura (di/orm) — ALTO
- **Causa**: `architecture.rs::detect_subproject_frameworks` (rt) casa keywords da vocabulary (Aho-Corasick)
  sobre o **texto cru** de todo arquivo, incluindo (a) doc-comments/testes do próprio
  `core/src/domain/vocabulary/frameworks.rs` que **define** as keywords, e (b) arquivos de teste.
  No `grain.model.json`: `_patterns.rust.frameworks=["di","framework","orm"]`, `architecture="layered"`.
- **Fix (agnóstico)**: exigir evidência admissível — excluir (i) arquivos de teste por convenção universal
  (`is_test_path`), (ii) arquivos que **definem** a vocabulary (`is_vocabulary_definition`, colocado junto da
  tabela de padrões para ficar em sincronia), e (iii) hits dentro de comentário (`is_comment`). Arquitetura:
  filtrar `paths`/`layer_edges` pelo mesmo conjunto admissível.
- **SSOT**: `core::…::is_test_path`, `core::…::is_comment` (promovido de `structural_extract.rs:569`),
  `frameworks::is_vocabulary_definition` / `architecture::is_vocabulary_definition`.

### V2 — Nós-lixo no grafo vindos de teste + nome de entidade malformado — MÉDIO
- **Causa**: (1) `file_utils::visit` não exclui testes → corpos de teste entram na extração; (2)
  `ast/entity.rs` captura `@name` cru de um nó `type_identifier` dentro de região ERROR do tree-sitter,
  produzindo "nomes" de centenas de chars (ex.: `asserteq-hit-category-frameworkcategory-orm-...`).
- **Fix (agnóstico)**: (1) `is_test_path` (mesma SSOT do V1) filtra em `structural_extract::extract`;
  (2) `is_plausible_entity_name` (sem whitespace/newline; `[A-Za-z0-9_.:]`; `MAX_ENTITY_NAME_LEN=128`)
  no boundary de `extract_entities` (AST + floor). Nós stale são reapeados sozinhos no próximo `mustard-rt run scan`.

### V3 — guards.md afirma skill inexistente + agents com lixo — ALTO/MÉDIO
- **Causa**: `guards_seed.rs::build_body` emite "a generated skill documents it" para **todos** os clusters
  (core: 17), mas só os top-8 viram skill (cap). Seção "Frameworks detected" emite regras vácuas
  ("DO follow the framework conventions") e falsas (di/orm). Agents (`scan_orchestrate.rs`):
  description `(, general)` / `(…, ui)` (role vaza no slot de stack); `## Guards` = `(populated by /scan)`;
  `notes.md` em Mandatory Reads não existe; Recommended Skills lista 4 de 8.
- **Fix (SSOT)**: extrair `scan_skill_render::qualified_clusters(&[&Value]) -> Vec<&Value>`
  (MIN_CLUSTER_FILES + is_noise_label + sort fileCount + cap) consumida por **build_plans (skill),
  guards_seed (conventions) e o Recommended-Skills do agent**. Remover a seção frameworks do guards.
  Agent: stack/role em slots separados (omitir stack vazio); dropar bloco `## Guards` embutido (já há
  Mandatory-Read pro guards.md real); `notes.md` condicional à existência; Recommended Skills = conjunto
  completo gerado (via `qualified_clusters`).

### V4 — CLAUDE.md + wiring finalize (artefatos derivados meio-aplicados) — ALTO
- **Causa**: ver "Descoberta de fundo". `promote_description` só roda no render (gated por hash);
  `{sub}/CLAUDE.md` é create-only (3 stubs) e o `cli/CLAUDE.md` está podre (skills/docs inexistentes:
  `cli-subcommand-module`, `modules.md`…).
- **Fix**: `enrich_block::reapply_derived(path)` (compartilha `promote_description` com `write_preserving`);
  sub-step `reapplyDerived` no `scan_finalize` percorrendo `collect_generated_md` (SSOT do walk) e
  promovendo desc → frontmatter, **independente do hash**. CLAUDE.md: decisão de ownership (ver Decisões).

### V5 — Resolvedor empata todos os skills + naming sem sentido — ALTO/MÉDIO
- **Causa**: `skill_resolve::score_skill` — tag (`[add,refactor]` const), scope (`code-editing` const),
  entity (vazio = morto), applies (só separa subprojeto). O braço de descrição é *fallback* atrás de
  `if reasons.is_empty()` que nunca dispara → todos empatam em 3.0; desempate alfabético escolhe errado.
- **Fix (agnóstico)**: braço de **overlap de tokens da description** de primeira classe (sempre computado),
  `+0.5/token distinto, cap +1.5`, usando o **mesmo tokenizer** do intent (SSOT). Compõe acima de
  applies+scope sem dominar. Naming: adicionar `src`,`app`,`apps`,`pkg`,`packages` a `NOISE_LABELS`
  (`lib` já lá) → dropa `cli-src-pattern`, mantém `core-domain`/`ast`. `describe_cluster` sem gramática
  quebrada ("a new `commands`").

### V6 — Skill no padrão canônico (description = trigger) — ALTO
- **Causa**: `describe_cluster` produz stub meta ("Convention for the `X` cluster…"), não trigger; o
  `<!--desc-->` (trigger real da IA) fica **visível no corpo** mesmo após promovido ao frontmatter (duplica).
  Template manda editar `description:` direto (compete com a promoção).
- **Fix**: `describe_cluster` vira trigger "Use when …" determinístico (do label/folder/ext) — fallback útil
  pré-enrich. `promote_description` **remove** a linha `<!--desc-->` do corpo após promover (DRY; hash é do
  esqueleto, preservação intacta). Validator (`scan_md_validate`) checa trigger ("use when") warn-level.
  Remover o caminho de edição direta de `description:` no template (promoção é o único escritor).

### V7 — Prompt de enrich auto-contido (insumo embutido) + gap→instrução — ALTO
- **Causa**: ambos os builders de prompt (`render_enrich_prompt` string + `render_prompt`/template) mandam
  a IA **ler** os `Ref:` e o CLAUDE.md. O Rust **já lê** o conteúdo dos samples em `cluster_discovery`
  (`enrich_cluster`) e **descarta** o texto cru após extrair shape.
- **Fix (max-Rust)**: reter um snippet cru bounded por sample (`sampleSnippets`, reusando o floor de
  `extract_top_of_file_lines`); `build_enrich_insumo(root, sub, model)` monta um bundle (shape +
  snippets + glossário `model.e[].description` de `grain.model.json`) embutido no prompt; `detect_gaps(...)` emite linhas
  "GAP: {campo} não derivado — infira a partir dos snippets". Reescrever os prompts: embutir bundle,
  **remover** instruções de leitura, manter EVIDENCE RULE reapontada para "os snippets embutidos".
  Os dois caminhos consomem o **mesmo** `build_enrich_insumo`/`detect_gaps` (DRY). **Não** copiar o
  `build_tooling_block` (não-agnóstico — ver achado extra).

---

## Achado extra — vazamento de agnosticismo em `build_tooling_block`

`scan_precompute.rs:66-110` hardcoda `.net`/`csharp`/`c#`/`python`/`fastapi`/`django`/`package.json`/
`.csproj`/`pyproject.toml`/`pytest`/`ruff`/`dotnet`. Viola a invariante. Fix proposto: derivar o bloco de
tooling do `detect_commands` agnóstico (que já existe), não de matching de extensão/keyword por stack.

---

## Abstrações compartilhadas (SSOT a extrair)

| SSOT | Home | Consumidores |
|---|---|---|
| `is_test_path(rel)` | core (vocabulary/ast) | V1 (framework), V2 (entity) |
| `is_comment(line)` | core (promovido de structural_extract) | V1 (evidência), extrator estrutural |
| `is_vocabulary_definition(content)` | junto da tabela em frameworks.rs/architecture.rs | V1 |
| `is_plausible_entity_name(name)` | core ast/entity.rs | V2 |
| `qualified_clusters(&[&Value])` | scan_skill_render.rs | V3 (skill, guards, agent skills) |
| tokenizer único | skill_resolve.rs | V5 (intent + description) |
| `reapply_derived(path)` + `collect_generated_md` | enrich_block / scan_orchestrate | V4 (finalize) |
| `build_enrich_insumo` + `detect_gaps` | scan_orchestrate | V7 (ambos os prompts) |

---

## Ordem de execução (dependências) com gates de build/test

1. **CORE** (`packages/core`): `is_test_path`, `is_comment` (promoção), `is_vocabulary_definition`,
   `is_plausible_entity_name` + V2 entity guard. → `cargo test -p mustard-core`.
2. **RT-DETECT** (`architecture.rs`, `structural_extract.rs`, `cluster_discovery.rs`): V1 evidência,
   V2 skip-test, V7 `sampleSnippets`. Depende de (1). → `cargo test -p mustard-rt` (módulo scan).
3. **RT-SKILL** (`scan_skill_render.rs`, `skill_resolve.rs`, `guards_seed.rs`, `scan_md_validate.rs`):
   `qualified_clusters` (V3), V5 score+naming, V6 trigger+validator. → testes.
4. **RT-ORCH** (`scan_orchestrate.rs`, `enrich_block.rs`, `scan_finalize.rs`, template, `scan_precompute.rs`):
   V3 agents, V4 reapply+collect+CLAUDE.md, V6 promote-strip+prompt, V7 insumo+gaps, achado-extra tooling.
   Depende de (1,2,3). → testes + `cargo build`.
5. **Verificação E2E**: `mustard-rt run scan-orchestrate --force` + enrich + `scan-finalize` no próprio repo;
   confirmar `grain.model.json` sem di/orm, grafo sem nós-lixo, guards sem claims falsos, resolvedor desempatando,
   descriptions promovidas (skill-resolve diferenciando), prompt auto-contido.

## Decisões de produto (resolvidas)

- **D1 — ownership do `{sub}/CLAUDE.md` = Opção A**: Rust dono de `## Stack`/`## Commands` (regen
  idempotente, seção-bounded, sem tocar `## Guards`); **dropar** `## Scan References` + `## Recommended
  Skills` (duplicatas sem dono; agents leem `guards.md`/`stack.md`/`SKILL.md`). Corrigir o `cli/CLAUDE.md`
  podre como parte do regen.
- **D2 — `build_tooling_block` corrigido agora**: derivar o bloco de tooling do `detect_commands`
  agnóstico, removendo o branching hardcoded por stack.

## Correção de rumo — modelo canônico do /scan (decisão do mantenedor)

O conserto inicial de V1 (corroborar o keyword-scan com manifesto) foi **superado**. O modelo correto:

1. **Linguagem primeiro** (Rust, dos arquivos de config/extensões); **padrões/framework/arquitetura derivam por linguagem**.
2. **Identificação de framework/stack vem dos arquivos de configuração** (dependências declaradas em `Cargo.toml`/`package.json`/`.csproj`+settings C#/`go.mod`/`pyproject.toml`) — **nunca** de substring no código-fonte, **nunca** de conhecimento embutido no binário.
3. **Escada de conhecimento:** Rust determinístico → (rung futura) **buscar na web** o que o Rust não classifica, cachear → montar os `.md` → **IA só para lapidar** a prosa. O `grain.model.json` (produzido por `mustard-rt run scan`) é a **ponte de entrada** que identifica o projeto pelos arquivos de config.

**Por que o di/orm/framework era falso:** `detect_subproject_frameworks` varria o texto-fonte do próprio mustard (que contém exemplos de framework por ser software de detecção), e o já-removido `sync-detect` tinha tokens hardcoded (`actix/axum/…`). Os dois violavam o modelo.

### Fase 7 (em execução) — re-base por manifesto
- Framework no `grain.model.json` passa a sair das **dependências declaradas** no manifesto, classificadas por um vocab **externo** (`.claude/vocab/frameworks.toml`, schema `dep→categoria`), semeado como **dado em template** (não `include_str!` no binário) e copiado pelo init.
- Remove a varredura de texto-fonte (label de framework no modelo) + os tokens hardcoded do já-removido `sync-detect`.
- Dependência não classificada → **gap** (insumo para a rung web futura), nunca inventada.
- Finaliza o **reaper de nós órfãos** do `node_gen` (apaga artefatos cuja entidade sumiu do `grain.model.json`).
- Resultado-alvo: `_patterns.rust.frameworks` vazio para o mustard (Cargo.toml sem axum/diesel); detecção real preservada (projeto que declara a dep é detectado).

### Follow-ups registrados (não nesta fase)
- **Rung "web"** da escada: preencher gaps (dep/linguagem desconhecida) buscando na web e cacheando em `.claude/vocab/` — recomendação: agente de IA via WebSearch acionado pelo gap do Rust, resultado vira dado cacheado (regime permanente volta a ser Rust-determinístico).
- **Externalizar** também o vocab de **decorator** (usado pelo `structural_extract` para achar declarações decoradas) e o de **arquitetura** (`architecture_builtin.toml`) — hoje ainda `include_str!`.
- **Reestruturar "linguagem-primeiro"** no pipeline inteiro (padrões derivados explicitamente da linguagem detectada).
- Riqueza determinística do skill (descrição/trigger a partir de imports/base-class/declarações) para o resolvedor desempatar **sem** IA — a "Fase de skill útil" pendente.

Demais "open decisions" dos workflows resolvidas pelo princípio: desc-strip = B1 (remover do corpo);
validator = warn por default; `reapply_derived` roda sempre que houver bloco enriquecido com desc (não
só `--enrich`, pois é idempotente e cura o estado atual); `sampleSnippets` efêmeros no build do prompt
(sem inchar o `grain.model.json`); pesos do braço de descrição = +0.5/token cap +1.5.
