---
name: otel-collector-v1-traces-route
description: The collector now serves /v1/traces alongside /v1/metrics and /v1/logs, routing into the economy spans table via sources::otel::ingest
metadata:
  type: project
---

`apps/rt/src/run/otel/collector.rs` aceita 3 rotas POST desde W3b (2026-05-21): `/v1/metrics`, `/v1/logs`, **e `/v1/traces`**.

**Why:** O `Store` antigo (`claude_code_otel`) só conhecia metrics/logs. A W1 economia introduziu a tabela `spans` unificada. A W3 conecta os 3 sources (OTEL, transcript, RTK) nela. O endpoint `/v1/traces` é onde o exporter OTLP nativo do Claude Code envia spans `gen_ai.*` com token usage — antes da W3 esses spans eram silenciosamente 404'd. Agora `project_traces_into_economy` reserializa o body já parseado em string e delega para `mustard_core::economy::sources::otel::ingest`, depois faz fan-out via `writer::record_span`.

**How to apply:** Se adicionar mais rotas (`/v1/profiles`, etc.), seguir o mesmo pattern: novo branch em `project_into_store`, helper dedicado para fan-out via writer, canary log a cada falha. Não tocar em `/v1/metrics` e `/v1/logs` — essas continuam alimentando `claude_code_otel` (consumidor: `metrics` subcommand legado).
