# (root)

## Guards

- `pull.ff = only` é escolha por-máquina em `.git/config`; NÃO nativize no instalador. O `base_gate` (`apps/rt/src/commands/event/base_gate.rs:149`) já prescreve `git pull --ff-only origin {base}` na recusa, e `--ff-only` só passa quando a base de integração não tem commit próprio — a invariante que o despacho exige. `pull.rebase true` absorveria em silêncio um commit nascido direto no `dev` e esconderia justamente o defeito que o portão existe para pegar. Reaplicação deliberada continua possível: `git pull --rebase` na linha de comando vence a config.
- O instalador NUNCA escreve em `.git/config` — só lê (`config --get remote.origin.url`, `apps/cli/src/commands/init.rs:726`). Toda escrita de config no código vive sob `#[cfg(test)]`; mantenha assim.
