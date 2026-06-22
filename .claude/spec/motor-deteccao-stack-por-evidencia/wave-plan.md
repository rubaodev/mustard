# Plano de Waves

## Tabela de Waves

| Wave | Spec | Papel | Depende de | Resumo |
|------|------|-------|------------|--------|
| 1 | [[wave-1-impl]] | impl | — | Core: tipo StackDetection + campo detected_stacks (aditivo serde) + registro declarativo stacks.toml |
| 2 | [[wave-2-impl]] | impl | [[wave-1-impl]] | Core: motor generico infer_stacks (funde manifest_dep + path_marker + code_signature, scoring por convergencia) |
| 3 | [[wave-3-impl]] | impl | [[wave-2-impl]] | Scan: detected_stacks no modelo nativo + fiacao do motor em ingest/main + propagacao ao digest |
| 4 | [[wave-4-impl]] | impl | [[wave-3-impl]] | Scan: fixtures multi-stack + teste e2e de deteccao + gate anti-hardcode |

## Critérios de Aceitação
- AC-2 — Registro declarativo de stack: stacks.toml com schema multi-sinal e >=2 stacks de linguagens distintas: `cargo test -p mustard-core stacks_registry_parses`
- AC-4 — Evolucao serde aditiva: detected_stacks com #[serde(default)], payload antigo desserializa, frameworks permanece: `cargo test -p mustard-core detected_stacks_serde_compat`
- AC-3 — Motor generico por convergencia funde os 3 sinais e gradua confianca; teste prova baixa vs alta: `cargo test -p mustard-core infer_stacks`
- AC-1 — Build/test verdes nas crates afetadas: `cargo build -p mustard-core -p scan && cargo test -p mustard-core`
- AC-5 — Fiado e2e: scan da fixture Laravel produz detected_stacks com name=laravel e os sinais: `cargo test -p scan stack_detection_e2e`
- AC-6 — Invariante agnostica: git diff nao introduz literal de stack em .rs de logica de producao: `git diff --stat -- '*.rs'`
