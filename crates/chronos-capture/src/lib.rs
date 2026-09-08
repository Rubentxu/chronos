//! chronos-capture: Trace adapter trait and capture pipeline.

pub mod adapter;
pub mod config;
pub mod factory;
pub mod observation_log;
pub mod pipeline;
pub mod state_recorder;

pub use adapter::TraceAdapter;
pub use config::CaptureConfig;
pub use factory::AdapterRegistry;
pub use observation_log::{replay_observations, replay_target, ObservationLogWriter, PersistError};
pub use pipeline::CapturePipeline;
pub use state_recorder::StateObservationRecorder;
