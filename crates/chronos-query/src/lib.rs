//! chronos-query: Query engine for trace data.
//!
//! Provides the `QueryEngine` that can execute queries against trace data
//! using pre-built indices for fast lookups.

pub mod engine;
pub mod eval_dispatcher;
pub mod expr_eval;

pub use engine::QueryEngine;
pub use eval_dispatcher::{EvalBackend, SessionEvalDispatcher};
