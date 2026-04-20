//! chronos-query: Query engine for trace data.
//!
//! Provides the `QueryEngine` that can execute queries against trace data
//! using pre-built indices for fast lookups.

pub mod engine;

pub use engine::QueryEngine;
