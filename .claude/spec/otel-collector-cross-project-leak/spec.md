# otel-collector-cross-project-leak

## Contexto

Follow-up arquitetural da spec `otel-economy-summary-bridge`. O collector OTEL é um processo **global**: quando vários projetos emitem OTLP, um sobrevivente captura tudo e grava sob o **seu próprio** projeto. Prova empírica: ~224 KB de tokens da sessão `fda6d733` (rodada em `c:/atiz/sialia`) foram gravados em `mustard/.claude/.session/otel-unattached/`, e só ~61 KB em `sialia/.claude/.session/otel-unattached/`.

A spec-mãe já consertou o caso **in-project** (roteia por `payload.session_id` quando o dir da sessão existe no projeto do collector) e mantém sessão estrangeira em `otel-unattached` em vez de arquivar errado. Falta eliminar o vazamento **cross-projeto**: um datapoint cujo `session_id` pertence a OUTRO projeto deve ir para o `.claude` daquele projeto, não para o do collector.

## Checklist

- [x] Mecanismo escolhido: lookup no transcript-store do Claude Code (`~/.claude/projects/<enc>/<session_id>.jsonl`, campo `cwd` lossless) — reúso de `home_dir()`/`ClaudePaths`
- [x] Resolução implementada no caminho de escrita de métrica do collector (`metric_target`/`foreign_project_for_session`)
- [x] Teste de roteamento cross-projeto

## Critérios de Aceitação

- [x] AC-1: uma métrica cujo `session_id` resolve para um projeto diferente do collector é gravada sob o `.claude/.session/<id>/.events/` daquele projeto. Command: `cargo test -p mustard-rt otel_metric_routed_cross_project`
- [x] AC-2: sem resolução possível, permanece em `otel-unattached` (sem regressão do caso in-project). Command: `cargo test -p mustard-rt otel_metric_unresolvable_stays_unattached`

## Causa raiz

OTLP de métrica não carrega cwd/projeto; o collector resolve o root pelo próprio ambiente. Não existe hoje um mapa `session_id`→projeto (`.harness/sessions/` está vazio/legado).

## Plano

A decidir na fase PLAN — comparar: (a) registro `session_id`→project_root escrito pelo hook `SessionStart` de cada projeto, lido pelo collector; (b) collector varre roots conhecidos procurando `.claude/.session/<id>/`; (c) um collector por projeto em porta dedicada. Avaliar custo, corrida entre processos e agnosticismo.

## Limites

- Não altera o que o dashboard lê do próprio SQLite.
- Atribuição por fase continua fora (OTEL sem dimensão de fase) — TF separada.
- Migração dos eventos JÁ vazados em `mustard/.claude/.session/otel-unattached/` (dados históricos misatribuídos) é cleanup one-off, fora do escopo deste fix de roteamento; o roteamento corrigido não reproduz o vazamento.