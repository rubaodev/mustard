# W3 — Dashboard + doctor check + contrato canônico em CLAUDE.md

### Stage: Execute
### Outcome: Active
### Flags:
### Checkpoint: 2026-05-26T00:00:00Z

## Contexto

Fecha o ciclo: dashboard agora lê dos paths novos (struct), `doctor` ganha capacidade nova para auditar filesystem contra catálogo, e o contrato narrativo em `apps/cli/templates/CLAUDE.md` passa a apontar para `ClaudePaths` como fonte canônica em vez de descrever a árvore em prosa.

## Tarefas

- [ ] **T3.1** — Migrar `apps/dashboard/src-tauri/src/watcher.rs`. Hoje observa `.claude/.harness/`, `.claude/spec/`, etc. via strings literais; passa a usar `ClaudePaths::for_project(root).harness_dir()` e `.spec_dir()`. Atenção: watcher recursivo precisa observar `spec/{name}/qa-report.json` agora (novo path); confirmar que glob recursivo já cobre.

- [ ] **T3.2** — Migrar `apps/dashboard/src-tauri/src/db.rs`. Leitor de `.claude/.harness/mustard.db`. Substituir por `ClaudePaths::for_project(root).harness_dir().join("mustard.db")` (ou criar acessor `mustard_db_path()` na struct se W1 não previu).

- [ ] **T3.3** — Migrar `apps/dashboard/src/data/commands-catalog.ts`. Hoje cataloga paths em strings TS. Passar a importar de um arquivo JSON gerado pelo `mustard-rt run claude-paths --format json` (build-time), ou hardcoded apontando para os novos caminhos canônicos. Decisão tática: **hardcoded TS apontando para os novos caminhos** — gerador automático foge ao escopo da spec.

- [ ] **T3.4** — Implementar `mustard-rt run doctor --check claude-paths --format json`. Novo módulo `apps/rt/src/run/doctor_claude_paths.rs` (ou estender `doctor.rs` se for pequeno). Compara:
  - `ClaudePaths::documented_dirs()` × `fs::read_dir(".claude")` → `unexpected_dirs` se filesystem tem dir não catalogado, `missing_dirs` se catálogo tem mas filesystem não (mas dir não é obrigatório, então só WARN).
  - `ClaudePaths::cache_files()` dentro de `cache_dir()` × filesystem → mesma lógica.
  - Output: `{ok: bool, divergences: [{path, kind: "unexpected"|"missing", severity: "warn"|"error"}]}`.

- [ ] **T3.8** — Implementar `mustard-rt run doctor --check workspace-leaks --format json`. Mesmo módulo ou irmão. Walk no workspace listando todo `.claude/` que **não** seja o da raiz. Para cada um, classifica:
  - **OK** se contém apenas: `commands/`, `skills/`, `agents/`, `services.json`, `refs/`, `recipes/`, `CLAUDE.md`, `.cluster-cache.json`, `.interpret-cache.json` (output legítimo do scan)
  - **Vazamento** (`severity: "warn"`) se contém qualquer outro tipo: `.harness/`, `.agent-state/`, `.pipeline-states/`, `memory/`, `plans/`, `.metrics/`, `spec/`, `.agent-memory/`
  - Não auto-deleta; reporta path + comando de limpeza sugerido.

- [ ] **T3.9** — Implementar `mustard-rt run doctor --check i1 --format json`. Grep no filesystem por **qualquer** `.claude/.claude/` aninhado em qualquer lugar do workspace. Reporta como **erro crítico** (`severity: "error"`, exit code != 0): presença física é evidência de bug ativo, não estado tolerável. Output: `{ok: bool, violations: [PathBuf]}`. Honra `MUSTARD_WORKSPACE_ROOT` quando setada.

- [ ] **T3.10** — Default `mustard-rt run doctor` (sem `--check`) executa todos os checks (`claude-paths`, `workspace-leaks`, `i1`) e agrega o output. Exit code != 0 se qualquer check com `severity: "error"` falhar. Mantém retrocompat com flags individuais.

- [ ] **T3.5** — Reescrever seção em `apps/cli/templates/CLAUDE.md`. Hoje (do contrato canônico fechado em W2.T2.4 da [[2026-05-25-mustard-deep-refactor]]) diz: "todo path em `.claude/` deve ter consumidor declarado em pelo menos uma das três subprojects, exceto caches `.X.json` e diretórios documentados (`worktrees/`, `.pipeline-states/`, `.qa-reports/`)". Passa a dizer: "Todos os paths sob `.claude/` são definidos em `mustard_core::ClaudePaths` (`packages/core/src/claude_paths.rs`). Para adicionar um novo subdir ou cache, adicione método na struct e teste. `mustard-rt run doctor --check claude-paths` valida o filesystem contra o catálogo."

