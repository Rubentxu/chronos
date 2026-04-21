//! chronos-sandbox: Sandbox execution environment for multi-language debug targets.
//!
//! This crate provides containerized sandbox environments for launching and
//! debugging programs in various languages (Rust, Java, Go, Python, Node.js, C++).
//!
//! ## Architecture
//!
//! - **Container layer**: Podman-based container management via Quadlet files
//! - **Debug targets**: Language-specific debug protocol implementations
//! - **KPI collection**: Performance overhead, latency, and throughput measurement
//! - **Session management**: Trace capture and workload generation
//! - **Manifest system**: YAML-based scenario definition and scoring

pub mod container;
pub mod error;
pub mod kpi;
pub mod manifest;
pub mod session;
pub mod targets;
pub mod workload;

pub use error::SandboxError;
