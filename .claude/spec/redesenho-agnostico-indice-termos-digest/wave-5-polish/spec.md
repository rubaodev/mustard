# wave-5-polish

## Resumo

P6+P7 — estratificação de samples por subprojeto + diversidade MMR + peso por classe de kind no catálogo publicado

## Rede

- Pai: [[redesenho-agnostico-indice-termos-digest]]
- Depende de: [[wave-4-matching]]

## Tarefas

- [ ] Estratificação: quando ≥2 estratos (projects[].dir do modelo) têm match, cada um garante ≥1 vaga nos samples; repositório de 1 projeto degenera para o ranking global sem efeito
- [ ] Diversidade MMR greedy (λ em TOML) nas vagas restantes: penalidade por similaridade com os já escolhidos (Jaccard de subtokens + profundidade de diretório comum + vizinhança de imports); desempate por path ascendente — no mesmo módulo coeso da amostragem definido na onda 3
- [ ] Peso por classe de kind no ranking do catálogo publicado (tipo ×2.5, membro ×1 — vocabulário de kind genérico do motor, valores em TOML) protegendo o cap MAX_TERMS da inundação de termos de membro
- [ ] Teste stratified_samples.rs com fixture de monorepo (2 subprojetos de linguagens distintas)
- [ ] Rodada final: cargo test --workspace + cargo clippy verdes; varredura de regressão nos guards do scan (zero nomes de linguagem de programação em src/)

## Arquivos

- `apps/scan/src/digest.rs`
- `apps/scan/tests/stratified_samples.rs`
- `apps/scan/tests/fixtures/monorepo_mix/`
