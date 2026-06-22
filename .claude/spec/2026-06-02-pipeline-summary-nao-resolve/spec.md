# Tactical Fix: gate consultivo `pipeline-summary` bloqueava o veredito do close-orchestrate

## Contexto

Tactical fix derivado de [[redesenhar-geracao-claude-md-hibrido]].

Durante o CLOSE da spec pai, o `close-orchestrate` reportou `overall: fail` por causa do gate `pipeline-summary`, enquanto verify/qa/review-spans/docs-stale passavam.

**Diagnóstico inicial (descartado):** "`pipeline-summary` não resolve `--spec-dir` relativo". FALSO — reproduzido depois: o comando funciona com caminho relativo (barra normal e invertida) e absoluto; `fs::read_to_string` é wrapper de path simples e o binário não troca de cwd. A falha do subprocesso foi **transitória** (provável janela de escrita atômica/rename de `spec.md` por um passo concorrente).

**Causa raiz real:** em `apps/rt/src/commands/pipeline/close_orchestrate.rs`, o gate `pipeline-summary` é comentado como "advisory — always passes" (passo 5, só renderiza o relatório Done/Falta/Próximos), MAS o veredito era `let overall_pass = gates.iter().all(|g| g.ok)` — incluindo o gate consultivo. Logo, qualquer falha (transitória ou não) do subprocesso de summary **derrubava o CLOSE inteiro**, contrariando a própria intenção documentada.

## Critérios de Aceitação

- [x] AC-1: o veredito do close exclui o gate consultivo `pipeline-summary` (falha dele não bloqueia; gate bloqueante que falha ainda bloqueia) — Command: `cargo test -p mustard-rt -- advisory_summary_failure_does_not_block_close`
- [x] AC-2: suite do close-orchestrate verde — Command: `cargo test -p mustard-rt -- close_orchestrate close_overall`

## Arquivos

- `apps/rt/src/commands/pipeline/close_orchestrate.rs` — novo helper `close_overall(gates)` que filtra o gate consultivo (`ADVISORY_GATE = "pipeline-summary"`) antes do `all(|g| g.ok)`; `overall_pass` passa a usá-lo. O gate ainda REPORTA seu `ok` real (visibilidade), mas não entra no veredito. Constante `ADVISORY_GATE` reusada no push do gate (fonte única). + teste de regressão `advisory_summary_failure_does_not_block_close`.

## Status

RESOLVIDO. `cargo test -p mustard-rt -- close_overall advisory_summary close_orchestrate` = 12 passed; clippy limpo no arquivo alterado. ⚠ Binário `mustard-rt` não reinstalado — precisa rebuild p/ o close-orchestrate em runtime parar de bloquear no summary.

<!-- wikilinks-footer-start -->
- [redesenhar-geracao-claude-md-hibrido](?) ⚠ unresolved
<!-- wikilinks-footer-end -->