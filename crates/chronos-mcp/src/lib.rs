//! chronos-mcp: MCP server for time-travel debugging.
//!
//! Implements the Model Context Protocol server that exposes Chronos
//! debugging tools to AI assistants.

pub mod composition;
pub mod concurrency_wire;
pub mod cost_memory_wire;
pub mod init_error;
pub mod security;
pub mod server;
pub mod telemetry_wire;

pub use concurrency_wire::{
    build_graph_from_wire, classify_pair_from_json, classify_pair_via_wire,
    wire_version as concurrency_wire_version, GraphEdgeWire, RaceClassificationWire,
};
pub use cost_memory_wire::{
    run_uat_m7_01_as_json, run_uat_m7_01_wire, run_uat_m7_02_as_json, run_uat_m7_02_wire,
    to_wire as to_uat_wire, wire_version as cost_memory_wire_version, UatResultWire,
};
pub use server::ChronosServer;
pub use telemetry_wire::{default_telemetry_receiver, in_memory_telemetry};
