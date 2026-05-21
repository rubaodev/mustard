---
name: mustard-prd
description: "Use when the user wants to lapidate (estruturar) a free-text intent into a structured PRD JSON matching the PrdForm of the dashboard's PRD Builder page. Create the JSON only — no spec file, no code Read, no opinion — confronts the intent against entity-registry + Glob."
source: manual
---
<!-- mustard:generated -->
# /mustard:prd — Lapidador de Intenção em PRD JSON

## Trigger

`/mustard:prd <intent>`

`<intent>` é texto livre passado pelo usuário (por exemplo: `adicionar refresh token no login`). Recebido via `$ARGUMENTS`.

## Description

Este comando recebe uma intenção em linguagem natural e devolve um **JSON** no formato exato do `PrdForm` consumido pela página *PRD Builder* do dashboard. Ele NÃO escreve nada em disco, NÃO chama `Task(Explore)`, NÃO lê arquivos de código, e NÃO opina se a ideia faz sentido. Apenas (1) estrutura o texto livre no shape esperado e (2) confronta mecanicamente o conteúdo contra `entity-registry.json` e contra a existência de paths via `Glob`.

Pensado para ser invocado de fora (do dashboard via Tauri) com `claude -p` + `--output-format json`, de modo que o consumidor receba JSON puro, sem prosa nem markdown.

## Action

1. **Receber a intenção.** Ler `$ARGUMENTS` como string única. Se vier vazio, devolver um JSON mínimo com `summary: ""` e `_confront` zerado — nunca abortar.

2. **Confronto com entity-registry (sem Read).**
   - Extrair tokens em `PascalCase` da intenção via regex local (`/\b[A-Z][a-zA-Z0-9]+\b/g`).
   - Rodar `Grep` em `.claude/entity-registry.json` por cada token. NÃO usar `Read` no arquivo inteiro.
   - Tokens com match → `_confront.entitiesFound[]`.
   - Tokens sem match → `_confront.entitiesMissing[]`.

3. **Confronto com paths (apenas existência, sem Read).**
   - Rodar `Glob` em padrões comuns do projeto (ex: `src/**/*.{ts,tsx}`, `apps/**/*`, `src/**/*.{rs,py}`) — apenas para conferir existência. NÃO abrir nenhum arquivo.
   - Para cada entidade encontrada, sugerir paths típicos (ex: `src/{entity}.ts`, `apps/api/src/routes/{entity}.ts`) e checar existência via `Glob`.
   - Paths que existem → `_confront.pathsExist[]`.
   - Paths que a intenção implica mas que não existem → `_confront.pathsMissing[]`.

4. **Inferir scope** com a heurística abaixo (decisão local, sem perguntar ao usuário):
   - `scope = "full"` SE `entitiesFound.length >= 2` OU `intent.split(' ').length >= 15` OU `intent` casa com `/CRUD|migration|workflow|fluxo|esquema/i`.
   - Caso contrário → `scope = "light"`.

5. **Montar o JSON** no shape descrito em *Output shape* abaixo. Imprimir **apenas** o JSON cru (`camelCase`, válido para `JSON.parse`) — sem fence markdown, sem comentário, sem prosa antes ou depois. Esta é a única saída do comando.

6. **Restrições duras** (violar = bug):
   - NUNCA chamar `Task(Explore)`.
   - NUNCA usar `Read` em arquivos de código.
   - NUNCA opinar se a ideia é boa, viável ou prioritária.
   - NUNCA incluir análise em prosa dentro do JSON.
   - NUNCA criar arquivo de spec, nem escrever em `.claude/spec/`.
   - NUNCA misturar saídas (logs, banners, tabelas) com o JSON.

## Output shape (JSON)

Formato TypeScript-like esperado pelo `PrdForm`:

```ts
{
  type: "feature" | "bugfix",                     // default "feature"
  slug: string,                                   // slugify(title), kebab-case
  title: string,                                  // curto, imperativo, derivado da intenção
  scope: "light" | "full",                        // inferido pela heurística do passo 4
  summary: string,                                // 1-2 frases técnicas (síntese, não análise)
  why?: string,                                   // opcional — motivação de negócio se óbvia
  layers: {
    backend: boolean,
    frontend: boolean,
    database: boolean,
    design: boolean,
    docs: boolean,
    testes: boolean
  },
  boundaries: string[],                           // paths do Glob, pré-populados por layer/entidade
  checklist: string[],                            // 3-6 passos imperativos (passos, NÃO análise)
  acceptanceCriteria: {
    title: string,
    command: string                               // comando executável (cross-shell)
  }[],                                            // 1-3 itens
  decisionsNotObvious?: string[],                 // opcional
  nonGoals?: string[],                            // opcional
  _confront: {
    entitiesFound: string[],
    entitiesMissing: string[],
    pathsExist: string[],
    pathsMissing: string[]
  }
}
```

