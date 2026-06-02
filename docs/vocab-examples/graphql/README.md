# GraphQL vocabulary example

A curated **example** framework vocabulary that teaches mustard's local-first
scan to recognise GraphQL (NestJS code-first + Apollo / graphql-js / async-graphql).
It is **data you opt into** — the mustard core stays generic and ships no GraphQL
knowledge of its own.

Everything here runs **fully local**: classification reads your build manifest.
**No network, no LLM.**

## How the registry label is computed (MANIFEST-DRIVEN)

The registry's `_patterns.{stack}.frameworks` label is the classification of a
subproject's **DECLARED dependencies** — read from its build manifest
(`Cargo.toml`, `package.json`, `*.csproj`, `go.mod`, `pyproject.toml`) — through
the `[[dependency]]` rules in this vocab. It is **not** a substring scan of your
source code: a project that merely *mentions* a GraphQL token in a comment or a
string carries **no** label; only a **declared dependency** does. This removes
the false positive that source-scanning produced for framework-detection
software (whose own source lists framework tokens as data).

A declared dependency the vocab does **not** map is recorded under
`_patterns.{stack}.unclassifiedDependencies` — a gap surfaced for the future
web-fetch rung, never fabricated into a category.

## How to use

Copy the example into your project's vocab directory:

```sh
mkdir -p <your-project>/.claude/vocab
cp docs/vocab-examples/graphql/frameworks.toml <your-project>/.claude/vocab/frameworks.toml
```

Then re-run the scan to rebuild the repo model, e.g.:

```sh
mustard-rt run scan
```

## Two schemas in one file

- `[[dependency]]` — the **primary** schema that drives the registry label.
  `category` (one of `orm` / `framework` / `di`), `names` (exact dependency
  names), `prefixes` (name prefixes / namespaces, e.g. `@nestjs/graphql`).
- `[[signal]]` — the legacy literal code-pattern schema, retained for the
  structural extractor's **decorator tagging** only (NOT the registry label).

`mustard init` seeds a cross-ecosystem `frameworks.toml` at
`.claude/vocab/frameworks.toml`; extend it (or replace it with this example) to
teach the scan your stack. When the file is **absent**, classification is empty
(correct, not an error) — the binary bakes no framework knowledge.

## What it produces

When a scanned subproject DECLARES a dependency this vocab maps, the union of
classified **categories** lands in the registry under
`_patterns.{stack}.frameworks` (a JSON array, written only when non-empty).
Declaring `async-graphql` (mapped to `framework`) yields:

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

The `frameworks` array carries the **category** (`orm` / `framework` / `di` —
the closed `FrameworkCategory` enum), **not** the specific framework **name**.
So a GraphQL dependency shows up as `"framework"`, indistinguishable from an
axum or Spring dependency. There is no `"graphql"` label today.

## Files

- `frameworks.toml` — the curated GraphQL vocab (`[[dependency]]` rules + the
  legacy `[[signal]]` decorator patterns).
- `README.md` — this file.

The integration test `apps/rt/tests/scan_vocab_override.rs` validates *this
shipped file* end-to-end and offline: it copies `frameworks.toml` into a temp
project that **declares** `async-graphql` in `Cargo.toml`, runs
`mustard-rt run scan`, and asserts the model's `rust` frameworks contain
`"framework"` WITH the vocab and is absent WITHOUT it.
