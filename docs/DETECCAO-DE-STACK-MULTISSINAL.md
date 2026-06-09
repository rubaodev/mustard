# DETECÇÃO DE STACK POR EVIDÊNCIA MULTI-SINAL

> Documento de arquitetura. Define como o Mustard deve identificar **frameworks/stacks**
> (Laravel, Django, Rails, Spring, Next…) de forma **agnóstica, dirigida por dados e SOLID**.
> Status: **proposta** (planejamento — não implementado). Gatilho: suporte a PHP/Laravel
> (spec `adicionar-suporte-php-laravel-ao`).

## 1. Motivação e o insight central

Adicionar "suporte a PHP/Laravel" expôs uma confusão conceitual que vale para **qualquer** stack:

- **Linguagem ↔ gramática.** PHP, Python, Go são detectados por *gramática* (tree-sitter),
  porque linguagem é **estrutura sintática**. O Mustard já faz isso da forma ideal:
  `apps/scan/languages.toml` registra extensão → gramática, e o motor é cego ao nome. Adicionar
  linguagem = um dado. **Nada a mudar aqui.**
- **Framework/stack ↔ assinatura/evidência — NUNCA gramática.** Laravel não tem gramática própria:
  é uma biblioteca + convenções *dentro* do PHP. A gramática enxerga `class`/`use`/`function`;
  ela não tem como saber que `Illuminate\` "é Laravel". Framework só se identifica por **sinais**:
  o que foi importado, qual dependência foi declarada, quais arquivos-marcadores existem.

Portanto a pergunta "detectar Laravel via gramática?" não tem resposta — framework é sempre
**inferência por evidência**. Este documento define o motor que faz essa inferência de forma
genérica, para qualquer stack, sem citar nenhuma por nome dentro de `src/`.

## 2. Princípios (invariantes)

1. **Data-driven / Open-Closed.** Adicionar uma stack nova = adicionar **dados** num registro
   declarativo. Nunca lógica nova. O motor em `src/` jamais cita um nome de framework/stack.
2. **Motor cego.** A lógica apenas *itera* o registro e aplica regras genéricas de casamento.
   Mesma filosofia do `languages.toml` (linguagem) e `manifests.toml` (build-system).
3. **Determinismo.** Sem IA, sem rede, sem heurística não reprodutível. Ordenação e desempate
   estáveis (a invariante do miner em `apps/scan/CLAUDE.md`).
4. **Evidência convergente, não regra única.** Confiança cresce quando vários sinais independentes
   apontam para a mesma stack. Um sinal isolado é fraco; convergência é forte.
5. **Compatibilidade serde.** O modelo (`domain/model/`) é contrato público (rt, dashboard
   renderizam sobre ele). A evolução é **aditiva** — nunca quebra o shape existente.

## 3. Estado atual (verificado)

Existem hoje **dois sinais**, fragmentados, e um deles está **desligado**:

### Sinal 1 — dependências do manifesto (FIADO, funciona)
- `apps/scan/src/ingest.rs:146` → `infer_frameworks(&manifests)` chama
  `facts::rank_by_frequency(deps)` e devolve as dependências **rankeadas por frequência** (top 12),
  verbatim do manifesto, sem filtrar por nome conhecido — agnóstico.
- `apps/scan/src/facts.rs:64` popula `project.frameworks` com essa lista em `enrich_projects()`.
- `apps/scan/manifests.toml` + `apps/scan/src/manifests.rs` (`json_deps`, etc.): leitor genérico,
  6 formatos; deps extraídas por **seletor de dados**, não por campo hardcoded.
- **Efeito:** num projeto Laravel, `laravel/framework` (declarado no `composer.json`) aparece em
  `frameworks`. É assim que "Laravel" surge hoje — 100% dado, zero código específico.

### Sinal 2 — assinaturas de código (EXISTE, mas ÓRFÃO)
- `packages/core/src/domain/vocabulary/frameworks.rs:268` → `detect_framework_signals(content)`
  e `:289` `detect_framework_signals_with(root, content)` (honra override do usuário em
  `.claude/vocab/frameworks.toml`). Retornam `Vec<FrameworkHit>{ pattern, category, start, end, line }`.
- Automaton Aho-Corasick único (`KeyedAutomaton::from_groups`), construído de
  `frameworks_builtin.toml`.
- **Schema atual do registro** — `[[signal]]` com `category ∈ {orm, framework, di}` + `patterns`
  (substrings literais). **É ANÔNIMO:** detecta *"há sinal de framework/ORM/DI"*, **não** nomeia
  qual stack. Ex.: `pgTable(`→orm, `axum::`→framework, `@Injectable`→di.
- **Confirmado:** `detect_framework_signals*` **não tem caller de produção** (só `#[cfg(test)]`).
  Os sinais de Laravel adicionados na onda 2 da spec atual vivem aqui — corretos no lugar, mas
  hoje **não chegam a nenhum output** que o usuário veja.

### Sinal 3 — arquivos/diretórios marcadores (NÃO EXISTE)
- Não há conceito de "path marker" (`artisan`, `manage.py`, `next.config.js`, `app/Models/`).
  O único "marcador" hoje é a presença do próprio manifesto.

### Shape do modelo (localização verificada)
- O **modelo nativo do scan** vive em `apps/scan/src/model.rs`: `frameworks: Vec<String>` em
  ProjectModel (`:16`, repo-wide) e ProjectUnit (`:50`, por unit); montado em
  `apps/scan/src/main.rs:250` a partir da struct intermediária `apps/scan/src/ingest.rs:20`.
- Reexposto no digest do scan em `apps/scan/src/digest.rs:34` (CapabilityDigest).
- O **tipo projetado pelo core** é `packages/core/src/domain/scan.rs:128` (`struct Project`,
  `frameworks` em `:139`) — usado por `digest`/`facts` do `mustard-core`.
- Tudo isso é **contrato público serde** (rt/dashboard renderizam) ⇒ trocar `Vec<String>` por
  `Vec<FrameworkHit>` quebraria o shape. Evolução tem de ser **aditiva**.

## 4. Arquitetura proposta

Um **motor de evidência multi-sinal**, em duas camadas: *sinais* de baixo nível (genéricos,
sem nome) e *stacks* de alto nível (registro declarativo que dá nome por convergência de sinais).

### 4.1 Os três tipos de sinal (todos dado)
| Sinal | Fonte | Custo | Força | Estado |
|-------|-------|-------|-------|--------|
| `manifest_dep` | nome de dependência no manifesto (`laravel/framework`, `django`, `next`) | barato | forte (declarado) | já existe |
| `path_marker` | arquivo/dir característico (`artisan`, `manage.py`, `next.config.*`, `app/Models/`) | barato | médio | a criar |
| `code_signature` | substring/símbolo no conteúdo (`Illuminate\`, `from django`, `Route::`) via Aho-Corasick | médio | médio (uso real) | existe, órfão |

### 4.2 Registro declarativo de stack (novo, dado)
Um registro — `stacks.toml` — onde cada stack **nomeada** lista suas evidências. O motor é cego:
só itera as entries. Exemplo (ilustrativo):

```toml
[[stack]]
name = "laravel"            # rótulo emitido; o motor não conhece "laravel", só copia
language = "php"            # opcional: restringe o casamento à linguagem certa
manifest_deps = ["laravel/framework"]
path_markers  = ["artisan", "app/Http/Controllers/"]
code_signatures = ["Illuminate\\", "Route::"]   # reusa o matcher Aho-Corasick

[[stack]]
name = "django"
language = "python"
manifest_deps = ["django", "Django"]
path_markers  = ["manage.py", "wsgi.py"]
code_signatures = ["from django", "django.db"]
```

> **Decisão a refinar no spec:** estender `frameworks_builtin.toml` (mantendo o sinal anônimo
> category+patterns como camada de baixo nível) **ou** introduzir `stacks.toml` separado. Recomendação:
> registro novo `stacks.toml` para a camada nomeada, reusando o matcher Aho-Corasick existente para
> `code_signatures`. O `frameworks_builtin.toml` anônimo continua útil para Guards ("tem ORM/DI?").

### 4.3 Motor genérico + scoring
- Entrada: manifestos (deps já parseadas), a lista de arquivos do walk (paths), e o conteúdo
  (já lido em `ingest.rs:60-138`).
- Para cada `[[stack]]` do registro, conta **quantos tipos de sinal** bateram. Confiança por
  convergência: 1 tipo = baixa; 2 = média; 3 = alta. Determinístico, sem pesos mágicos opacos
  (os limiares ficam explícitos e versionados).
- Saída: lista de stacks detectadas com `{ name, confidence, signals: [...] }` — **explicável**
  (cada detecção carrega quais sinais a sustentaram).

### 4.4 Ponto de fiação
`apps/scan/src/ingest.rs:146` (`infer_frameworks`) é o lugar natural: ali as deps já são rankeadas
e o walk/conteúdo estão disponíveis no mesmo módulo. Um `infer_stacks(manifests, source_files, walk)`
roda ao lado e alimenta o novo campo do modelo.

### 4.5 Evolução do shape (aditiva, serde-safe)
- **Manter** `frameworks: Vec<String>` (deps rankeadas) — retrocompatível, consumidores atuais
  intactos.
- **Adicionar** campo novo opcional, ex. `detected_stacks: Vec<StackDetection>` em ProjectModel/
  ProjectUnit/CapabilityDigest, com `#[serde(default)]` para não quebrar payloads antigos.
- Dashboard e digest passam a poder mostrar a stack nomeada + confiança + sinais; quem só lê
  `frameworks` continua funcionando.

## 5. Decisões de design

1. **Aditivo, nunca destrutivo** no modelo serde (guard de `domain/model/`).
2. **Override do usuário preservado:** o caminho `.claude/vocab/` (override-aware) se estende ao
   `stacks.toml` — o projeto pode declarar stacks internas.
3. **Fail-safe, não fail-open:** ausência de registro/erro de parse degrada para "sem stacks
   detectadas" sem panic e sem inventar — coerente com a tolerância a falha do miner. (Evitar a
   armadilha do vocabulário de regressão: degradar ≠ fingir sucesso.)
4. **Sem nomes em `src/`:** todo nome de stack/sinal vive em `.toml`. AC anti-hardcode por `git diff`.
5. **Determinismo:** ordenação por (confiança DESC, primeira-ocorrência ASC); desempate estável.

## 6. Plano de implementação (esboço de ondas)

1. **Modelo + registro (dado):** adicionar `StackDetection` + `detected_stacks` (aditivo, serde-default);
   criar `stacks.toml` com o schema e 1–2 stacks de validação (Laravel + uma de outra linguagem).
2. **Motor genérico (core):** `infer_stacks` reusando deps + walk + `detect_framework_signals` para
   `code_signatures`; introduzir `path_marker` matching genérico; scoring por convergência.
3. **Fiação (scan):** ligar `infer_stacks` em `ingest`/`facts`; propagar `detected_stacks` ao digest.
4. **Consumo + testes:** dashboard/digest exibem a stack nomeada; fixtures multi-stack (Laravel +
   Django/Next) provando detecção por convergência e o gate anti-hardcode em `src/`.

## 7. Não-objetivos

- Detecção de versão exata do framework.
- IA/heurística estatística — o motor é puramente declarativo.
- Substituir a detecção de linguagem (gramática) — ela já está correta.
- Detectar todas as stacks do mundo de uma vez — o registro cresce por dado, sob demanda.

## 8. Relação com a spec `adicionar-suporte-php-laravel-ao`

O que já está **verde e agnóstico** permanece e é independente deste motor:
- PHP como **linguagem** (gramática `tree-sitter-php` + queries `.scm`) — forma ideal, entregue.
- `composer.json` como **build-system** (`manifests.toml`) — entregue.
- `laravel/framework` aparecendo via deps do manifesto — entregue.

O que **migra para este documento/spec**: a detecção de Laravel por **assinatura de código**
(os sinais `Illuminate\`/`Route::`/`artisan` da onda 2). Eles ficam preservados em
`frameworks_builtin.toml` como insumo: serão consumidos quando o motor multi-sinal for fiado —
deixando de ser código órfão e passando a ser o primeiro caso de validação do motor.
