//! Enforcement modules — one module per concern, behind the `mustard-core`
//! `Check` / `Observer` contract.
//!
//! Each module consolidates a *family* of the old JavaScript hooks (b3 spec §
//! Arquitetura): porting 1:1 would preserve the fragmentation the migration
//! exists to remove.
//!
//! - Waves 1-2: [`bash_guard`] — the Bash-tool family.
//! - Wave 3: the Task / Subagent family — [`budget`] (prompt/return size),
//!   [`model_routing`] (model-selection gate), [`tracker`] (tool-use /
//!   main-context counters + agent/tool telemetry), [`skills_audit`]
//!   (recommended-skills count advisory).

pub mod bash_guard;
pub mod budget;
pub mod model_routing;
pub mod skills_audit;
pub mod tracker;
