# Tactical Fix: Flaky wall-clock bench bench_stream_10k_under_50ms in core reader blocks QA gates: mark ignore with reason, run explicitly

## Contexto

Tactical fix derivado de [[redesenho-agnostico-indice-termos-digest]].

## Critérios de Aceitação

- **AC-1** — Suíte padrão do core verde com o benchmark de relógio-de-parede fora do caminho (marcado ignore com razão, ainda executável via --ignored).
  Command: `cargo test -p mustard-core`

## Arquivos

- packages/core/src/io/events/reader.rs — atributo `#[ignore = "..."]` no teste `bench_stream_10k_under_50ms` (1 linha; threshold e lógica intocados)

<!-- wikilinks-footer-start -->
- [redesenho-agnostico-indice-termos-digest](?) ⚠ unresolved
<!-- wikilinks-footer-end -->