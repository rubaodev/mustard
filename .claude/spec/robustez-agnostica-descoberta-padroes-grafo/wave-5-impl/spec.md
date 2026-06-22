# wave-5-impl

## Resumo

RT: payload do feature expoe stacks + facts line dos Guards ganha stacks= (gerador, parser, preserve)

## Rede

- Pai: [[robustez-agnostica-descoberta-padroes-grafo]]
- Depende de: [[wave-4-impl]]

## Tarefas

- [ ] - [ ] Ler apps/rt/src/commands/feature.rs:25-65 (digest_query -> payload JSON do `mustard-rt run feature`): adicionar campo `stacks` ao payload, vindo do DigestQuery estendido na onda 4 (name+confidence+signals, ou name+confidence se o payload ficar verboso — decida pelo consumo do orquestrador).
- [ ] - [ ] Ler apps/rt/src/commands/scan_claude.rs:163 (build_guards_block gera `<!-- facts: kind=...; frameworks=... -->`) e :418 (alimentado por project.frameworks). Adicionar `stacks=name(0.95),name2(0.65)` a partir de project.detected_stacks (o campo JA chega populado via read_projects — verificado na auditoria). MANTER frameworks= por compatibilidade. Se detected_stacks vazio, omitir o segmento stacks= (linha identica a atual).
- [ ] - [ ] Estender o parser apps/rt/src/commands/scan_guards/list.rs:133-166 (parse_facts) para o segmento stacks= e o preserve em apply.rs:151-154 — os tres em conjunto (gerador/parser/preserve), senao o apply degrada a linha.
- [ ] - [ ] Testes (nome exato p/ QA): `stacks_facts_*` — (1) build_guards_block com detected_stacks produz stacks= e sem ele produz a linha legada identica; (2) parse_facts faz round-trip do segmento novo; (3) payload do feature carrega stacks. Espelhar o estilo dos testes existentes de scan_claude/scan_guards.
- [ ] - [ ] Rodar os testes dos modulos tocados (`cargo test -p mustard-rt scan_claude scan_guards feature stacks_facts` ou o filtro que cubra) e reportar numeros; suite completa do rt e pesada — rode-a uma vez ao final se o tempo permitir.

## Arquivos

- `apps/rt/src/commands/feature.rs`
- `apps/rt/src/commands/scan_claude.rs`
- `apps/rt/src/commands/scan_guards/list.rs`
- `apps/rt/src/commands/scan_guards/apply.rs`
