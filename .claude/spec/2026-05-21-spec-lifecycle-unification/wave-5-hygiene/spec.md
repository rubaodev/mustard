# Wave 5 — Hygiene: spec_hygiene hook + auto-close protegido por gate

### Parent: [[2026-05-21-spec-lifecycle-unification]]
### Wave: 5
### Role: rt
### Status: approved
### Phase: PLAN
### Lang: pt
### Checkpoint: 2026-05-21T00:00:00Z

## Resumo

Adiciona o hook `spec_hygiene` ao `mustard-rt` que roda em `SessionStart`. Para cada spec ativa, ele determina se a spec está em estado "candidata a fechar" e dispara auto-close **somente se** o close-gate (build/lint/test/QA) passa. Emite eventos `hygiene.*` para auditabilidade (renderizados pelo dashboard em Wave 6).

## Arquivos

```
apps/rt/src/hooks/spec_hygiene.rs               (novo — hook principal)
apps/rt/src/hooks/mod.rs                        (registrar)
apps/rt/src/dispatch.rs                         (wire ao SessionStart event)
apps/rt/src/lib.rs                              (expose módulo)
apps/rt/src/run/emit_pipeline.rs                (aceitar kinds hygiene.* — adicionar a KNOWN_KINDS)
apps/rt/tests/spec_hygiene.rs                   (novo — cenários)
apps/rt/tests/hygiene_event_kinds.rs            (novo — KNOWN_KINDS)
```

## Tarefas

### Algoritmo de detecção

- [ ] Hook roda **antes** do `session_start` injection (ordem do dispatcher).
- [ ] Para cada spec em `.claude/spec/` cujo `SpecState` tem `outcome == Active`:
  1. Lê AC do spec.md: se algum AC não tem `[x]` → status = `incomplete`, continue.
  2. Lê `git log --oneline -1 -- "{spec_dir}"` para o último commit que tocou a spec.
  3. Lê SQLite: timestamp do último evento de qualquer kind para essa spec.
  4. Determina categoria:
     - **candidate**: todos AC `[x]` + commit recente (≤72h) toca arquivos da spec + último evento da spec há ≥6h.
     - **stale**: todos AC `[x]` + último evento há ≥72h (sem commit recente).
     - **abandoned-suspect**: AC parciais + último evento há ≥30 dias.
     - **healthy**: qualquer outra coisa (não age).
- [ ] Para `candidate`, executar o close-gate:
  - `verify-pipeline` (build/lint/test).
  - QA: re-rodar AC se possível (idempotente — `mustard-rt run qa-run --spec NAME`).
  - Se tudo verde ⇒ emit `hygiene.autoclose` + `pipeline.outcome: completed` + reescreve header da spec para `Outcome: Completed`.
  - Se algo vermelho ⇒ emit `hygiene.skipped` com `blocker: build_red | ac_failing | qa_missing`.
- [ ] Para `stale`, emit `hygiene.detected { reason: "stale" }`. Não age — só sinaliza.
- [ ] Para `abandoned-suspect`, emit `hygiene.detected { reason: "abandoned_suspect" }`. Não age.

### Eventos novos

Adicionar ao `KNOWN_KINDS` em `emit_pipeline.rs`:

```
hygiene.detected
hygiene.autoclose
hygiene.skipped
```

Payload de cada:

```json
hygiene.detected   { "spec": "...", "reason": "stale|abandoned_suspect|candidate", "evidence": { "ac_pct": 1.0, "last_event_at": "...", "last_commit_at": "..." } }
hygiene.autoclose  { "spec": "...", "gate_result": { "build": "pass", "qa": "pass" }, "emitted_at": "..." }
hygiene.skipped    { "spec": "...", "blocker": "build_red|ac_failing|qa_missing", "details": "..." }
```

### Configuração

- [ ] Hook respeita env var `MUSTARD_HYGIENE_MODE`:
  - `off`: hook desativado (volta a comportamento atual).
  - `detect`: só emite `hygiene.detected` para todas as categorias; NUNCA auto-fecha.
  - `auto` (default): comportamento descrito acima.

### Testes

- [ ] `tests/spec_hygiene.rs`:
  - Cenário 1: spec com todos AC `[x]`, commit há 4h, último evento há 8h, build verde, QA pass ⇒ hygiene emite `hygiene.autoclose` + `pipeline.outcome: completed`.
  - Cenário 2: idem cenário 1 mas build vermelho ⇒ emite `hygiene.skipped` com `blocker: build_red`, **não** fecha.
  - Cenário 3: spec com AC parcial, último evento há 60 dias ⇒ `hygiene.detected { reason: "abandoned_suspect" }`.
  - Cenário 4: `MUSTARD_HYGIENE_MODE=off` ⇒ hook não emite nada.
  - Cenário 5: idempotência — hook rodar 2x na mesma spec já fechada não emite eventos duplicados.

### Build/Lint

- [ ] `cargo build -p mustard-rt && cargo test -p mustard-rt && cargo clippy -p mustard-rt -- -D warnings`.

## Acceptance Criteria

- [ ] AC-W5-1: `cargo build -p mustard-rt` passa.
- [ ] AC-W5-2: `cargo test -p mustard-rt` passa (incluindo todos os 5 cenários do `tests/spec_hygiene.rs`).
- [ ] AC-W5-3: `cargo clippy -p mustard-rt -- -D warnings` passa.
- [ ] AC-W5-4: Rodando o hook manualmente (`mustard-rt run hooks-test --hook spec_hygiene --event session_start`) contra `.claude/spec/2026-05-21-tf-skill-mirror/` (AC todas `[x]`, commit `abb5b63` há horas) emite `hygiene.autoclose` + altera o header para `Outcome: Completed`. Specs já fechadas não geram evento.
- [ ] AC-W5-5: `mustard-rt run event-projections --view session-summary` lista eventos `hygiene.*` recentes.

## Limites

**IN:** apenas os arquivos listados.

**OUT:**
- Dashboard / UI dos eventos hygiene — Wave 6.
- Header das outras specs em `.claude/spec/` — Wave 7 migra em batch.

## Notas de segurança

- O auto-close **nunca** roda sem close-gate. Se `verify-pipeline` falha, hygiene não escreve o evento `pipeline.outcome: completed`. Spec quebrada não fecha.
- Audit-log via SQLite: `hygiene.autoclose` registra o gate_result que foi verde no momento — auditável depois.
- Reversibilidade: o usuário pode emitir `pipeline.flag.set blocked` numa spec fechada por engano; ela volta para Active e re-aparece em "Ativas". Não há "undo automático".
