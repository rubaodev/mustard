# Lexico supervisionado: promover pontes confirmadas pela re-consulta da IA a sugestoes curadas do lexico do projeto, com evento de candidato, comando de listagem/aceite e nunca auto-aplicar

<!-- drafter:tone=didactic — Write this spec narrative in didactic tone — expand abbreviations on first use (AC = Acceptance Criteria, wave = onda) and prefer plain words over jargon. -->

<!-- PRD -->

## Contexto

O digest responde consultas de feature com um relatório honesto por termo: o que casou (e por qual degrau — exato, acento, flexão ou léxico bilíngue) e o que ficou de fora, nomeado. Quando um termo de domínio não tem ponte entre idiomas (ex.: um pedido em português contra código em inglês), a nota orienta o orquestrador (a inteligência artificial que conduz o pipeline) a re-consultar no vocabulário do código — e essa re-consulta costuma acertar. Hoje, porém, esse acerto se perde: a tradução que funcionou não vira conhecimento durável, e a mesma ponte precisa ser redescoberta a cada feature. Esta spec fecha o ciclo: quando a re-consulta confirma uma ponte (termo X falhou; termo Y casou forte na sequência), a ferramenta registra o par como candidato e oferece um comando de revisão para o humano aceitar — o aceite grava a entrada no léxico do próprio projeto (.claude/lexicons/), nunca na seed embarcada, e a próxima consulta casa de primeira, sem inteligência artificial no caminho. O léxico cresce com o uso, sempre curado por gente.

Âncoras (do scan):
- apps/rt/src/commands/spec/tactical_fix_detect.rs
- apps/scan/src/graph.rs
- apps/rt/src/commands/agent/context_inject.rs
- apps/rt/src/commands/spec/spec_clear.rs
- packages/core/src/domain/regression_check/snapshot.rs
- apps/rt/src/commands/spec/active_specs.rs
- packages/core/src/domain/knowledge.rs
- apps/rt/src/hooks/session/spec_hygiene_observer.rs

Fatias recorrentes (precedente a espelhar): id (×2)

Por que agora: o redesenho agnóstico do índice acabou de entregar o relatório por termo e o overlay de léxico por projeto — as duas pontas que este ciclo conecta; sem ele, cada buraco de vocabulário paga o custo da re-consulta para sempre.

## Usuários/Stakeholders

Times que pedem features em português sobre código em inglês (ou o inverso) e o orquestrador do pipeline, que deixa de redescobrir as mesmas traduções; o mantenedor do Mustard, que mantém a seed pequena e genérica sem pressão para inchá-la. Solicitante: Rubens.

## Métrica de sucesso

Depois de uma feature em que a re-consulta fechou um buraco de vocabulário, o comando de revisão lista o par com a evidência (termo que faltou, termo que casou, arquivos), e após o aceite humano a MESMA consulta original passa a casar via léxico de primeira — sem nenhuma escrita automática ter acontecido antes do aceite.

## Não-Objetivos

Auto-aplicar candidatos sem aceite humano (decisão herdada do tactical-fix-detect: nunca auto-aprovar); tocar a seed embarcada da ferramenta (candidatos vão sempre ao léxico do projeto); tradução por modelo de linguagem dentro do tool (a IA traduz no pipeline; o tool só observa e registra); qualquer rede ou heurística não-determinística na correlação.

## Critérios de Aceitação

- **AC-1** — Correlação determinística: duas consultas consecutivas do mesmo contexto (mesma spec/sessão) em que um termo falhou (tier none) e a re-consulta casou forte geram um evento lexicon.candidate com o par e a evidência.
  Command: `cargo test --workspace lexicon_correlation`
- **AC-2** — Revisão com aceite: o comando de listagem mostra candidatos com evidência; o aceite grava a entrada no .claude/lexicons/<par>.toml do projeto (nunca na seed embarcada) com ordenação determinística.
  Command: `cargo test --workspace lexicon_accept`
- **AC-3** — Nunca auto-aplica: sem aceite explícito, nenhum arquivo de léxico é alterado, mesmo com candidatos pendentes.
  Command: `cargo test --workspace lexicon_no_auto`
- **AC-4** — Workspace inteiro verde.
  Command: `cargo test --workspace`

<!-- PLAN -->

## Arquivos

- apps/rt/src/commands/feature.rs — correlação entre consultas consecutivas (report da anterior × matches da atual) + emissão do lexicon.candidate
- apps/rt/src/commands/ (novo) lexicon_suggest.rs — listar candidatos com evidência + aceite que grava no léxico do projeto (espelhar o padrão sugestão-sem-aplicar do tactical_fix_detect.rs)
- packages/core — tipo serde do candidato (padrão core-model), se o evento precisar de shape tipado
- apps/cli/templates — prosa do /feature: 1 linha oferecendo a revisão de candidatos quando houver pendentes
- testes correspondentes no rt

## Limites

IN: rt (correlação + evento + comando de revisão/aceite) e a prosa do SKILL; escrita SÓ em .claude/lexicons/<par>.toml do projeto consumidor.
OUT: seed embarcada do scan; matching/digest (já prontos — o overlay lê o arquivo do projeto); qualquer aplicação automática.

## Decisões não-óbvias

- Sugestão-sem-aplicar é invariante (precedente: tactical-fix-detect "não auto-aprovar"); o aceite é o único caminho de escrita.
- A evidência da correlação é o report matched k/n que o digest já emite — nenhuma telemetria nova é necessária; a correlação é uma dobra determinística sobre eventos existentes.
- Candidato duplicado ou já coberto pelo léxico vigente é descartado em silêncio (dedup por chave folded).
## Preocupações

- (review, menor) O emissor do feature.query atribui via diretório de trabalho enquanto o digest aceita --root: com --root diferente do cwd, o evento cai no workspace do cwd e o lexicon-suggest --root <outro> encontra zero consultas. Segue a convenção dos emissores da face run, mas a divergência é real — alinhar quando a face run ganhar atribuição por --root.
- (review, menor) Com id de sessão irresoluvível, o filtro de sessão do collect_queries é pulado e eventos de spec de outras sessões correlacionam juntos — escolha tolerante documentada; apertar se virar ruído.
- (review, menor) 2 lints warn-level nos arquivos tocados (doc_lazy_continuation pré-existente em feature.rs; needless_borrow em lexicon_suggest.rs:164) — nenhum é deny.
