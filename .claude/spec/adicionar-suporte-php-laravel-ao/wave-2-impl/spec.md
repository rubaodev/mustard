# wave-2-impl

## Resumo

Camada core (packages/core): sinais do Laravel no vocabulario de frameworks

## Rede

- Pai: [[adicionar-suporte-php-laravel-ao]]

## Tarefas

- [ ] - [ ] Ler packages/core/src/domain/vocabulary/frameworks_builtin.toml e seu cabecalho de schema: cada [[signal]] tem category (orm | framework | di) e patterns (substrings literais casadas por Aho-Corasick em uma passagem; a ordem entre categorias e prioridade em colisao cross-category).
- [ ] - [ ] Acrescentar sinais do Laravel no formato existente, sem criar categoria nova: na categoria framework, adicionar substrings literais como o namespace raiz `Illuminate\` (um backslash), `Route::` (rotas) e `extends Controller` (controller base). Manter os patterns curtos e como SINAIS, nao gramaticas.
- [ ] - [ ] Para o ORM Eloquent, adicionar na categoria orm o sinal de Model: confirmar a forma exata que casa (ex.: `Illuminate\Database\Eloquent\Model` no use, e/ou `extends Model`). ATENCAO: `extends Model` e `extends Controller` podem ja existir no toml - NAO duplicar pattern existente; reusar/realocar se preciso.
- [ ] - [ ] Decidir onde declarar o sinal `artisan` de forma agnostica (substring literal numa categoria existente), confirmando que ele aparece em conteudo de arquivo de codigo (nao so em composer.json), para nao criar falso-negativo no teste.
- [ ] - [ ] Garantir que TODOS os literais novos (Illuminate, Route::, artisan, etc.) vivem apenas neste .toml embutido via include_str! (frameworks.rs:41 BUILTIN_FRAMEWORKS_TOML), sem tocar em frameworks.rs nem em qualquer arquivo de packages/core/src/*.rs.
- [ ] - [ ] Rodar a suite do core para confirmar que detect_framework_signals casa o conteudo Laravel e que o automaton unico Aho-Corasick continua valido: `cargo test -p mustard-core`.

## Arquivos

- `packages/core/src/domain/vocabulary/frameworks_builtin.toml`
