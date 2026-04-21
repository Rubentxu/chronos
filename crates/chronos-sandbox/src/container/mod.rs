//! Container management layer for chronos-sandbox.
//!
//! Provides Podman-based container orchestration via Quadlet files and CLI invocations.

pub mod network;
pub mod pod;
pub mod quadlet;
pub mod runner;
pub mod volume;

// Re-exports
pub use network::{create_network, remove_network, SandboxNetwork};
pub use pod::PodManager;
pub use quadlet::{ContainerOptions, QuadletBuilder, QuadletFile};
pub use runner::{wait_ready, ContainerRunner, RunningContainer};
pub use volume::{create_volume, remove_volume, SandboxVolume};
