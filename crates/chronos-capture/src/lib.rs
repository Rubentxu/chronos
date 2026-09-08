//! chronos-capture: Trace adapter trait and capture pipeline.

pub mod adapter;
pub mod config;
pub mod factory;
pub mod observation_log;
pub mod pipeline;
pub mod session_feed;
pub mod state_recorder;

pub use adapter::TraceAdapter;
pub use config::CaptureConfig;
pub use factory::AdapterRegistry;
pub use observation_log::{
    evaluate_property_on_session, property_violation_on_session, render_property_report,
    replay_observations, replay_target, report_dsl_properties_on_session,
    report_properties_on_session, ObservationLogWriter, PersistError, PropertyEvaluation,
};
pub use pipeline::CapturePipeline;
pub use session_feed::{open_durable_feed, SegmentedLogBackend};
pub use state_recorder::StateObservationRecorder;
