# wave-1-rt

## Resumo

Léxico supervisionado: evento feature.query + correlação determinística + comando lexicon-suggest com aceite humano (nunca auto-aplicar)

## Rede

- Pai: [[lexico-supervisionado-promover-pontes-confirmadas]]

## Tarefas

- [ ] Emitir feature.query: o run feature passa a registrar {queryTerms, report compacto (matched/total/reason + terms term/tier/lang)} como evento atribuído à spec/sessão ativa, usando a MESMA infraestrutura de emissão dos eventos existentes (mesmo canal do emit-event/route) — payload determinístico, sem timestamps próprios além do que o canal já carimba
- [ ] Comando novo lexicon-suggest (DOIS registros obrigatórios: variante no enum RunCmd em commands/mod.rs E braço no dispatch — esquecer o segundo compila mas o comando some): dobra determinística sobre os feature.query da mesma sessão/spec em ordem; para cada par consecutivo (q1, q2): termos X de q1 com tier none × termos NOVOS Y de q2 (Y não estava em q1) com tier exact/fold/stem viram candidatos {missed, bridged, files de evidência}; dedup por chave folded contra o léxico vigente (seed + overlay do projeto) e contra candidatos idênticos anteriores
- [ ] lexicon-suggest sem flags LISTA candidatos em JSON byte-estável; com --accept <missed>=<bridged> grava a entrada no <root>/.claude/lexicons/<par>.toml do projeto (NUNCA na seed embarcada; cria o arquivo a partir do shape do template se ausente; insere na seção [terms] em ordem alfabética preservando comentários existentes); par de idiomas resolvido como no digest (specLang da raiz + en)
- [ ] Invariante nunca-auto-aplicar: sem --accept nenhum arquivo é tocado, mesmo com candidatos pendentes (espelhar o padrão sugestão-sem-aplicar do tactical_fix_detect.rs); aceite de candidato já coberto é no-op idempotente
- [ ] Prosa: 1 linha no template do SKILL do /feature (apps/cli/templates) — após uma re-consulta bem-sucedida pós-weak/none, sugerir rodar lexicon-suggest para promover as pontes confirmadas
- [ ] Testes nomeados pelos ACs: lexicon_correlation_* (dois feature.query sintéticos → candidato com par e evidência; sem re-consulta → zero candidatos), lexicon_accept_* (aceite grava no overlay do projeto com ordenação determinística e nunca na seed), lexicon_no_auto_* (listagem não escreve nada), dedup contra léxico vigente

## Arquivos

- `apps/rt/src/commands/feature.rs`
- `apps/rt/src/commands/lexicon_suggest.rs`
- `apps/rt/src/commands/mod.rs`
- `apps/cli/templates/commands/mustard/feature/SKILL.md`
- `packages/core/src/domain/scan.rs`
