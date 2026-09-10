//! Chronos services layer — extracted business logic from chronos-mcp.
//!
//! Services are plain Rust structs that can be called from any RPC layer
//! (today's rmcp, tomorrow's REST, etc.).

pub mod analysis;
pub mod browser_probe;
pub mod debug_read;
pub mod debug_trace;
pub mod debug_trace_specialized;
pub mod diff;
pub mod error;
pub mod output;
pub mod probe;
pub mod query_service;
pub mod sessions;
pub mod tripwires;
