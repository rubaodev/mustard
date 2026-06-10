# Plano de Waves

## Tabela de Waves

| Wave | Spec | Papel | Depende de | Resumo |
|------|------|-------|------------|--------|
| 1 | [[wave-1-backend]] | backend | — | Instrumentar a adesão ao digest no rt: marco analyze.digest.used em feature.rs, comando digest-adherence-finalize que dobra a janela da sessão num analyze.digest.summary escopado ao spec, classificador is_source_file, classify_kind para analyze.*, extensão do metrics_observer para gravar path em Grep/Glob, e prosa SKILL. |
| 2 | [[wave-2-frontend]] | frontend | [[wave-1-backend]] | Superficializar a adesão no dashboard: projetar digest_used + source_reads_before_digest no SpecCard a partir do analyze.digest.summary escopado ao spec, espelhar no tipo TS, registrar tema/i18n dos eventos analyze.digest.*, e renderizar um badge no card consumidor. |

## Critérios de Aceitação
- cargo test -p mustard-rt source_class
- cargo test -p mustard-rt feature::tests
- cargo build -p mustard-rt && mustard-rt run digest-adherence-finalize --spec instrumentar-adesao-ao-digest-no
- cargo test -p mustard-rt digest_adherence
- cargo test -p mustard-rt classify
- cargo test --manifest-path apps/dashboard/src-tauri/Cargo.toml spec_card
- pnpm --dir apps/dashboard typecheck
- pnpm --dir apps/dashboard dashboard:build
