# Rt

> Parent: [../../CLAUDE.md](../../CLAUDE.md) | Orchestrator: [../../.claude/CLAUDE.md](../../.claude/CLAUDE.md)

<!-- mustard:scan-map -->
Tipo: cargo · 209 arquivos
Pesquise via `mustard-rt run feature` (digest) — não leia o repo direto.
<!-- /mustard:scan-map -->

## Guards

<!-- mustard:guards -->
<!-- facts: kind=cargo; frameworks=mustard-core, serde, serde_json, clap, tiny_http, mustard-mcp, ureq, tempfile, notify, rayon, sha2 -->
- Hook nunca pode entrar em pânico nem barrar a sessão por erro próprio: a degradação mora UMA vez no dispatcher (um `Check` com `Err` vira `Allow`) e no `main` (todo caminho termina em `process::exit(0)`); bloqueio se expressa no JSON `permissionDecision`, jamais via exit não-zero.
- `clippy::unwrap_used`/`expect_used` são `deny` em todo o crate (fora de `#[cfg(test)]`); em hook, degrade com `unwrap_or` / `let-else` / `ok()?`.
- Subcomando novo de `run` exige DOIS registros: a variante no enum `RunCmd` (commands/mod.rs) E o braço em `dispatch()`; esquecer o segundo compila mas o comando some.
- Observers (`hooks/observe`,`session`,`task`) são só efeito-colateral/telemetria: `observe()` retorna `()` e roda fire-and-forget — nunca devolva veredito por ali (decisão de bloqueio vive num `Check`).
- As faces `run` e `mcp` NÃO leem o stdin do harness (despachadas antes da leitura no `main.rs`); só `on`/`check` consomem o `HookInput`. Mantenha `main.rs` magro: roteamento de argv + match das faces, sem lógica de negócio.
- Saída de comando `run` deve ser determinística e byte-estável (JSON ordenado, sem timestamps/caminhos voláteis) — há snapshots `insta` e gates que comparam a saída.
<!-- /mustard:guards -->
