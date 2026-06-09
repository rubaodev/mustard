# wave-2-impl

## Resumo

Core: motor generico infer_stacks (funde manifest_dep + path_marker + code_signature, scoring por convergencia)

## Rede

- Pai: [[motor-deteccao-stack-por-evidencia]]
- Depende de: [[wave-1-impl]]

## Tarefas

- [ ] - [ ] Implementar `infer_stacks(deps, paths, contents)` em packages/core/src/domain/vocabulary/stacks.rs: para cada [[stack]] do registro, casar manifest_deps (substring nas deps parseadas), path_markers (presenca de arquivo/dir) e code_signatures (REUSAR detect_framework_signals / o automaton Aho-Corasick unico, nao instanciar outro).
- [ ] - [ ] Scoring por convergencia DETERMINISTICO: confianca cresce com o numero de TIPOS de sinal que bateram (1=baixa, 2=media, 3=alta); limiares explicitos e versionados.
- [ ] - [ ] Motor CEGO ao nome: a logica apenas itera o registro e copia o `name` verbatim; nenhum literal de stack/sinal em .rs. Saida explicavel (cada StackDetection carrega os sinais que a sustentaram).
- [ ] - [ ] Fail-safe: registro ausente/parse-erro degrada para vazio sem panic e SEM inventar deteccao.
- [ ] - [ ] Teste `infer_stacks` com fixture multi-sinal provando baixa (1 sinal) vs alta (3 sinais) confianca. `cargo test -p mustard-core infer_stacks`.

## Arquivos

- `packages/core/src/domain/vocabulary/stacks.rs`
