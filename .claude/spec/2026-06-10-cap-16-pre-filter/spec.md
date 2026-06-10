# Tactical Fix: Cap-16 pre-filter in domain_terms + lexicon seed entries vencido hierarquia + per-project lexicon overlay without rebuild

## Contexto

Tactical fix derivado de [[redesenho-agnostico-indice-termos-digest]].

## Critérios de Aceitação

- **AC-1** — Crate scan verde: seed com vencido/hierarquia, overlay de léxico por projeto (.claude/lexicons/<par>.toml estende a seed sem rebuild, determinístico, arquivo ausente/inválido degrada para a seed) e testes cobrindo os três comportamentos.
  Command: `cargo test -p scan`
- **AC-2** — Workspace verde (inclui o teste do cap ampliado do domain_terms no rt).
  Command: `cargo test --workspace`

## Arquivos

- apps/rt/src/commands/feature.rs — cap do domain_terms 16→32 com justificativa (palavras-função PT comiam as vagas; o digest já filtra stopwords e reporta por termo) + teste
- apps/scan/lexicons/pt-en.toml — princípio "não sei onde o mustard vai rodar": seed fica SÓ com equivalências genéricas de negócio; ENTRA vencido=["overdue","expired"] e hierarquia=["hierarchy","parent"] (genéricos); SAI titulo=["receivable"] (jargão de domínio fintech — num CMS título=title; vai para o léxico do projeto consumidor)
- apps/scan/src/stemmers.rs + apps/scan/src/matching.rs (+ digest.rs se necessário) — overlay por projeto: merge de <root do grain>/.claude/lexicons/<a>-<b>.toml sobre a seed (entradas do projeto VENCEM a seed; ordem determinística; ausente/inválido degrada para a seed; root vem do modelo, não do cwd)
- apps/cli/templates/.claude/lexicons/pt-en.toml — template com [terms] vazio e exemplos comentados (o init semeia a capacidade em projetos novos; merge preserva os existentes)
- apps/scan/tests/match_tiers.rs — testes: overlay vence a seed, ausência degrada, entradas novas casam

<!-- wikilinks-footer-start -->
- [redesenho-agnostico-indice-termos-digest](?) ⚠ unresolved
<!-- wikilinks-footer-end -->