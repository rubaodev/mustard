# Plano de Waves

## Tabela de Waves

| Wave | Spec | Papel | Depende de | Resumo |
|------|------|-------|------------|--------|
| 1 | [[wave-1-impl]] | impl | — | Camada scan (apps/scan): linguagem PHP + manifesto composer, 100% por dado |
| 2 | [[wave-2-impl]] | impl | — | Camada core (packages/core): sinais do Laravel no vocabulario de frameworks |
| 3 | [[wave-3-impl]] | impl | [[wave-1-impl]], [[wave-2-impl]] | Fixtures de projeto Laravel minimo + teste e2e de scan + gate anti-hardcode (AC-6) |

## Critérios de Aceitação
- AC-1 — O scan compila com a gramatica PHP (crate tree-sitter-php compativel com tree-sitter 0.26): `cargo build -p scan`
- AC-2 — Extracao de PHP funciona: um teste parseia um .php com namespace/use/class/funcao e o motor generico extrai os simbolos sem no de gramatica em src/: `cargo test -p scan php_extraction`
- AC-3 — composer.json reconhecido como build-system com deps de require/require-dev e scripts: `cargo test -p scan composer_manifest`
- AC-4 — Laravel detectado como framework: uma amostra contendo `Illuminate\` (e afins) e rotulada Laravel pelo vocabulario via detect_framework_signals: `cargo test -p mustard-core laravel`
- AC-5 — Scan e2e de uma fixture Laravel produz um modelo com linguagem php + manifesto composer + framework Laravel: `cargo test -p scan php_laravel_fixture`
- AC-6 — Invariante agnostica: nenhum literal php/laravel/composer/artisan/illuminate (case-insensitive) em apps/scan/src nem packages/core/src; o grep retorna zero: `! rg -i "php|laravel|composer|artisan|illuminate" apps/scan/src packages/core/src`
