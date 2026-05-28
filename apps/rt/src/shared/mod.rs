//! `shared` — cross-face infrastructure consumed by **both** the enforcement
//! face (`hooks`) and the script face (`commands`).
//!
//! Keeping these here (instead of under `commands/`) preserves a clean
//! dependency DAG: `hooks` and `commands` both depend on `shared`, and `shared`
//! never depends back on either. A hook reaching into a command module would
//! invert that layering — this module exists to make that impossible.
//!
//! - [`context`] — run-context resolution (cwd / session-id / current-spec),
//!   the port of `hook-env.js`'s runtime probing.
//! - [`events`] — the NDJSON event bus: classification/routing ([`events::route`]),
//!   the append-only writer ([`events::writer_ndjson`]) and content-addressed
//!   blob spill ([`events::blob_spill`]).

pub mod context;
pub mod events;
