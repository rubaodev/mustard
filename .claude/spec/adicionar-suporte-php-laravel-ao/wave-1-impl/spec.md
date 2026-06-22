# wave-1-impl

## Resumo

Camada scan (apps/scan): linguagem PHP + manifesto composer, 100% por dado

## Rede

- Pai: [[adicionar-suporte-php-laravel-ao]]

## Tarefas

- [ ] - [ ] Fechar o risco de versão PRIMEIRO: inspecionar as versões publicadas de `tree-sitter-php` e confirmar qual release declara compatibilidade com `tree-sitter 0.26` (as grammars atuais no Cargo.toml sao 0.23-0.25). Se NENHUMA versao for compativel com o core 0.26 pinado em apps/scan/Cargo.toml:35, NAO force nem faca downgrade do core - pare e escale como decisao de design (a invariante de versao unica do native lib `links=tree-sitter` no workspace e inegociavel).
- [ ] - [ ] Confirmar o simbolo `LanguageFn` exportado pela crate: `LANGUAGE_PHP` reconhece `<?php ... ?>` embutido em texto, enquanto `LANGUAGE_PHP_ONLY` e PHP puro. Verificar na doc/fonte da crate qual constante cobre arquivos `.php` de codigo Laravel (controllers/models comecam com `<?php`) e escolher a adequada.
- [ ] - [ ] Em apps/scan/Cargo.toml, adicionar a linha de gramatica aliasada ao lado das demais (linhas 37-41), no MESMO formato: `grammar_php = { package = "tree-sitter-php", version = "<versao compativel confirmada>" }`. Nao tocar no pin `tree-sitter = "0.26"`.
- [ ] - [ ] Em apps/scan/languages.toml, adicionar uma entry `[[language]]` espelhando exatamente as existentes: `name = "php"`, `extensions = ["php"]` (sem ponto), `dir = "php"`, e `grammar = "grammar_php::<LANGUAGE_PHP|LANGUAGE_PHP_ONLY confirmado>"`. O build.rs emite a expressao `grammar` verbatim, entao o simbolo nunca aparece em src/.
- [ ] - [ ] Criar queries/php/tags.scm usando SO o vocabulario generico de captura (espelhar queries/python/tags.scm): mapear `use` (importacao) -> @import, declaracao de `namespace` -> @namespace, classe -> @definition.class com @name, funcao/metodo -> @definition.function com @name. Validar os nomes de nos da gramatica PHP por fora (AST de exemplo), NUNCA ensinando no algum a extract.rs.
- [ ] - [ ] Criar queries/php/supertypes.scm espelhando queries/python/supertypes.scm: capturar a base de `extends` e as interfaces de `implements` como @supertype. Manter so o vocabulario generico.
- [ ] - [ ] Em apps/scan/manifests.toml, adicionar uma entry `[[manifest]]` para o composer no formato existente (espelhar a entry npm/package.json): kind composer, filename `composer.json`, format json, name dir, deps `require` e `require-dev`, scripts `scripts`. Conferir se o leitor generico json ja cobre esse shape (nao inventar formato novo).
- [ ] - [ ] Rodar `cargo build -p scan` para que o build.rs releia languages.toml e embuta as novas queries/php/*.scm; corrigir qualquer erro de compilacao de query (tree-sitter descarta pattern invalido individualmente, mas confirme que tags/supertypes compilam).

## Arquivos

- `apps/scan/Cargo.toml`
- `apps/scan/languages.toml`
- `apps/scan/queries/php/tags.scm`
- `apps/scan/queries/php/supertypes.scm`
- `apps/scan/manifests.toml`
