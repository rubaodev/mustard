# wave-4-impl

## Resumo

Core: DigestQuery expoe stacks + 3a stack npm semeada no registro (generalidade alem de php/python)

## Rede

- Pai: [[robustez-agnostica-descoberta-padroes-grafo]]
- Depende de: [[wave-2-impl]]

## Tarefas

- [ ] - [ ] Ler packages/core/src/domain/scan.rs:66-88 (DigestQuery — a view tipada que o rt parseia do stdout de `scan digest --query`). A onda 2 adicionou `detected_stacks` ao QueryResult do scan (mesmo nome de campo).
- [ ] - [ ] Adicionar o campo de stacks ao DigestQuery com `#[serde(default)]` (payload antigo sem o campo desserializa). Reusar o tipo StackDetection de domain/vocabulary/stacks.rs. Aditivo: nenhum campo existente muda.
- [ ] - [ ] Semear a 3a stack no packages/core/src/domain/vocabulary/stacks.toml, de ecossistema npm (ex. nextjs: manifest_deps=["next"], path_markers=["next.config.js","next.config.mjs","next.config.ts"], code_signatures verificadas contra codigo Next real — confirme as assinaturas tipicas, ex. "next/router" ou "next/navigation"). SO DADO; verificar que nao colide com sinais existentes (dedup first-key-wins do automaton).
- [ ] - [ ] Testes (filtro `stacks` do QA ja cobre): atualizar stacks_registry_parses p/ >=3 stacks/3 ecossistemas; novo teste de parse do DigestQuery com stacks presente E ausente (default).
- [ ] - [ ] Rodar `cargo test -p mustard-core` completo.

## Arquivos

- `packages/core/src/domain/scan.rs`
- `packages/core/src/domain/vocabulary/stacks.toml`