- [ ] **T3.6** — Atualizar memória [[feedback_claude_dir_audit]] (em `C:/Users/ruben/.claude/projects/c--Atiz-mustard/memory/`) com referência ao novo módulo. Adicionar linha sobre `doctor --check claude-paths`.

- [ ] **T3.7** — Estender wave-12-mixed da [[2026-05-25-mustard-deep-refactor]] **somente** se backup de rollback for necessário. Decisão tática: **não estender**; spec mãe está Completed; rollback desta spec é `git revert` dos commits dela.

## Critérios de Aceitação

- [ ] **AC-W3.1** — Dashboard compila. Command: `rtk pnpm --filter mustard-dashboard build`
- [ ] **AC-W3.2** — Dashboard backend compila. Command: `rtk cargo build -p mustard-dashboard`
- [ ] **AC-W3.3** — `doctor --check claude-paths` registrado. Command: `rtk mustard-rt run doctor --help 2>&1 | rtk node -e "let s='';process.stdin.on('data',c=>s+=c);process.stdin.on('end',()=>{if(!s.includes('claude-paths'))process.exit(1)})"`
- [ ] **AC-W3.4** — `doctor --check claude-paths --format json` retorna `{ok: true, divergences: []}` no projeto Mustard pós-migração. Command: `rtk mustard-rt run doctor --check claude-paths --format json | rtk node -e "let s='';process.stdin.on('data',c=>s+=c);process.stdin.on('end',()=>{const j=JSON.parse(s);if(!j.ok||j.divergences.length>0)process.exit(1)})"`
- [ ] **AC-W3.5** — `CLAUDE.md` template referencia `ClaudePaths`. Command: `rtk node -e "const t=require('fs').readFileSync('apps/cli/templates/CLAUDE.md','utf8');if(!/ClaudePaths|claude_paths\\.rs/.test(t))process.exit(1)"`
- [ ] **AC-W3.6** — Zero literais de path em `apps/dashboard/src-tauri/src/` fora de chamadas para `ClaudePaths`. Command: `rtk node -e "const{execSync}=require('child_process');const out=execSync('rtk grep -rn --include=\"*.rs\" \"\\\\.claude\" apps/dashboard/src-tauri/src',{encoding:'utf8'});const violations=out.split('\\n').filter(l=>l&&!/ClaudePaths|test/.test(l));if(violations.length>0){console.error(violations.join('\\n'));process.exit(1)}"`
- [ ] **AC-W3.7** — `doctor --check workspace-leaks` registrado. Command: `rtk mustard-rt run doctor --help 2>&1 | rtk node -e "let s='';process.stdin.on('data',c=>s+=c);process.stdin.on('end',()=>{if(!s.includes('workspace-leaks'))process.exit(1)})"`
- [ ] **AC-W3.8** — `doctor --check i1` registrado e falha com exit code != 0 quando `.claude/.claude/` existe. Command: `rtk powershell -Command "$tmp = New-Item -Type Directory ([System.IO.Path]::GetTempPath() + [System.Guid]::NewGuid().ToString()); New-Item -Type Directory ($tmp.FullName + '/.claude/.claude') | Out-Null; '{}' | Out-File ($tmp.FullName + '/mustard.json') -Encoding utf8; $env:MUSTARD_WORKSPACE_ROOT = $tmp.FullName; mustard-rt run doctor --check i1 2>&1 | Out-Null; $ec = $LASTEXITCODE; $env:MUSTARD_WORKSPACE_ROOT = $null; Remove-Item -Recurse -Force $tmp; if ($ec -eq 0) { exit 1 }"`
- [ ] **AC-W3.9** — `doctor` (sem flag) agrega todos os checks. Command: `rtk mustard-rt run doctor --format json | rtk node -e "let s='';process.stdin.on('data',c=>s+=c);process.stdin.on('end',()=>{const j=JSON.parse(s);if(!('claude_paths' in j)||!('workspace_leaks' in j)||!('i1' in j))process.exit(1)})"`

## Limites

`apps/dashboard/src-tauri/src/watcher.rs`, `apps/dashboard/src-tauri/src/db.rs`, `apps/dashboard/src/data/commands-catalog.ts`, `apps/rt/src/run/doctor.rs` (+ possíveis `doctor_claude_paths.rs`, `doctor_workspace_leaks.rs`, `doctor_i1.rs` novos), `apps/cli/templates/CLAUDE.md`, memória `C:/Users/ruben/.claude/projects/c--Atiz-mustard/memory/feedback_claude_dir_audit.md` (criar se não existir).

OUT: tudo mais.

## Role

mixed (dashboard + rt + cli template + memória global)
