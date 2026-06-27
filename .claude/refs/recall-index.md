# Recall recovery — find a method by what its BODY means when its NAME diverged

Loaded on demand when a scan builds the recall index, and when a `centralFound: false` miss needs recovery. **Single source of truth — reference this file, never copy the prose into a SKILL.**

## Why
The scan indexes declaration NAMES; it cannot know what a method DOES. When the user's query vocabulary diverges from the identifier (PT "efetivar" vs EN `EffectivateAsync`, or a synonym within one language), name-match never surfaces the right method — and no judge can re-rank what was never retrieved. Measured on real code: name-match recall **0/10** on cross-lingual holes. The meaning lives only in the body, so recall has to reach the body.

## At scan time — build the local recall index (no LLM, no per-method cost)
`/scan` builds a LOCAL embedding vector per logic method's body:

`mustard-embed build --model .claude/grain.model.json --embed-model code`

This writes `.claude/grain.vectors` — a compact binary sidecar, one vector per logic method, computed by a local code-specialised model (jina-code). NO LLM, no network, no per-method token cost; the cost is local compute, constant in repo size. INCREMENTAL: a re-scan reuses the stored vector of every unchanged method (matched by body hash) and re-embeds only new/changed ones. Fail-open: if `mustard-embed` is absent (headless / not installed), the step skips silently and the digest degrades to name-only.

## At query time — recover a miss
On a `centralFound: false` miss (the digest-validate judge reports the central concept was NOT found by name), the orchestrator runs:

`mustard-embed search --intent "<the missed concept, TRANSLATED TO ENGLISH>" --vectors .claude/grain.vectors`

It returns the files whose method BODIES mean the concept, ranked by similarity — exactly the methods whose NAME diverged from the request (field-measured: PT `efetivar`→`EffectivateAsync`, `dar baixa`→`WriteOffAsync`; name+embedding `combined@5 = 1.0` on medusa/saleor). It is RECALL ranked by relevance, **not precision** — on a large mixed repo the right file competes with interfaces, status/state services, and display components of similar vocabulary, so the target can sit at #2–#5. Each hit carries `method`+`line`: read those candidate spans (the top ~3-5) and pick the one that PERFORMS the action — this one-read re-rank IS the precision step, at ~zero cost. An empty result or an absent `mustard-embed` → fall through to the bridge re-query in [[digest-validate]].

## Superseded
The old recall flow is LEGACY, replaced by the local embedding index above — do NOT present it as current:
- the per-method **Sonnet `enrich-purpose --render` / `--apply`** that summarised every logic method's body into a one-sentence `purpose` (≈$50 of Sonnet once on a sialia-sized repo); and
- the lexical lookup **`mustard-rt run purpose-search`** over those `purpose` summaries.

The local embedding index gives the same cross-lingual recall (`combined@5 = 1.0`) with NO LLM and constant cost — it replaces both. See `docs/OTIMIZACAO-PURPOSE-ENRICH.md` for the measurement that retired the Sonnet flow.
