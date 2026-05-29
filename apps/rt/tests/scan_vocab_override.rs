// Integration tests are separate binary targets and not exempt from
// `clippy::unwrap_used` etc. via `#[cfg(test)]`. Mirror the carve-out from
// `src/main.rs` so test panics on `.unwrap()` remain valid assertions.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::map_unwrap_or,
    clippy::uninlined_format_args
)]

//! End-to-end proof that the GraphQL local-vocab EXAMPLE specialises mustard's
//! local-first scan — fully offline, no `claude`, no LLM, no network.
//!
//! The shipped example `docs/vocab-examples/graphql/frameworks.toml` is the
//! *data the user opts into*; the core stays generic. These tests VALIDATE THE
//! SHIPPED FILE (read via `CARGO_MANIFEST_DIR`, not an inline copy):
//!
//! - **WITH the vocab**: a temp Rust project whose source contains GraphQL
//!   tokens (`@Resolver`, a `gql` tagged template) + the example dropped at
//!   `.claude/vocab/frameworks.toml`. After `sync-registry --force`,
//!   `_patterns.rust.frameworks` must contain `"framework"`.
//! - **WITHOUT the vocab (baseline)**: the SAME fixture minus the vocab file.
//!   The built-in base does not know GraphQL tokens, so
//!   `_patterns.rust.frameworks` must be absent or empty.
//!
//! Harness mirrors `scan_cold_path.rs`: locate the binary via
//! `env!("CARGO_BIN_EXE_mustard-rt")`, build a temp fixture, run with an
//! isolated PATH so no real `claude` is ever spawned, and assert on the written
//! `.claude/entity-registry.json`.

use std::path::Path;
use std::process::Command;

/// The GraphQL fixture source. Contains two GraphQL signals — a NestJS
/// `@Resolver` decorator and a `gql` tagged template — and DELIBERATELY no
/// built-in signal (no `@Injectable`, `@Entity`, `pgTable(`, `axum::`, …), so
/// the baseline (built-in base) case stays empty.
const GRAPHQL_SOURCE: &str = "\
// A GraphQL resolver written in a .rs file purely so detect_stack = rust.
// The framework detector is language-agnostic: it scans content for literal
// signals, not grammar, so these tokens fire regardless of the host language.
@Resolver
pub fn users_resolver() {}

const QUERY = gql`query { users { id } }`;
";

/// Absolute path to the SHIPPED example vocab, derived from this crate's
/// manifest dir (`apps/rt`) up to the repo root. Reading the shipped file
/// (rather than inlining a copy) is what makes the test validate the artifact
/// the docs ship.
fn shipped_vocab_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("docs")
        .join("vocab-examples")
        .join("graphql")
        .join("frameworks.toml")
}

