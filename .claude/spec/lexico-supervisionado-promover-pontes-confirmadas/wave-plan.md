# Plano de Waves

## Tabela de Waves

| Wave | Spec | Papel | Depende de | Resumo |
|------|------|-------|------------|--------|
| 1 | [[wave-1-rt]] | rt | — | Léxico supervisionado: evento feature.query + correlação determinística + comando lexicon-suggest com aceite humano (nunca auto-aplicar) |

## Critérios de Aceitação
- **AC-1** — Correlação determinística: duas consultas consecutivas do mesmo contexto (mesma spec/sessão) em que um termo falhou (tier none) e a re-consulta casou forte geram um evento lexicon.candidate com o par e a evidência. Command: `cargo test --workspace lexicon_correlation`
- **AC-2** — Revisão com aceite: o comando de listagem mostra candidatos com evidência; o aceite grava a entrada no .claude/lexicons/<par>.toml do projeto (nunca na seed embarcada) com ordenação determinística. Command: `cargo test --workspace lexicon_accept`
- **AC-3** — Nunca auto-aplica: sem aceite explícito, nenhum arquivo de léxico é alterado, mesmo com candidatos pendentes. Command: `cargo test --workspace lexicon_no_auto`
- **AC-4** — Workspace inteiro verde. Command: `cargo test --workspace`
