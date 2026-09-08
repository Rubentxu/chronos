//! chronos-capture: Trace adapter trait and capture pipeline.

pub mod adapter;
pub mod config;
pub mod factory;
pub mod pipeline;
pub mod state_recorder;

pub use adapter::TraceAdapter;
pub use config::CaptureConfig;
pub use factory::AdapterRegistry;
pub use pipeline::CapturePipeline;
pub use state_recorder::StateObservationRecorder;