Regras de preenchimento:

- `slug` = `slugify(title)` — derivado, nunca pedido ao usuário.
- `title` = imperativo curto (≤8 palavras) derivado da intenção.
- `type` = `"bugfix"` se intenção casa com `/bug|erro|quebrad|fix|corrigir|broken/i`; senão `"feature"`.
- `layers.*` = `true` se a intenção menciona sinais daquela camada (ex: "tela"/"componente" → frontend; "endpoint"/"API" → backend; "tabela"/"campo"/"coluna" → database).
- `boundaries[]` = paths existentes (de `_confront.pathsExist`) + paths novos plausíveis (de `_confront.pathsMissing`), filtrados pelas layers ativas.
- `acceptanceCriteria[].command` segue o padrão cross-shell (`node -e "..."`, `bash -c '...'`, ou comando único). Nunca usar `for`/`test`/`[ ]` cru.

## Example invocation

```bash
claude -p "/mustard:prd add refresh token to login" --output-format json --model claude-sonnet-4-6
```

Invocado do dashboard via Tauri: a UI passa o path do projeto como `cwd`, captura `stdout`, faz `JSON.parse`, e popula os campos do `PrdForm`.

## Example output

Para a intenção `adicionar export PDF nos relatórios`:

```json
{
  "type": "feature",
  "slug": "adicionar-export-pdf-nos-relatorios",
  "title": "Adicionar export PDF nos relatórios",
  "scope": "light",
  "summary": "Permitir que o usuário exporte relatórios já existentes em formato PDF a partir da UI atual de relatórios.",
  "why": "Usuários precisam compartilhar relatórios fora da ferramenta sem screenshot.",
  "layers": {
    "backend": true,
    "frontend": true,
    "database": false,
    "design": false,
    "docs": false,
    "testes": true
  },
  "boundaries": [
    "src/components/reports/ReportView.tsx",
    "src/api/reports/export.ts"
  ],
  "checklist": [
    "Adicionar botão Export PDF no header de ReportView",
    "Criar endpoint /reports/:id/export.pdf no backend",
    "Gerar PDF a partir do payload já carregado do relatório",
    "Disparar download no frontend após resposta"
  ],
  "acceptanceCriteria": [
    {
      "title": "Endpoint de export responde 200 com content-type application/pdf",
      "command": "node -e \"fetch('http://localhost:3000/reports/1/export.pdf').then(r => process.exit(r.headers.get('content-type')==='application/pdf'?0:1))\""
    }
  ],
  "decisionsNotObvious": [
    "Biblioteca de PDF (puppeteer vs pdfkit) — escolher na PLAN"
  ],
  "nonGoals": [
    "Customização visual do PDF (templates, temas)"
  ],
  "_confront": {
    "entitiesFound": ["Report"],
    "entitiesMissing": [],
    "pathsExist": ["src/components/reports/ReportView.tsx"],
    "pathsMissing": ["src/api/reports/export.ts"]
  }
}
```

## Rules

- Saída é **JSON puro** — sem markdown, sem fence, sem prosa, sem logs. Deve passar em `JSON.parse` sem ajuste.
- NUNCA `Task(Explore)`. NUNCA `Read` em arquivos de código. Apenas `Grep` no `entity-registry.json` e `Glob` para checar existência.
- NUNCA opinar (não há "isso é uma boa ideia", "isso vai dar problema", "considere X").
- NUNCA criar arquivos em `.claude/spec/`. Este comando não escreve nada em disco.
- `slug` é sempre derivado de `title` (kebab-case). `title` é sempre derivado da intenção.
- `scope` é inferido pela heurística do passo 4 — nunca perguntar ao usuário.
- `_confront` é sempre preenchido (mesmo que com arrays vazios). É a parte mecânica e auditável da saída.
- Cross-shell em `acceptanceCriteria[].command`: preferir `node -e "..."` ou `bash -c '...'`. Nunca `for`/`test`/`[ ]` cru (quebra no Windows).
- Comentários e identificadores em código (incluindo os comandos de AC) sempre em inglês, independente do idioma da intenção do usuário.
- Se a intenção for vazia ou ininteligível, devolver JSON mínimo válido com `summary: ""` e `_confront` zerado — nunca abortar com erro.
