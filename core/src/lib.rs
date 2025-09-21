#![deny(missing_docs)]
//! ai_core: shared building blocks (config, KV, logging, job types).

/// Configuration helpers (AppId, dirs, load_or_init, etc.)
pub mod cfg;
/// Simple file-backed KV store with serde helpers.
pub mod store;
/// Tracing/log initialization helpers.
pub mod logx;
/// Shared job model used by scheduler, web, and tools.
pub mod job;
