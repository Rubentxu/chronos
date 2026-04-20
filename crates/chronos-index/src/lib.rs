//! chronos-index: In-memory indices for trace events.

pub mod builder;

// Re-export index types from domain
pub use chronos_domain::{ShadowIndex, TemporalIndex};
