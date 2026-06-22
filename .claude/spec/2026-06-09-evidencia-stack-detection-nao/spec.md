# Tactical Fix: Evidencia de stack detection nao desconta fixtures e diretorios de teste

## Contexto

Tactical fix derivado de [[robustez-agnostica-descoberta-padroes-grafo]]. Medição pós-spec: o scan do PRÓPRIO repo mustard detecta `laravel(0.95)` e `django(0.65)` no nível do repositório — toda a evidência vem de `apps/scan/tests/fixtures/{php_laravel,python_django}` (sinais observados: `dep:laravel/framework` do composer.json DA FIXTURE, `path:routes/web.php`, `code:Illuminate\...`). Enganoso: o mustard não é um app Laravel; fixtures/testes não são evidência da stack do projeto.

Fix data-driven (espelha o precedente do `stopwords.toml` desta mesma linhagem): uma lista de SEGMENTOS de diretório convencionais de teste/fixture vive como DADO (ex.: `tests`, `test`, `__tests__`, `spec`, `specs`, `fixtures`, `testdata`, `__mocks__`) e a coleta de evidência para `infer_stacks` (repo-level em `ingest.rs` e por-unit em `main.rs`) filtra paths/contents/deps cujo caminho contenha um desses segmentos como componente. As TRÊS classes de sinal são filtradas (a dep veio do manifesto da fixture). A mineração de convenções/papéis NÃO muda (testes continuam visíveis ao miner — o filtro é só na evidência de stack). Nenhum nome de diretório hardcoded em `src/`.

## Critérios de Aceitação

- **AC-1** — Scan de uma fixture-mãe que contém um projeto real + uma subpasta de teste com outra stack: só a stack do projeto real é detectada no nível do repo (a fixture aninhada não vaza)
  Command: `cargo test -p scan stack_evidence_excludes`
- **AC-2** — As fixtures existentes continuam detectando suas stacks quando escaneadas DIRETAMENTE (a raiz do scan estando dentro da árvore de teste não suprime — o filtro é relativo à raiz escaneada)
  Command: `cargo test -p scan stack_detection_e2e`
- **AC-3** — Suítes completas verdes (scan + core)
  Command: `cargo test -p scan`

## Arquivos

- `apps/scan/src/ingest.rs` — filtrar paths/contents/deps de evidência repo-level por segmento de teste (relativo à raiz escaneada)
- `apps/scan/src/main.rs` — mesmo filtro na evidência por-unit (`infer_unit_stacks`)
- arquivo de dado novo com os segmentos (ex.: `apps/scan/test-dirs.toml` ou seção no arquivo de dado mais coerente — decidir espelhando `stopwords.toml`)
- `apps/scan/tests/` — teste novo `stack_evidence_excludes_*` com fixture-mãe

<!-- wikilinks-footer-start -->
- [robustez-agnostica-descoberta-padroes-grafo](?) ⚠ unresolved
<!-- wikilinks-footer-end -->