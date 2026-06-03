//! `scan-guards-list` / `scan-guards-apply` — the Wave-2 enrich hand-off for
//! the `## Guards` block that Wave 1's deterministic `scan_claude` renderer
//! seeds into every SUBPROJECT `CLAUDE.md`.
//!
//! Wave 1 emits, for subprojects only (never the workspace root), a pending
//! Guards block delimited by [`scan_claude::GUARDS_PENDING_OPEN`] …
//! [`scan_claude::GUARDS_CLOSE`], carrying the deterministic facts (kind,
//! frameworks) in an HTML comment:
//!
//! ```text
//! ## Guards
//!
//! <!-- mustard:guards pending -->
//! <!-- facts: kind=rust; frameworks=serde, clap -->
//! <!-- /mustard:guards -->
//! ```
//!
//! - [`list`] finds every `CLAUDE.md` still carrying the `pending` marker,
//!   parses its facts, and emits a JSON worklist for the enrich agent.
//! - [`apply`] splices the agent's authored guards into that span (preserving
//!   every other byte), caps the line count, refuses the root, and flips the
//!   marker to its non-pending form so a re-run of `list` no longer picks it up.
//!
//! Both reuse the marker constants from `scan_claude` (single source — no
//! literal drift) and are fully fail-open per the `mustard-rt run` contract.

pub mod apply;
pub mod list;
