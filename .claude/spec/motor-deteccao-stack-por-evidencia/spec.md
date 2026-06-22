# Motor de deteccao de stack por evidencia multi-sinal agnostico e data-driven

<!-- drafter:tone=didactic — Write this spec narrative in didactic tone — expand abbreviations on first use (AC = Acceptance Criteria, wave = onda) and prefer plain words over jargon. -->

<!-- PRD -->

## Contexto

Implementa o motor descrito em **`docs/DETECCAO-DE-STACK-MULTISSINAL.md`** (leia-o antes de planejar/executar): identificar frameworks/stacks (Laravel, Django, Rails, Next…) por **evidência multi-sinal**, de forma agnóstica e dirigida por dados.

Hoje há dois sinais fragmentados: dependências do manifesto (`apps/scan/src/ingest.rs:146` `infer_frameworks` → `frameworks: Vec<String>`, fiado) e assinaturas de código (`packages/core/src/domain/vocabulary/frameworks.rs:268` `detect_framework_signals`, **órfão** — sem caller de produção). Falta o sinal de arquivo/diretório marcador e, sobretudo, uma camada que **nomeie a stack por convergência de sinais**.

Insight: linguagem se detecta por gramática (já correto); **framework/stack se detecta por sinais, nunca por gramática**. O motor deve ser cego ao nome da stack — toda especificidade vive em um registro de dados.

Por que agora: o suporte a PHP/Laravel (spec `adicionar-suporte-php-laravel-ao`) entregou a fundação agnóstica (linguagem + manifesto), mas expôs que a detecção de framework não tem um mecanismo genérico fiado. Este motor resolve isso para qualquer stack.

## Usuários/Stakeholders

O próprio Mustard e quem escaneia projetos com framework: o digest, os Guards e o dashboard passam a exibir a stack **nomeada** (com confiança e os sinais que a sustentaram), não só a lista de dependências crua.

## Métrica de sucesso

Escanear um projeto produz `detected_stacks` com a stack **nomeada** + confiança por convergência + os sinais que a sustentaram — para qualquer stack do registro, sem citar nenhuma por nome em `src/`. Adicionar uma stack nova é editar **um dado** (`stacks.toml`), nunca código. Consumidores atuais de `frameworks` seguem funcionando (evolução serde aditiva).

## Não-Objetivos

- Detecção de versão exata do framework.
- IA/heurística estatística — o motor é puramente declarativo e determinístico.
- Substituir a detecção de linguagem por gramática (já está correta).
- Esgotar todas as stacks de uma vez — o registro cresce por dado, sob demanda.
- Tocar a fundação já entregue da spec PHP/Laravel (gramática + composer).

## Critérios de Aceitação

- **AC-1** — Build e testes verdes nas crates afetadas após a mudança
  Command: `cargo test -p mustard-core`
- **AC-2** — Registro declarativo de stack: `stacks.toml` com schema multi-sinal (`manifest_deps`/`path_markers`/`code_signatures`, opcional `language`) e ≥2 stacks de validação de linguagens distintas
  Command: `cargo test -p mustard-core stacks_registry_parses`
- **AC-3** — Motor genérico por convergência: `infer_stacks` funde os três tipos de sinal e gradua a confiança pelo nº de tipos que bateram (baixa vs alta provada em teste)
  Command: `cargo test -p mustard-core infer_stacks`
- **AC-4** — Evolução serde aditiva: `detected_stacks` com `#[serde(default)]`; payload antigo sem o campo desserializa e `frameworks` permanece
  Command: `cargo test -p mustard-core detected_stacks_serde_compat`
- **AC-5** — Fiado no pipeline e2e: escanear a fixture Laravel produz `detected_stacks` com `name=laravel` e os sinais que a sustentaram (modelo, por-unit e digest)
  Command: `cargo test -p scan stack_detection_e2e`
- **AC-6** — Invariante agnóstica (anti-hardcode): nenhum literal de stack (`laravel|django|illuminate|artisan`) em código executável dos `.rs` de produção tocados; literais só em `.toml`/fixtures/testes/doc-comments
  Command: `cargo test -p scan`

<!-- PLAN -->

## Arquivos

Panorâmico (detalhe por onda nas sub-specs). Ver `docs/DETECCAO-DE-STACK-MULTISSINAL.md` §4.

**Tipo `StackDetection` + campo `detected_stacks` (aditivo, `#[serde(default)]`, não remove `frameworks`):**
- `apps/scan/src/model.rs` — modelo nativo do scan: ProjectModel (`:16`) e ProjectUnit (`:50`) carregam `frameworks: Vec<String>`; adicionar ali `detected_stacks`.
- `apps/scan/src/ingest.rs:20` (struct intermediária) e `apps/scan/src/main.rs:250` (montagem `frameworks: ing.frameworks`) — propagar o novo campo.
- `apps/scan/src/digest.rs:34` (CapabilityDigest) — reexpor `detected_stacks`.
- `packages/core/src/domain/scan.rs:128` (`struct Project`, `frameworks` em `:139`) — o tipo projetado pelo core; reexpor `detected_stacks` se o digest/facts do core o expõe.

**Registro + motor (genéricos, cegos ao nome):**
- `packages/core/src/domain/vocabulary/stacks.toml` — **novo** registro declarativo (stacks nomeadas × sinais).
- `packages/core/src/domain/vocabulary/stacks.rs` — **novo** motor `infer_stacks`: reusa `detect_framework_signals`/`KeyedAutomaton` (Aho-Corasick) para `code_signatures`, casa `manifest_deps` e `path_markers` genericamente, scoring por convergência. (O tipo `StackDetection` pode ser definido aqui em `mustard-core` e reusado por `apps/scan`, já que o scan depende do core.)

**Fiação + consumo:**
- `apps/scan/src/ingest.rs:146` (ao lado de `infer_frameworks`) e `apps/scan/src/facts.rs` — chamar o motor com deps + walk + conteúdo já disponíveis e alimentar `detected_stacks`.
- `apps/scan/tests/` + fixtures multi-stack (Laravel + uma segunda stack) — testes e2e + compat serde + gate anti-hardcode.

## Dependências

Ondas majoritariamente lineares (cada camada precisa da anterior): **Onda 1 (modelo + registro)** → **Onda 2 (motor no core)** → **Onda 3 (fiação no scan)** → **Onda 4 (consumo + testes e2e + gate)**.

## Limites

IN: tipo `StackDetection`, registro `stacks.toml`, motor genérico `infer_stacks` (incl. path-marker matching), fiação no scan, propagação ao digest, testes e fixtures.
OUT: lógica que cite nome de stack em `src/`; detecção de versão; IA; troca da detecção de linguagem; a fundação PHP/Laravel já entregue.

## Concerns

- **CONCERN (Onda 3, premissa falsa corrigida):** o plano assumia que `apps/scan` depende de `mustard-core` — verificado: **não depende** (era a causa-raiz de `detect_framework_signals` ser órfão: vocabulário no core, miner no scan, deslinkados). Decisão: adicionar `mustard-core` como dependência de `apps/scan` (mesma direção apps→packages de `rt`/`cli`; sem ciclo cargo — o core só localiza o binário `scan` em runtime; pins de `tree-sitter 0.26` já coordenados). Alternativa rejeitada: duplicar o motor no scan (viola dedup). Trade-off: binário do scan cresce (core puxa tiktoken-rs/rayon).