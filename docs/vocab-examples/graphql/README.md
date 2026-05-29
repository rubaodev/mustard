# GraphQL vocabulary example

A curated **example** framework vocabulary that teaches mustard's local-first
scan to recognise GraphQL code (NestJS code-first + Apollo / graphql-js). It is
**data you opt into** — the mustard core stays generic and ships no GraphQL
knowledge of its own.

Everything here runs **fully local**: framework detection is a single
Aho-Corasick pass over your source files. **No network, no LLM.**

## How to use

Copy the example into your project's vocab directory:

```sh
mkdir -p <your-project>/.claude/vocab
cp docs/vocab-examples/graphql/frameworks.toml <your-project>/.claude/vocab/frameworks.toml
```

Then run a scan that rebuilds the registry, e.g.:

```sh
mustard-rt run sync-registry --force
```

## IMPORTANT: `load` replaces the built-in base wholesale

`FrameworkVocabulary::load` does **not** merge — when
`.claude/vocab/frameworks.toml` exists it **replaces the built-in base
term-for-term**. Dropping *only* the GraphQL file makes the scanner forget the
built-in ORM / web / DI signals (Drizzle `pgTable(`, `axum::`, `FastAPI(`,
`@Injectable`, …).

To keep the built-in signals **and** add GraphQL, append the built-in
`[[signal]]` entries to your copy:

```sh
cp docs/vocab-examples/graphql/frameworks.toml <your-project>/.claude/vocab/frameworks.toml
# Append the built-in base so its orm/framework/di signals survive the replace.
cat packages/core/src/domain/vocabulary/frameworks_builtin.toml >> <your-project>/.claude/vocab/frameworks.toml
```

The result is one TOML file with the GraphQL `framework` entry **plus** the
three built-in `orm` / `framework` / `di` entries. Categories are independent
table-array entries, so having two `category = "framework"` entries is fine —
their patterns are unioned.

## What it produces

When a scanned subproject contains any of the GraphQL signals, the union of
fired **categories** lands in the registry under
`_patterns.{stack}.frameworks` (a JSON array, written only when non-empty).
Because every GraphQL pattern is `category = "framework"`, you get:

```json
"_patterns": {
  "rust": {
    "frameworks": ["framework"]
  }
}
```

`guards-seed` then reads that array and a `## Frameworks detected` section is
added to the generated `guards.md` for the stack.

## KNOWN LIMITATION — labels are categories, not framework names

The `frameworks` array carries the signal **category** (`orm` / `framework` /
`di` — the closed `FrameworkCategory` enum), **not** the specific framework
**name**. So a GraphQL match shows up as `"framework"`, indistinguishable from
an axum or Spring match. There is no `"graphql"` label today.

A future vocabulary enhancement could carry a per-signal name alongside the
category so the registry can report `"graphql"` specifically. Until then, treat
the category as the contract.

## Files

- `frameworks.toml` — the curated GraphQL vocab (all `category = "framework"`).
- `README.md` — this file.

The integration test `apps/rt/tests/scan_vocab_override.rs` validates *this
shipped file* end-to-end and offline: it copies `frameworks.toml` into a temp
project, runs `sync-registry --force`, and asserts `_patterns.rust.frameworks`
contains `"framework"` WITH the vocab and is absent WITHOUT it.
