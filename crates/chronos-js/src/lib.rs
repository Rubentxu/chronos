//! Chronos JavaScript/Node.js adapter using Chrome DevTools Protocol (CDP).
//!
//! This adapter spawns a Node.js process with `--inspect` enabled and communicates
//! with it via CDP over WebSocket to capture breakpoint, step, and exception events.

pub mod adapter;
pub mod cdp_client;
pub mod debugger;
pub mod error;
pub mod eval_backend;
pub mod semantic_resolver;
pub mod subprocess;

pub use adapter::{CdpSession, JsAdapter, JsCdpAdapter};
pub use error::JsAdapterError;
pub use eval_backend::JsCdpEvalBackend;
pub use semantic_resolver::JsSemanticResolver;
