//! chronos-sandbox: MCP testing infrastructure for Chronos.
//!
//! Provides a test client for the Chronos MCP server that can:
//! - Spawn MCP server instances as child processes
//! - Send JSON-RPC calls and receive responses
//! - Manage debug sessions and capture probes

pub mod client;
pub mod programs;

pub use client::{McpTestClient, McpSession};
