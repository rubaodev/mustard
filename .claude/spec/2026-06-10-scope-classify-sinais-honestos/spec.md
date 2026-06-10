# Tactical Fix: scope-classify — newEntityCount estrutural + sliceMatchCount sem gatilho cego de full

## Contexto

Auditoria 2026-06-10 (memória `mustard-sialia-payables-audit`): na run payables do sialia o classificador retornou `full` com `newEntityCount: 5` — os "5 entes novos" eram `{As, Graph, QL, Nenhuma, UI}` (pascal_tokens conta qualquer palavra capitalizada da prosa; "Nenhuma" veio da frase "Nenhuma entidade nova" do próprio draft; GraphQL quebra em Graph+QL; contra o spec.md real daria 26). E `sliceMatchCount >= 2 ⇒ full` (scope_decompose.rs:391) mede sobreposição de vocabulário com o catálogo (qualquer pedido que nomeie o domínio dá ≥2; satura no cap Q_MAX_SLICES=12), não camadas atravessadas. O sinal correto (layerCount=1, fileCount=7) foi sobrescrito; com sinais honestos a run seria extended-light. Viés sistêmico: 13/16 specs payables encerradas no sialia = full.

Fix: (1) newEntityCount derivado estruturalmente do `## Arquivos`/`## Files` com marcador (create)/(novo) diffado contra paths do modelo, e/ou pascal_tokens filtrado por stoplists vendoradas + siglas de pipeline (UI/AC/QA/PRD/API) + não contar fragmento de split camelCase quando o token composto inteiro é conhecido; (2) sliceMatchCount deixa de forçar full quando layerCount==1 && fileCount<=8 (vira evidência de precedente para light/extended-light).

## Critérios de Aceitação

- **AC-1** — Testes do classificador: prosa PT com "Nenhuma entidade nova" → newEntityCount 0; "GraphQL" não vira Graph+QL; caso payables real (1 layer, 7 files, sliceMatchCount 7) → extended-light, não full.
  Command: `cargo test -p mustard-rt scope`
- **AC-2** — Workspace verde.
  Command: `cargo test --workspace`

## Arquivos

- apps/rt/src/commands/spec/scope_decompose.rs — regra de decisão (linhas ~389-393) + new_entity_count_from_model (~337-354)
- apps/rt/src/commands/spec/prd_build.rs — pascal_tokens (~92-134)
- testes em apps/rt