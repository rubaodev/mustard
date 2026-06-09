# wave-1-impl

## Resumo

Core: tipo StackDetection + campo detected_stacks (aditivo serde) + registro declarativo stacks.toml

## Rede

- Pai: [[motor-deteccao-stack-por-evidencia]]

## Tarefas

- [ ] - [ ] Definir a struct serde `StackDetection { name, confidence, signals }` em packages/core (junto dos tipos de dominio, ex. domain/scan.rs ou domain/vocabulary/stacks.rs) — pura, sem IO, contrato publico, reusavel pelo apps/scan (que depende do core).
- [ ] - [ ] Adicionar `detected_stacks: Vec<StackDetection>` com `#[serde(default)]` ao tipo projetado do core `packages/core/src/domain/scan.rs:128` (struct Project) SEM remover/alterar `frameworks: Vec<String>` (scan.rs:139).
- [ ] - [ ] Criar packages/core/src/domain/vocabulary/stacks.toml: schema [[stack]] com name, language (opcional), manifest_deps, path_markers, code_signatures; semear >=2 stacks de linguagens distintas (ex.: laravel/php + django/python) como DADO.
- [ ] - [ ] Adicionar o parser do stacks.toml espelhando FrameworkVocabulary (include_str! do builtin + override-aware via .claude/vocab/), sem hardcode de nome de stack na logica.
- [ ] - [ ] Testes: `stacks_registry_parses` e `detected_stacks_serde_compat` (payload antigo sem o campo desserializa; frameworks permanece). `cargo test -p mustard-core`.

## Arquivos

- `packages/core/src/domain/scan.rs`
- `packages/core/src/domain/vocabulary/stacks.toml`
