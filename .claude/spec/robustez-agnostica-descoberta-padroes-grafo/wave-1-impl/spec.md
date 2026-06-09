# wave-1-impl

## Resumo

Grafo de imports: resolucao generica de namespace/FQCN para TODAS as linguagens parametrizadas

## Rede

- Pai: [[robustez-agnostica-descoberta-padroes-grafo]]

## Tarefas

- [ ] - [ ] Ler apps/scan/src/graph.rs:191-226 e entender a resolucao atual: ns_index keyed por namespace; ramo de path exige '/'. Defeito verificado: FQCN PHP `App\Models\User` nunca resolve -> graph.edges=0 SEMPRE em PHP; consequencia em cascata: layers colapsam em L0, touchpoints/hubs/fan-in vazios.
- [ ] - [ ] ANTES de mudar, escrever um teste de caracterizacao por linguagem: fixtures minimas (2-3 arquivos cada) com import INTERNO real em C# (using Ns), Python (from pkg.mod import X), TypeScript (import relativo), Go (import de package interno), Rust (use crate::...), PHP (use App\Models\User) — rodar e registrar quais linguagens JA produzem edges hoje (nao regredir nenhuma).
- [ ] - [ ] Implementar a resolucao GENERICA (sem nome de linguagem na logica): (a) normalizar separadores de segmento (`\`, `::`, `.`, `/`) para uma forma canonica ao indexar e ao resolver; (b) se o import nao casar um namespace do ns_index diretamente, retentar com o ultimo segmento removido (caso FQCN-de-tipo); (c) o ramo de path deixa de exigir '/' literal (usa a forma canonica). Determinismo: desempates estaveis.
- [ ] - [ ] Testes `graph_resolution_*` (nome exato p/ o filtro do QA): um por linguagem, assertando edges>0 com import interno + um teste de nao-regressao (linguagens que ja resolviam continuam com os MESMOS edges).
- [ ] - [ ] Smoke de cascata: na fixture PHP ampliada (criar sob tests/fixtures se necessario: 3 Models + 2 Controllers + 2 Services com imports internos), validar que hubs/touchpoints/layers deixam de ser vazios apos o fix.
- [ ] - [ ] Rodar `cargo test -p scan` completo — atencao a testes existentes que assertem hubs/layers/fan_in (facts_cli etc.): se mudarem por haver MAIS edges (melhoria intencional), atualizar o assert com justificativa em comentario de teste; se mudarem de forma inexplicavel, PARE e reporte.

## Arquivos

- `apps/scan/src/graph.rs`
- `apps/scan/tests/graph_resolution.rs`
- `apps/scan/tests/fixtures/`
