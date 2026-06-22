# wave-4-impl

## Resumo

Scan: fixtures multi-stack + teste e2e de deteccao + gate anti-hardcode

## Rede

- Pai: [[motor-deteccao-stack-por-evidencia]]
- Depende de: [[wave-3-impl]]

## Tarefas

- [ ] - [ ] Criar fixtures multi-stack minimas sob apps/scan/tests/fixtures/ (ao menos Laravel/PHP e uma segunda stack p.ex. Django/Python) cobrindo os tres tipos de sinal (dep no manifesto + arquivo-marcador + assinatura de codigo).
- [ ] - [ ] Teste e2e `stack_detection_e2e` (espelhar facts_cli.rs): escanear a fixture Laravel e validar que detected_stacks contem name=laravel com os sinais que a sustentaram.
- [ ] - [ ] Gate AC-6 anti-hardcode: confirmar que o git diff desta mudanca NAO introduz nome de stack/sinal (laravel/django/illuminate/artisan) em .rs de logica de apps/scan/src ou packages/core/src; tudo vive em .toml/fixtures/testes.
- [ ] - [ ] Rodar a suite completa verde: `cargo test -p scan` (incl. stack_detection_e2e) + `cargo test -p mustard-core`.

## Arquivos

- `apps/scan/tests/fixtures/`
- `apps/scan/tests/stack_detection_e2e.rs`
