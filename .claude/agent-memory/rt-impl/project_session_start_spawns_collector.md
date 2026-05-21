---
name: session-start-spawns-otel-collector
description: SessionStart now spawns mustard-rt run otel-collector detached and writes PID file; legacy out-of-scope note in session_start.rs was reversed
metadata:
  type: project
---

Desde 2026-05-21 (W3b da economia-moat-unification), `apps/rt/src/hooks/session_start.rs::SessionStart::evaluate` chama `spawn_otel_collector(&cwd)` que faz `Command::new(env::current_exe()?).args(["run","otel-collector"]).spawn()`. O PID vai em `<project>/.claude/.harness/.otel-collector.pid`. Idempotência via `is_process_alive(pid)` (kill -0 em Unix, `tasklist /FI` em Windows — sem `unsafe`, sem `windows-sys`).

`spawn_transcript_watcher()` é opt-in (`MUSTARD_TRANSCRIPT_WATCH=1`).

`session_cleanup.rs::clean_otel_pid` continua removendo o PID file no SessionEnd — agora não é "legacy cleanup", é parte ativa do contrato de idempotência (limpa o PID stale para o próximo SessionStart respawnar limpo).

**Why:** O módulo originalmente declarava o spawn como "out of scope (B4 script dep)" — comentário foi atualizado. Com o b4 port completo (`mustard-rt run otel-collector` existe), não há mais razão pra não spawnar.

**How to apply:** Se quiser desabilitar o spawn pra debug, não use env var nova — extenda `is_process_alive` ou adicione um early-return condicional. Manter o spawn como side-effect padrão do SessionStart é o contrato.
