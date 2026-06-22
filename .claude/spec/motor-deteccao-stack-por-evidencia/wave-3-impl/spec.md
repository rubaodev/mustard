# wave-3-impl

## Resumo

Scan: detected_stacks no modelo nativo + fiacao do motor em ingest/main + propagacao ao digest

## Rede

- Pai: [[motor-deteccao-stack-por-evidencia]]
- Depende de: [[wave-2-impl]]

## Tarefas

- [ ] - [ ] Adicionar `detected_stacks` (aditivo, serde default) ao modelo nativo do scan: apps/scan/src/model.rs ProjectModel (:16) e ProjectUnit (:50), reusando o tipo StackDetection do core.
- [ ] - [ ] Propagar o campo pela struct intermediaria apps/scan/src/ingest.rs:20 e pela montagem apps/scan/src/main.rs:250 (frameworks: ing.frameworks).
- [ ] - [ ] Em apps/scan/src/ingest.rs:146 (ao lado de infer_frameworks), chamar infer_stacks com as deps dos manifestos + os paths do walk + o conteudo ja lido (ingest.rs:60-138); popular detected_stacks.
- [ ] - [ ] Reexpor detected_stacks no digest: apps/scan/src/digest.rs:34 (CapabilityDigest). Nao quebrar frameworks nem o preserve_order; ordenacao/desempate estaveis (miner deterministico).
- [ ] - [ ] `cargo build -p mustard-core -p scan && cargo test -p scan`.

## Arquivos

- `apps/scan/src/model.rs`
- `apps/scan/src/ingest.rs`
- `apps/scan/src/main.rs`
- `apps/scan/src/digest.rs`