/// Build the temp project: `mustard.json` + `.claude/` + a `Cargo.toml` (so
/// `detect_stack` = rust) + `src/foo.rs` carrying the GraphQL tokens. When
/// `with_vocab` is true, the shipped example is copied to
/// `.claude/vocab/frameworks.toml`.
fn setup_project(project: &Path, with_vocab: bool) {
    std::fs::create_dir_all(project.join(".claude")).expect("create .claude");
    std::fs::create_dir_all(project.join("src")).expect("create src");
    // A minimal mustard.json — present so the fixture mirrors a real project
    // root (no architecture pin; framework detection is independent of it).
    std::fs::write(project.join("mustard.json"), "{}\n").expect("write mustard.json");
    // Cargo.toml ⇒ detect_stack = "rust" ⇒ stack key is `rust`.
    std::fs::write(
        project.join("Cargo.toml"),
        "[package]\nname = \"graphql-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    std::fs::write(project.join("src/foo.rs"), GRAPHQL_SOURCE).expect("write src/foo.rs");

    if with_vocab {
        let vocab_dir = project.join(".claude").join("vocab");
        std::fs::create_dir_all(&vocab_dir).expect("create .claude/vocab");
        let shipped = shipped_vocab_path();
        let toml = std::fs::read_to_string(&shipped).unwrap_or_else(|e| {
            panic!(
                "read shipped example vocab {}: {e}",
                shipped.display()
            )
        });
        std::fs::write(vocab_dir.join("frameworks.toml"), toml)
            .expect("write .claude/vocab/frameworks.toml");
    }
}

/// Run `mustard-rt run sync-registry --force` in `project`, offline (isolated
/// PATH so no real `claude`, gate OFF), and return the written registry text.
fn run_sync_registry(project: &Path) -> String {
    let bin = env!("CARGO_BIN_EXE_mustard-rt");
    // Isolate PATH so no real `claude` is found even on a dev host — keeps the
    // run fully deterministic and offline.
    let empty_bin_dir = tempfile::tempdir().expect("empty bin dir");
    let isolated_path = empty_bin_dir.path().to_string_lossy().into_owned();

    let output = Command::new(bin)
        .args(["run", "sync-registry", "--force"])
        .current_dir(project)
        .env("PATH", &isolated_path)
        .env("CLAUDE_PROJECT_DIR", project.to_string_lossy().as_ref())
        .env("MUSTARD_INTERPRET_CACHE", "off")
        // Cold-path LLM intentionally UNSET → default-OFF, no subprocess.
        .env_remove("MUSTARD_SCAN_LLM")
        .output()
        .expect("run mustard-rt");

    assert!(
        output.status.success(),
        "sync-registry exited {:?}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let registry_path = project.join(".claude").join("entity-registry.json");
    assert!(registry_path.exists(), "entity-registry.json not written");
    std::fs::read_to_string(&registry_path).expect("read registry")
}

/// Extract the `_patterns.rust.frameworks` array from the registry JSON.
/// Returns `None` when the key is absent (the legacy / no-signal shape).
fn rust_frameworks(registry: &str) -> Option<Vec<String>> {
    let doc: serde_json::Value = serde_json::from_str(registry).expect("registry is valid JSON");
    doc.get("_patterns")
        .and_then(|p| p.get("rust"))
        .and_then(|r| r.get("frameworks"))
        .and_then(|f| f.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// WITH the shipped GraphQL vocab dropped at `.claude/vocab/frameworks.toml`,
/// the scan must specialise: the GraphQL tokens in `src/foo.rs` fire as
/// `category = "framework"`, so `_patterns.rust.frameworks` contains
/// `"framework"`. End-to-end, offline.
#[test]
fn graphql_vocab_override_surfaces_framework_label() {
    let project = tempfile::tempdir().expect("project tempdir");
    setup_project(project.path(), true);

    let raw = run_sync_registry(project.path());
    let frameworks = rust_frameworks(&raw).unwrap_or_default();

    assert!(
        frameworks.iter().any(|f| f == "framework"),
        "WITH the GraphQL example vocab, _patterns.rust.frameworks must contain \
         \"framework\"; got {frameworks:?}\nregistry:\n{raw}"
    );
}

/// BASELINE — the SAME fixture with NO `.claude/vocab/frameworks.toml`. The
/// built-in base ships no GraphQL tokens, so no framework signal fires and
/// `_patterns.rust.frameworks` is absent (or empty). This is the control that
/// proves the WITH result comes from the opt-in vocab, not the core.
#[test]
fn baseline_without_vocab_has_no_framework_label() {
    let project = tempfile::tempdir().expect("project tempdir");
    setup_project(project.path(), false);

    let raw = run_sync_registry(project.path());
    let frameworks = rust_frameworks(&raw);

    assert!(
        frameworks.as_ref().map_or(true, Vec::is_empty),
        "BASELINE (no vocab) must NOT surface a framework label — the built-in \
         base does not know GraphQL tokens; got {frameworks:?}\nregistry:\n{raw}"
    );
}
