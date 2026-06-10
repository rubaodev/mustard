# wave-4-matching

## Resumo

P2 — escada de match por tiers (exato > fold > stem same-language > glossário) + novo contrato matched k/n no QueryResult/DigestQuery + consumidores

## Rede

- Pai: [[redesenho-agnostico-indice-termos-digest]]
- Depende de: [[wave-3-ranking]]

## Tarefas

- [ ] Indexar também o identificador-inteiro lowercased por declaração (1 entrada extra por decl)
- [ ] Substituir token_match (prefixo ≥4 bidirecional) pela escada de tiers em módulo próprio e coeso (ex.: apps/scan/src/matching.rs — digest.rs só orquestra, chamada módulo-qualificada direta, sem wrapper): T1 token exato OU ident-inteiro exato > T2 accent-fold > T3 stem da mesma língua > T4 glossário bilíngue (traduções como sinônimos OR); pesos ~10× por degrau; igualdade exata de chave em todo tier
- [ ] Módulo stemmers.rs espelho-de-dado (código de idioma → algoritmo do rust-stemmers; carve-out documentado — idiomas naturais, nunca linguagens de programação; zero comportamento por idioma além da seleção); dependência rust-stemmers no Cargo.toml
- [ ] Vendorizar stoplists Snowball como dado em apps/scan/stoplists/ (pt e en no seed; idioma novo = 1 arquivo de dado + 1 linha no espelho); idiomas da consulta = dedup([specLang da raiz, en]) — zero detecção de idioma
- [ ] Seed de léxico de domínio apps/scan/lexicons/ como dado (pt-en.toml no seed; par de idiomas novo = 1 arquivo, extensível por projeto)
- [ ] Contrato: QueryResult (apps/scan/src/digest.rs) e DigestQuery (packages/core/src/domain/scan.rs) trocam miss:bool por report — por termo {term, tier, lang, files}, agregado matched k/n, razão none|generated_only|weak|strong — seguindo o padrão core-model (tipos serde puros, sem efeito colateral)
- [ ] Atualizar o consumidor apps/rt/src/commands/feature.rs (payload expõe o report; a nota orienta re-consulta no vocabulário do código e Explore quando weak/none) e o teste do core que desserializa a saída real do digest
- [ ] Teste match_tiers.rs cobrindo falso cognato morto, ident-inteiro exato e match por glossário com tier reportado (exemplos pt↔en por serem o par do seed)

## Arquivos

- `apps/scan/src/digest.rs`
- `apps/scan/src/matching.rs`
- `apps/scan/src/stemmers.rs`
- `apps/scan/stoplists/pt.txt`
- `apps/scan/stoplists/en.txt`
- `apps/scan/lexicons/pt-en.toml`
- `apps/scan/Cargo.toml`
- `packages/core/src/domain/scan.rs`
- `apps/rt/src/commands/feature.rs`
- `apps/scan/tests/match_tiers.rs`
