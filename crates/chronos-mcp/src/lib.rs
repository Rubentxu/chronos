//! chronos-mcp: MCP server for time-travel debugging.
//!
//! Implements the Model Context Protocol server that exposes Chronos
//! debugging tools to AI assistants.

pub mod init_error;
pub mod security;
pub mod server;

pub use server::ChronosServer;
