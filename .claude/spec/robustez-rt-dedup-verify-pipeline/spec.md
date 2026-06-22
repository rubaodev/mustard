# Robustez do rt: dedup do verify-pipeline por workspace cargo e fallback de session_id no run-face

<!-- drafter:tone=didactic -->

## Contexto

Dois defeitos do crate `apps/rt` surgiram durante o conserto de atribuição de sessão dos hooks (spec [[hooks-posttooluse-descartam-session-id]]). São independentes entre si, mas ambos de robustez de telemetria/verificação e ambos no `rt` — tratados juntos por economia.

1. **`verify-pipeline` redundante em monorepo.** `discover_via_grain` (`apps/rt/src/commands/pipeline/verify_pipeline.rs:98`) cria um alvo de verificação para **cada** projeto do `grain.model.json` e roda neles o **mesmo** comando global do `mustard.json` (`cargo build` / `cargo test`). Como quase todos os projetos (`apps/rt`, `packages/core`, `apps/cli`, `apps/scan`, `apps/mcp`, raiz, e até os dirs Python/React) resolvem para o **mesmo workspace cargo**, isso vira ~8 builds redundantes do workspace inteiro, sequenciais — cada um com timeout de 600s. Resultado observado: `close-orchestrate` gastou ~32 min no portão `verify-pipeline` e ainda deu `fail`.

2. **Emissores *run-face* atribuem a `unknown`.** `context::session_id()` (`apps/rt/src/shared/context.rs:88-94`) só lê as variáveis de ambiente `MUSTARD_SESSION_ID`/`CLAUDE_SESSION_ID`, ausentes nos processos `mustard-rt run …`. Comprovado ao vivo: o evento `pipeline.economy.operation.invoked` caiu em `.session/unknown/` mesmo com o conserto dos hooks já valendo. O hook de `SessionStart` já cria `.claude/.session/<id>/` com o `session.start`, então há fonte para um fallback.

## Usuários/Stakeholders

Quem fecha pipelines (o portão `verify-pipeline` deixa de gastar ~32 min e de falso-falhar) e quem lê métricas por-sessão do painel (os eventos run-face deixam de vazar para `unknown`).

## Métrica de sucesso

`verify-pipeline` roda o workspace cargo **uma vez** (não N) e fica verde; eventos run-face emitidos durante uma sessão carregam o `session_id` real em vez de `unknown`.

## Não-Objetivos

- Não redesenhar o `verify-pipeline` para comandos por-stack (Python/React continuam sem verificação própria — gap separado).
- Não tocar o lado da leitura do painel nem o conserto já feito nos hooks PostToolUse.
- Não verificar o `apps/dashboard/src-tauri` (fora do workspace) aqui — é escopo da spec do painel.

## Critérios de Aceitação

- **AC-1** — dedup por workspace: um teste de unidade prova que alvos do `verify-pipeline` que compartilham o mesmo workspace cargo colapsam para um só.
  Command: `cargo test -p mustard-rt verify_pipeline_dedups_shared_cargo_workspace`
- **AC-2** — fallback run-face: um teste de unidade prova que a resolução de `session_id` cai no dir `.session/<id>/` não-`unknown` mais recente quando o ambiente não traz o id.
  Command: `cargo test -p mustard-rt session_id_falls_back_to_newest_session_dir`
- **AC-3** — a suíte do crate fica verde.
  Command: `cargo test -p mustard-rt`
- **AC-4** — sem aviso novo de lint.
  Command: `cargo clippy -p mustard-rt --all-targets`

## Causa raiz

- **Fix 1:** o comando de verificação é global (um só par `build`/`test` do `mustard.json`), mas é aplicado por-projeto. Em um único workspace cargo, `cargo build`/`cargo test` é idempotente em qualquer dir-membro — rodar N vezes é puro desperdício e multiplica o risco de timeout.
- **Fix 2:** `session_id()` não tem fallback para a fonte que já existe no disco (o dir da sessão criado pelo `SessionStart`), espelhando o que `current_spec()` já faz com o pipeline-state mais recente por mtime.

## Plano

1. **Dedup do `verify-pipeline`** (`verify_pipeline.rs`): após `discover_targets`, deduplicar alvos cujo comando é `cargo` pelo **workspace cargo raiz** que resolvem. Resolver via caminhada de ancestrais buscando o primeiro `Cargo.toml` que contém `[workspace]` (`fn cargo_workspace_root(start) -> Option<PathBuf>`, puro filesystem, sem spawnar cargo). Chave de dedup = `workspace_root` quando o comando é cargo, senão o `cwd`. Mantém o primeiro alvo por chave. Fail-open: sem `[workspace]` encontrado → chave = `cwd` (não funde dirs distintos).
2. **Fallback de `session_id`** (`context.rs`): após as leituras de env e antes de cair em `"unknown"`, resolver o dir `.claude/.session/<id>/` não-`unknown` mais recente por mtime e devolver seu nome. Extrair `fn newest_session_dir(session_dir: &Path) -> Option<String>` testável (o `unsafe` é proibido no crate, então o teste exercita o helper sem mexer em env, como `route.rs` já faz). Fail-open a `"unknown"`.
3. Testes de regressão `verify_pipeline_dedups_shared_cargo_workspace` (AC-1) e `session_id_falls_back_to_newest_session_dir` (AC-2).

Depois do verde: rebuild do binário (`cargo install --path apps/rt --force`).

## Limites

- Mudanças isoladas em dois arquivos do `rt`; sem mudar contratos públicos nem o formato de evento.
- A dedup é conservadora: só funde alvos cargo que comprovadamente compartilham o mesmo `[workspace]`; qualquer dúvida → não funde.
- O fallback de `session_id` é heurístico (sessão mais recente vence), igual ao `current_spec()` — aceitável para telemetria, não load-bearing.

## Checklist

- [x] `verify_pipeline.rs`: `cargo_workspace_root` (caminhada de ancestrais por `[workspace]`) + dedup dos alvos cargo por workspace raiz
- [x] `context.rs`: `newest_session_dir` + wiring no `session_id()` após o env, antes do `"unknown"`
- [ ] Testes `verify_pipeline_dedups_shared_cargo_workspace` (AC-1) e `session_id_falls_back_to_newest_session_dir` (AC-2)
- [ ] Suíte `cargo test -p mustard-rt` verde + clippy limpo

<!-- signals: apps/rt,verify-pipeline,session-attribution,run-face -->

<!-- wikilinks-footer-start -->
- [hooks-posttooluse-descartam-session-id](?) ⚠ unresolved
<!-- wikilinks-footer-end -->