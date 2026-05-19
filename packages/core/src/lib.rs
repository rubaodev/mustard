#![forbid(unsafe_code)]
// `clippy::unwrap_used` is `deny` workspace-wide so no hook-path code can
// panic (b2 spec § Preocupações — fail-open). Clippy does *not* exempt
// `#[cfg(test)]` code from that lint, so the spec's "exceto em módulos de
// teste" carve-out is applied explicitly here: under `cfg(test)`, `.unwrap()`
// / `.expect()` are allowed — a panicking assertion *is* a test failure.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
//! `mustard-core` — shared foundation crate for the Mustard Rust migration.
//!
//! This crate concentrates the logic that hooks, scripts, and the CLI all
//! depend on, so the port from JavaScript (epics B3/B4/B5) stays lean instead
//! of re-implementing the same primitives dozens of times.
//!
//! Layers (built incrementally across the b2 waves):
//!
//! - [`model`] — pure `serde` data types with zero side effects: the harness
//!   event schema, the hook contract, and `pipeline-state`. **Wave 1.**
//! - [`io`] — side-effecting infrastructure behind traits: the event log,
//!   `pipeline-state` read/write, and fail-open filesystem primitives.
//!   **Wave 2.**
//! - [`error`] — the crate's typed error plus fail-open helpers.
//! - cross-cutting foundation — [`config`] (enforcement modes), [`env`] (the
//!   `hook-env.js` port), [`metrics`] (the `metrics-emit.js` port), and
//!   [`knowledge`] (knowledge extraction + the inter-agent context-selection
//!   API). **Wave 3.**

pub mod config;
pub mod env;
pub mod error;
pub mod io;
pub mod knowledge;
pub mod metrics;
pub mod model;
