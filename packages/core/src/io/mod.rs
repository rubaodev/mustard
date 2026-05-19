//! Side-effecting infrastructure — the `io` layer.
//!
//! Where [`model`](crate::model) is pure data, `io` is everything that
//! touches the filesystem. Each capability is exposed **behind a trait** so
//! consumers and tests inject a fake instead of the concrete implementation
//! (Dependency Inversion):
//!
//! - [`event_store`] — the [`EventSink`](event_store::EventSink) trait and
//!   [`JsonlEventStore`](event_store::JsonlEventStore), the append-only /
//!   replay implementation over `.claude/.harness/events.jsonl`.
//! - [`pipeline_repo`] — the [`PipelineRepo`](pipeline_repo::PipelineRepo)
//!   trait and [`FsPipelineRepo`](pipeline_repo::FsPipelineRepo), read/write
//!   of `.claude/.pipeline-states/{specName}.json`.
//! - [`fs`] — the fail-open primitives (atomic write, append, read) that the
//!   two stores above are built on.
//!
//! Every operation in this layer is fail-open: it returns
//! [`Result`](crate::error::Result) and never panics, so the hooks that
//! consume it can degrade safely on any I/O failure.

pub mod event_store;
pub mod fs;
pub mod pipeline_repo;
