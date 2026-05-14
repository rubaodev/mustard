# Mustard 2.0 — Runtime (Bun-only)

> **Em português simples (1 parágrafo):** A partir do Mustard 2.0, **Bun é obrigatório**. Não tem fallback pra Node — `mustard init` aborta com instruções de instalação se Bun não estiver no PATH. Hooks, scripts, CLI e testes rodam todos em Bun. Cold-start dos hooks cai de ~150ms (Node) para ~20ms (Bun), e o event store usa `bun:sqlite` nativo (sem npm deps). Claude Code v2.1.113+ já vem com Bun embutido — pra usuários fora do Claude Code basta `scoop install bun` (Windows) ou `curl -fsSL https://bun.sh/install | bash` (Unix).

## Status

- **Mustard 2.0+**: Bun-only.
- **Runtime mínimo**: Bun 1.2.0
- **Sem fallback**: a CLI, hooks, scripts e testes não suportam Node.

## Por que Bun

- **Cold-start ~10× mais rápido** que Node em hooks (cada chamada de tool spawn 1 processo).
- **`bun:sqlite` nativo** — event store sem dependência externa (Phase 1).
- **TypeScript nativo** — `src/` pode rodar `.ts` direto sem build step.
- **Test runner integrado** — `bun test` substitui `node --test`.

## Detecção e enforcement

`mustard init` e `mustard update` checam Bun via:

1. `typeof Bun !== 'undefined'` ou `process.versions.bun`
2. Se falhar, exit 1 com instruções de instalação.

Em hooks/scripts gerados, o mesmo check roda no shim `templates/hooks/_lib/runtime-shim.js` (chamado por `event-store.js` etc.).

```ts
{
  kind: 'bun',
  version: string,
  claudeCodeVersion?: string
}
```

## Instalando Bun

### Windows

```powershell
# Opção 1: Scoop
scoop install bun

# Opção 2: PowerShell (oficial)
powershell -c "irm bun.sh/install.ps1 | iex"
```

Bun 1.2+ tem suporte production-grade no Windows.

### Linux / macOS

```bash
curl -fsSL https://bun.sh/install | bash
```

### Verificar instalação

```bash
bun --version
# Esperado: 1.2.x ou superior
```

## Onde Bun é usado

| Camada | Onde | Como |
|---|---|---|
| CLI | `bin/mustard.js` | shebang `#!/usr/bin/env bun` + check explícito |
| Hooks | `templates/hooks/*.js` | shebang `#!/usr/bin/env bun`; `settings.json` invoca `bun "$CLAUDE_PROJECT_DIR"/.claude/hooks/X.js` |
| Scripts | `templates/scripts/**/*.js` | shebang `#!/usr/bin/env bun`; settings allow-list usa `Bash(bun .claude/scripts/...)` |
| Event store | `templates/hooks/_lib/event-store.js` | `bun:sqlite` (WAL + FTS5) |
| MCP server | `dist/mcp/mustard-memory.js` | `settings.json` invoca via `bun` |
| Testes | `tests/**/*.test.{cjs,js}` | `bun test` |
| CI | `.github/workflows/ci.yml` | `oven-sh/setup-bun@v2` (Node não é mais instalado) |

## Persistência

`mustard init` grava em `.claude/mustard.json`:

```jsonc
{
  "runtime": {
    "kind": "bun",
    "version": "1.2.18",
    "chosenAt": "2026-05-12T18:10:00Z"
  }
}
```

- `mustard init` grava o `runtime` com base na detecção atual.
- `mustard update` **preserva** o campo `runtime`.
- O `chosenAt` ajuda a auditar quando o projeto adotou Bun.

### Dois arquivos `mustard.json`

| Arquivo | Conteúdo |
|---|---|
| `./mustard.json` (root) | Git-flow legacy (`branches`, `parent`, etc.) — usado por `close-gate.js`, `review-gate.js` |
| `./.claude/mustard.json` | Runtime info (`{ kind, version, chosenAt }`) |

Os dois são independentes.

## Troubleshooting

### "mustard: Bun runtime required (>= 1.2.0)"

A CLI foi invocada com Node ou sem Bun no PATH. Instale Bun (veja seção acima) e re-execute.

### "bun: command not found" mas eu instalei

Em sessões shell pré-existentes o `PATH` pode estar desatualizado. Abra um novo terminal ou recarregue o profile (`. $PROFILE` no PowerShell, `source ~/.bashrc` em Unix).

### "Logs de detecção"

```bash
MUSTARD_RUNTIME_VERBOSE=1 mustard init
# stderr: [mustard:runtime] kind=bun version=1.2.18
```

### Claude Code v2.1.113+

A partir dessa versão, Bun já vem embutido no Claude Code — o usuário final não precisa instalar nada. Hooks/MCP rodam direto.

## Histórico

- **Phase 0 (Mustard 2.0)**: layer de detecção com fallback Node — *deprecated*.
- **Phase 1+**: `bun:sqlite` no event store.
- **Phase 5 (atual)**: **Bun obrigatório**. Removido fallback Node, `--runtime` flag, `MUSTARD_RUNTIME=node` override.
