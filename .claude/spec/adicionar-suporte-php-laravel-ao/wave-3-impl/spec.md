# wave-3-impl

## Resumo

Fixtures de projeto Laravel minimo + teste e2e de scan + gate anti-hardcode (AC-6)

## Rede

- Pai: [[adicionar-suporte-php-laravel-ao]]
- Depende de: [[wave-1-impl]], [[wave-2-impl]]

## Tarefas

- [ ] - [ ] Criar uma fixture de projeto Laravel minimo sob apps/scan/tests/fixtures/php_laravel/: um composer.json com require/require-dev (ex.: laravel/framework) e um bloco scripts; ao menos um app/Models/User.php que faca `namespace App\Models;`, `use Illuminate\Database\Eloquent\Model;` e `class User extends Model`; e um arquivo de rota (routes/web.php) usando `Route::`. Cobrir os sinais que a onda 2 declarou.
- [ ] - [ ] Escrever o teste e2e de extracao PHP php_extraction (espelhar o padrao de apps/scan/tests/facts_cli.rs: invocar o binario via env!("CARGO_BIN_EXE_scan")): parsear um .php com namespace/use/class/funcao e validar no JSON que imports/namespaces/declarations sairam do motor generico.
- [ ] - [ ] Escrever o teste e2e composer_manifest: escanear a fixture (subcomando scan com --out para um temp dir, ou facts) e validar que o manifesto composer aparece com suas deps de require/require-dev e seus scripts no modelo, preservando a ordem do manifesto (preserve_order).
- [ ] - [ ] Escrever o teste e2e php_laravel_fixture: rodar o scan sobre apps/scan/tests/fixtures/php_laravel e validar no grain.model.json resultante: (a) linguagem php presente em languages/modules; (b) manifesto/projeto com kind composer; (c) framework Laravel rotulado em frameworks. Usar diretorio temporario e limpeza, como faz facts_cli.rs.
- [ ] - [ ] Verificar o gate AC-6 anti-hardcode: confirmar que `rg -i "php|laravel|composer|artisan|illuminate"` em apps/scan/src e packages/core/src retorna ZERO ocorrencias - toda especificidade ficou em .toml/.scm/Cargo.toml/fixtures, nunca em src/.
- [ ] - [ ] Rodar a suite completa: `cargo test -p scan` (php_extraction, composer_manifest, php_laravel_fixture) e o gate de grep, garantindo verde e invariante agnostica intacta.

## Arquivos

- `apps/scan/tests/fixtures/php_laravel/composer.json`
- `apps/scan/tests/fixtures/php_laravel/app/Models/User.php`
- `apps/scan/tests/fixtures/php_laravel/routes/web.php`
- `apps/scan/tests/php_extraction.rs`
- `apps/scan/tests/php_laravel_fixture.rs`
