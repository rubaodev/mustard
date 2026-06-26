# Saleor order app — benchmark de recall cross-lingual PT→EN (Python)

Repo OSS público (saleor/saleor, app `saleor/order`, ~329 funções, **Python/Django**, código 100% inglês), queries pt-BR. O 3º projeto / 3ª linguagem (C#/TS + TS + Python).

## Resultado (2026-06-25)
| retrieval | @1 | @5 | @12 |
|---|---|---|---|
| name-match | 0.0 | 0.1 | — |
| purpose-search | 0.2 | **0.6** | 0.7 |

## O LIMITE exposto (honesto)
3 misses, TODOS gap de sinônimo PT-interno entre a query e o purpose:
- `faturar` pagamento → purpose disse "cobrança" (faturar≠cobrança)
- pedido `quitado` → "totalmente pago" (quitado≠pago)
- `reabastecer` estoque → "repor estoque" (reabastecer≠repor)
O verbo do USUÁRIO ≠ o verbo natural do CÓDIGO (ambos PT). O cross-lingual (PT→EN) o purpose resolve; o sinônimo-PT-interno NÃO — precisa de PONTE DE LÉXICO. É a fronteira do fosso.

## Lição de setup (bug achado+corrigido)
O 1º enrich filtrou `/order/` e casou `graphql/order/` (camada GraphQL), PERDENDO o app `order/` (actions.py) → GT sem purpose → 0/10 falso. Corrigido (filtro `startswith("order/")`). Reforça: enriquecer o arquivo CERTO importa, e o e2e pega.

## Quadro dos 3 projetos
| projeto | linguagem | arquitetura | name→purpose (@5/@12) |
|---|---|---|---|
| sialia (privado) | C#/TS | services limpos | 0/10 → 10/10 (juiz) |
| Medusa | TS | action files limpos | 0.36 → 0.91 |
| Saleor | Python | GraphQL-heavy | 0.1 → 0.6 |
O fosso rende 0.6–1.0 conforme (a) limpeza da arquitetura e (b) alinhamento do vocabulário da query com a ação do código. Resíduo = sinônimo → léxico.
