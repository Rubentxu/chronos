//! chronos-mcp: MCP server for time-travel debugging.
//!
//! Implements the Model Context Protocol server that exposes Chronos
//! debugging tools to AI assistants.

pub mod composition;
pub mod concurrency_wire;
pub mod init_error;
pub mod security;
pub mod server;

pub use concurrency_wire::{
    build_graph_from_wire, classify_pair_from_json, classify_pair_via_wire, wire_version,
    GraphEdgeWire, RaceClassificationWire,
};
pub use server::ChronosServer;
