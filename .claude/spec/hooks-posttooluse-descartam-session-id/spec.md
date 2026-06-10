# Hooks PostToolUse descartam session_id e gravam eventos em .session/unknown/

<!-- drafter:tone=didactic -->

## Contexto

Toda a telemetria por-ferramenta do Mustard (eventos `tool.use`, `tool.result`, `pipeline.economy.*`) está sendo gravada em `.claude/.session/unknown/.events/` no projeto consumidor, em vez de ir para a pasta da sessão real (`.session/<id>/`). Observado em campo no repositório `sialia`: praticamente todos os eventos por-ferramenta de um dia inteiro de trabalho caíram em `unknown`. Apenas o evento `session.start` (emitido pelo hook de início de sessão) carrega o identificador de sessão correto.

Consequência: qualquer métrica por-sessão do painel (dashboard) colapsa em "unknown" — o agregador de sessões não consegue separar o trabalho de uma sessão do de outra. É um defeito do *escritor* (writer) em `apps/rt`, distinto da spec `dashboard-sqlite-out-telemetria-ndjson`, que trata só o lado da leitura em `apps/dashboard/src-tauri`.

## Usuários/Stakeholders

Quem opera o dashboard de telemetria e qualquer agregação por-sessão (consumo, atividade, trilha) — hoje todos veem "unknown".

## Métrica de sucesso

Eventos por-ferramenta de uma sessão passam a cair em `.session/<id>/`; zero eventos novos em `.session/unknown/` quando o `session_id` chega no input do hook.

## Não-Objetivos

- Não recriar SQLite nem mexer no lado da leitura do dashboard (`apps/dashboard/src-tauri`) — é outra spec.
- Não alterar o formato dos eventos nem os emissores *run-face* (que dependem corretamente de variável de ambiente).

## Critérios de Aceitação

- **AC-1** — a reprodução vira verde: um teste de regressão prova que um observer PostToolUse, recebendo `HookInput { session_id: Some("s-x"), .. }`, grava o evento sob `.session/s-x/`, e não sob `.session/unknown/`. Falha (código diferente de zero) antes do conserto e passa (código zero) depois.
  Command: `cargo test -p mustard-rt session_id_threaded_from_hook_input`
- **AC-2** — a suíte do crate fica verde, sem regressão nos hooks nem na telemetria.
  Command: `cargo test -p mustard-rt`
- **AC-3** — sem aviso novo de lint: o crate trata `clippy::unwrap_used`/`expect_used` como `deny`, e o conserto não pode introduzir nenhum.
  Command: `cargo clippy -p mustard-rt --all-targets`

## Causa raiz

O identificador de sessão chega ao hook pelo stdin do harness e já está modelado em `HookInput.session_id: Option<String>` (`packages/core/src/domain/model/contract.rs:137-139`). Mas:

1. `common::build_harness_event` (`apps/rt/src/hooks/task/common.rs:122`) crava `session_id: "unknown"` e nunca lê o `HookInput`.
2. `route::emit` (`apps/rt/src/shared/events/route.rs:129-139`) só honra um id vindo no evento no ramo `else` (quando `event.session_id` não é vazio nem `"unknown"`). Como o evento nasce `"unknown"`, cai no ramo de resolução por `shared::context::session_id()`, que só lê as variáveis de ambiente `MUSTARD_SESSION_ID`/`CLAUDE_SESSION_ID` (`apps/rt/src/shared/context.rs:88-94`) — ausentes no processo do hook PostToolUse — e devolve `"unknown"` → bucket `.session/unknown/`.
3. Agravante: `record_task_run` (`common.rs:32`) já *recebe* `session_id: Option<&str>` e o coloca no `SpanRecord`, mas ao emitir chama `emit_event(...)`, que descarta o id e crava `"unknown"` no envelope.

O hook de `SessionStart` escapa porque captura o id do input — por isso o `session.start` sai correto.

## Plano

Conserto cirúrgico num único ponto de estrangulamento (chokepoint) + repasse nos call-sites:

1. `common.rs`: `emit_event` e `build_harness_event` ganham um parâmetro `session_id: Option<&str>` e carimbam o valor no `HarnessEvent` (fallback `"unknown"` apenas quando `None`). `route::emit` já honra um id não-`unknown`.
2. Call-sites que têm `HookInput` à mão repassam `input.session_id.as_deref()`: `metrics_observer` (`tool.use`), `skill_usage_observer` e `subagent_observer` (2 call-sites: `agent.start` + o segundo emit).
3. `record_task_run` repassa o `session_id` que já recebe.
4. `tool_result_observer` (`apps/rt/src/hooks/observe/tool_result_observer.rs:268`) tem seu próprio `emit_event` local — aplicar o mesmo repasse a partir do seu `HookInput`.
5. Teste de regressão `session_id_threaded_from_hook_input` em `apps/rt` (cobre o AC-1).

Depois do verde: rebuild do binário para o conserto valer em sessão — atenção ao lock do `.exe` no Windows (o servidor MCP e o coletor OTEL travam o binário; reconectar `/mcp` e parar o coletor pela porta antes de reinstalar).

## Limites

- Mudança mecânica e uniforme: o mesmo repasse de `session_id` em cada emissor de hook.
- Emissores *run-face* (por exemplo `epic_fold::emit_event`) ficam de fora: não têm `HookInput`; dependem corretamente da resolução por variável de ambiente.
- Não recriar atribuição em outro lugar — o `route::emit` já busca por `event.session_id`; basta não jogá-lo fora antes.

## Checklist

- [x] `common.rs`: `emit_event`/`build_harness_event` aceitam `session_id: Option<&str>` e carimbam no `HarnessEvent`
- [ ] `record_task_run` repassa o `session_id` que já recebe ao `emit_event`
- [ ] `metrics_observer`/`skill_usage_observer`/`subagent_observer` repassam `input.session_id.as_deref()`
- [ ] `tool_result_observer` (emit local) repassa o `session_id` do seu `HookInput`
- [ ] Teste de regressão `session_id_threaded_from_hook_input` (AC-1) + suíte verde

<!-- signals: apps/rt,hooks,telemetry,session-attribution -->