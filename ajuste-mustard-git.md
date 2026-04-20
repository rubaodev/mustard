# Ajustar `/mustard:git` para lidar com worktree sujo, arquivos regenerados em runtime e submódulos com `.git` como arquivo

Revisar `commands/mustard/git/SKILL.md` (e implementação correlata) para resolver os seguintes problemas observados em uso real num monorepo com 5 submódulos.

## Problema 1 — Auto-stash inconsistente entre passos de merge

O passo `merge main` (dev → main) tem auto-stash chained no Bash. O passo `merge` (feature → dev) **não tem**. Resultado: qualquer worktree sujo aborta o sync com `error: Your local changes to the following files would be overwritten by checkout`.

Corrigir para que TODOS os passos de checkout (sync, merge, merge main) tenham auto-stash embutido no mesmo chain, com drop do stash apenas se ele foi criado pelo próprio skill (usar mensagem sentinel como `mustard-git-autostash-<action>-<ts>`).

## Problema 2 — Arquivos regenerados em runtime entre commit e checkout

Claude Code e RTK escrevem continuamente em `.claude/.agent-state/` e `.claude/.metrics/` **enquanto o skill roda**. Sequência observada: commit ok → push ok → entre push e `git checkout <target>`, Claude/RTK regravam esses paths → checkout aborta com "untracked working tree files would be overwritten" ou "local changes". O skill deve:

1. Detectar paths conhecidos como ephemeral do Claude/RTK:
   - `.claude/.agent-state/`
   - `.claude/.metrics/`
   - `.claude/.pipeline-states/`
   - `.claude/.detect-cache.json`
   - `.claude/.knowledge-seen.json`
2. Em cada repo operado, garantir que estão no `.gitignore` local OU em `.git/info/exclude` (sem commitar `.gitignore`, se preferir não tocar no repo).
3. Quando detectar que algum desses já está tracked (`git ls-files`), emitir warning e oferecer `git rm --cached` como sub-ação.

## Problema 3 — `.git` como arquivo em submódulos

No código do skill, qualquer referência a `.git/info/exclude` direta falha em submódulos porque `.git` é um arquivo apontador (`gitdir: ../../.git/modules/<name>`).

Usar SEMPRE `git rev-parse --git-path info/exclude` para resolver o caminho real antes de escrever.

## Problema 4 — Escopo ambíguo do action `commit`

Skill diz "commit dirty submodules" mas não define o que entra no commit (`git add -A` vs `git add <path>`).

Adicionar parâmetro explícito `--scope=all|staged|<path-pattern>` com default claro (sugestão: `all`) e documentar no SKILL.md. Parent repo pode querer commit seletivo de submodule pointer bumps sem tocar em trabalho em andamento.

## Problema 5 — Output não compactado em fast-forward

`git merge --ff-only` de centenas de arquivos emitiu >500KB de output (deletions/creations listados linha a linha), persistido em arquivo temp por exceder budget.

Adicionar `-q` ou redirecionar stat para contagem compacta:

```bash
git merge --ff-only -q && git --no-pager diff --stat HEAD@{1} HEAD | tail -3
```

## Problema 6 — Status final não reporta pendências residuais

Após `/git merge main` concluir, skill retorna tabela de sucesso mas não lista o que ficou no worktree.

Adicionar resumo final com `git status --short` por repo (parent + submódulos), categorizado em:

- (a) ephemeral ignorado (descartável)
- (b) código real pendente
- (c) untracked novo

## Problema 7 — Race entre stash e checkout (retry automático)

Mesmo com auto-stash correto, Claude Code e RTK podem escrever em `.claude/.agent-state/` e `.claude/.metrics/` **entre** o `git stash push` e o `git checkout <target>` (race de milissegundos).

Skill deve implementar retry automático: se `checkout` falhar com `"would be overwritten by checkout"` ou `"local changes would be overwritten"`, re-executar `git stash push -u -m <sentinel>` e tentar checkout novamente. Limite de 3 tentativas antes de abortar com erro descritivo apontando os paths culpados.

## Problema 8 — Ordem correta quando ephemeral já está tracked

