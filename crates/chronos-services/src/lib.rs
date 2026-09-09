//! Chronos services layer — extracted business logic from chronos-mcp.
//!
//! Services are plain Rust structs that can be called from any RPC layer
//! (today's rmcp, tomorrow's REST, etc.).

pub mod debug_read;
pub mod error;
pub mod output;
pub mod query_service;