Se o skill detectar que paths ephemeral (`.claude/.agent-state/*`, `.claude/.metrics/*`, etc.) já estão tracked via `git ls-files`, deve executar um **sub-fluxo preparatório** ANTES de qualquer `commit --scope=all`:

1. Append paths a `.gitignore` (ou `.git/info/exclude` se preferir não versionar)
2. `git rm --cached <paths>` para desvincular do tracking sem apagar do disco
3. Commit dedicado: `chore: ignore ephemeral <runtime> state`
4. Só então proceder com o commit principal solicitado pelo usuário

Sem isso, ephemerals são arrastados para dentro do commit "real" e contaminam o diff com ruído de runtime.

## Problema 9 — Preservação de stashes pré-existentes

Skill NUNCA deve executar `git stash pop` sem antes verificar que o stash alvo possui a mensagem sentinel criada pela própria execução atual (ex.: `mustard-git-autostash-<action>-<ts>`).

Cenário observado: repo já tinha `stash@{0}: fix: update Backend submodule ref (Libs submodule fix)` pré-existente do usuário. Um `git stash pop` ingênuo restauraria trabalho alheio por acidente.

Regra: `git stash list | grep -F "<sentinel-exata>"` → extrair índice → `git stash drop stash@{<N>}` ou `git stash pop stash@{<N>}` via índice específico, nunca stash@{0} implícito.

## Problema 10 — Política de default quando `--scope` não é passado

O action `commit` precisa de política explícita e documentada quando o usuário não passa `--scope`:

Sugestão: skill mostra `git status --short` categorizado, infere escopo provável (ex.: "só `.claude/*`" vs "tudo") e **pergunta uma única vez** com preview antes de decidir. Depois da primeira escolha na sessão, memoriza e não pergunta de novo para actions subsequentes.

Evita o cenário observado onde a interpretação ambígua de "commitar `.claude`" levou a 3 iterações (só `.claude/` → `.claude/` + `CLAUDE.md` → `git add -A`).

## Problema 11 — Apenas operações reversíveis no filesystem

Skill deve usar **exclusivamente** operações reversíveis em qualquer fluxo:

- `git rm --cached <path>` (preserva arquivo no disco)
- append em `.gitignore` ou `.git/info/exclude`
- `git stash push`/`git stash pop` (com sentinel)

NUNCA usar:

- `rm -f`, `rm -rf`
- `git clean -fd`
- `git checkout -f` ou `git reset --hard` para "desbloquear" checkout

Razão: ambientes com hooks de data safety (ex.: memórias/políticas do Claude Code) bloqueiam operações destrutivas filesystem e causam abortos abruptos no skill. Operações git-level são aceitas por serem reversíveis via reflog/stash.

## Casos de teste

1. Repo com `.claude/.agent-state/debug-loop-state.json` tracked + modified → skill deve ofertar untrack + gitignore, não apenas falhar em checkout.
2. 5 submódulos + parent, cada um com commits ahead em `feature` branch, worktree sujo em todos → `/git merge main` deve propagar até `main` em todos sem intervenção manual.
3. Submódulo em estado detached HEAD vs em branch → skill deve reportar claramente antes de tentar merge.
4. Parent com pointer bump pendente + worktree sujo não relacionado → `commit` com `--scope=apps/sialia-admin` deve commitar só o pointer.
5. Repo com stash pré-existente do usuário (`stash@{0}: fix: ...`) + auto-stash criado pelo skill → ao final, skill deve popar APENAS seu stash via índice correto, preservando o do usuário intacto.
6. Race condition: durante execução, processo externo modifica `.claude/.agent-state/*` entre stash e checkout → skill deve retry até 3x, depois abortar com mensagem clara.
7. Ambiente com hook de data safety que bloqueia `rm -f` → skill deve completar fluxo usando apenas `git rm --cached` + gitignore, sem tentar deletar filesystem.
8. Ephemeral tracked + worktree com código real dirty + commit sem `--scope` → skill deve mostrar preview `git status --short`, perguntar escopo uma vez, executar sub-fluxo de untrack ephemeral ANTES do commit principal.

## Arquivos a revisar

- `commands/mustard/git/SKILL.md`
- Implementação do action dispatcher do skill (se houver código JS/hook associado)
- Testes existentes de git skill, se existirem
