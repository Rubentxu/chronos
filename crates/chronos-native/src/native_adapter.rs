//! Native language trace adapter using ptrace.
//!
//! Implements the `TraceAdapter` trait from `chronos-capture` for
//! C/C++/Rust programs using Linux ptrace.

use chronos_capture::TraceAdapter;
use chronos_domain::{
    CaptureConfig, CaptureSession, EventData, EventType, Language, SourceLocation, TraceError,
    TraceEvent,
};
use std::path::Path;
use tracing::info;

use crate::ptrace_tracer::{PtraceConfig, PtraceEvent, PtraceTracer};
use crate::syscall_table::resolve_syscall;

/// Native language (C/C++/Rust) trace adapter using ptrace.
///
/// This adapter supports tracing compiled native binaries on Linux x86_64
/// using the ptrace system call interface.
pub struct NativeAdapter {
    /// Configuration for ptrace operations.
    ptrace_config: PtraceConfig,
}

impl NativeAdapter {
    /// Create a new native adapter with default configuration.
    pub fn new() -> Self {
        Self {
            ptrace_config: PtraceConfig::default(),
        }
    }

    /// Create a native adapter with custom ptrace configuration.
    pub fn with_config(config: PtraceConfig) -> Self {
        Self {
            ptrace_config: config,
        }
    }

    /// Build a CaptureSession from a successful ptrace launch.
    fn build_session(&self, pid: i32, config: &CaptureConfig) -> CaptureSession {
        let language = config
            .language
            .unwrap_or_else(|| Language::from_path(&config.target));

        let mut session = CaptureSession::new(pid as u32, language, config.clone());
        session.activate();
        session
    }

    /// Convert a PtraceEvent into a TraceEvent.
    ///
    /// Returns None for events that don't map to domain events.
    pub fn ptrace_event_to_trace_event(
        &self,
        ptrace_evt: &PtraceEvent,
        event_id: u64,
        timestamp_ns: u64,
    ) -> Option<TraceEvent> {
        match ptrace_evt {
            PtraceEvent::Stopped {
                pid,
                signal,
                signal_name,
            } => {
                // Map interesting signals to domain events
                match *signal {
                    // SIGTRAP at non-syscall stops — likely a breakpoint or single-step
                    5 => Some(TraceEvent::new(
                        event_id,
                        timestamp_ns,
                        *pid as u64,
                        EventType::BreakpointHit,
                        SourceLocation::from_address(0),
                        EventData::Breakpoint {
                            breakpoint_id: 0,
                            address: 0,
                        },
                    )),
                    // Other signals
                    _ => Some(TraceEvent::signal(
                        event_id,
                        timestamp_ns,
                        *pid as u64,
                        *signal,
                        signal_name.as_str(),
                        0,
                    )),
                }
            }

            PtraceEvent::Syscall {
                pid,
                syscall_nr,
                is_entry,
            } => {
                let event_type = if *is_entry {
                    EventType::SyscallEnter
                } else {
                    EventType::SyscallExit
                };

                Some(TraceEvent::new(
                    event_id,
                    timestamp_ns,
                    *pid as u64,
                    event_type,
                    SourceLocation::from_address(0),
                    EventData::Syscall {
                        name: resolve_syscall(*syscall_nr),
                        number: *syscall_nr,
                        args: Vec::new(),
                        return_value: 0,
                    },
                ))
            }

            PtraceEvent::PtraceEvent { pid, event_code, .. } => {
                // Map ptrace events to thread events for clone/fork
                let event_type = match *event_code {
                    // PTRACE_EVENT_CLONE = 3
                    3 => EventType::ThreadCreate,
                    // PTRACE_EVENT_FORK = 1
                    1 => EventType::ThreadCreate,
                    // PTRACE_EVENT_VFORK = 2
                    2 => EventType::ThreadCreate,
                    // PTRACE_EVENT_EXEC = 4
                    4 => EventType::Custom,
                    _ => EventType::Unknown,
                };

                Some(TraceEvent::new(
                    event_id,
                    timestamp_ns,
                    *pid as u64,
                    event_type,
                    SourceLocation::from_address(0),
                    EventData::Empty,
                ))
            }

            PtraceEvent::Exited { pid, exit_code } => {
                // Exited events don't map directly to TraceEvent types;
                // we use Custom to record the exit
                Some(TraceEvent::new(
                    event_id,
                    timestamp_ns,
                    *pid as u64,
                    EventType::Custom,
                    SourceLocation::from_address(0),
                    EventData::Custom {
                        name: "process_exit".into(),
                        data_json: format!(r#"{{"exit_code": {}}}"#, exit_code),
                    },
                ))
            }

            PtraceEvent::Signaled {
                pid,
                signal,
                signal_name,
                core_dumped: _,
            } => Some(TraceEvent::signal(
                event_id,
                timestamp_ns,
                *pid as u64,
                *signal,
                signal_name.as_str(),
                0,
            )),

            PtraceEvent::Registers { pid, regs } => Some(TraceEvent::new(
                event_id,
                timestamp_ns,
                *pid as u64,
                EventType::Custom,
                SourceLocation::from_address(regs.rip),
                EventData::Registers(regs.clone()),
            )),
        }
    }
}

impl Default for NativeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceAdapter for NativeAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        let program_path = Path::new(&config.target);

        if !program_path.exists() {
            return Err(TraceError::CaptureFailed(format!(
                "Target binary not found: {}",
                config.target
            )));
        }

        let mut tracer = PtraceTracer::new(self.ptrace_config.clone());

        let pid = tracer.launch(program_path, &config.args)?;

        info!("Native capture started for PID {}", pid);

        let session = self.build_session(pid, &config);

        Ok(session)
    }

    fn stop_capture(&self, session: &CaptureSession) -> Result<(), TraceError> {
        // In a real implementation, the tracer would be stored in the session
        // For now, we just log the stop
        info!("Stopping native capture for PID {}", session.pid);
        Ok(())
    }

    fn attach_to_process(
        &self,
        pid: u32,
        config: CaptureConfig,
    ) -> Result<CaptureSession, TraceError> {
        let mut tracer = PtraceTracer::new(self.ptrace_config.clone());

        tracer.attach(pid as i32)?;

        info!("Attached to running process PID {}", pid);

        let session = self.build_session(pid as i32, &config);

        Ok(session)
    }

    fn get_language(&self) -> Language {
        Language::C // Native adapter handles C, C++, and Rust
    }

    fn name(&self) -> &str {
        "native-ptrace"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::RegisterState;
    #[test]
    fn test_native_adapter_creation() {
        let adapter = NativeAdapter::new();
        assert_eq!(adapter.name(), "native-ptrace");
        assert_eq!(adapter.get_language(), Language::C);
        assert!(!adapter.supports_expression_eval());
    }

    #[test]
    fn test_native_adapter_default() {
        let adapter = NativeAdapter::default();
        assert_eq!(adapter.name(), "native-ptrace");
    }

    #[test]
    fn test_native_adapter_custom_config() {
        let config = PtraceConfig {
            trace_syscalls: true,
            capture_registers: false,
            follow_children: false,
        };
        let adapter = NativeAdapter::with_config(config);
        assert_eq!(adapter.name(), "native-ptrace");
    }

    #[test]
    fn test_start_capture_nonexistent_binary() {
        let adapter = NativeAdapter::new();
        let config = CaptureConfig::new("/nonexistent/binary");
        let result = adapter.start_capture(config);
        assert!(result.is_err());
        if let Err(TraceError::CaptureFailed(msg)) = result {
            assert!(msg.contains("not found"));
        } else {
            panic!("Expected CaptureFailed error");
        }
    }

    #[test]
    fn test_ptrace_event_to_trace_stopped_sigtrap() {
        let adapter = NativeAdapter::new();
        let ptrace_evt = PtraceEvent::Stopped {
            pid: 1234,
            signal: 5, // SIGTRAP
            signal_name: "SIGTRAP".into(),
        };

        let trace_evt = adapter
            .ptrace_event_to_trace_event(&ptrace_evt, 1, 1000)
            .expect("should convert");

        assert_eq!(trace_evt.event_type, EventType::BreakpointHit);
        assert_eq!(trace_evt.event_id, 1);
        assert_eq!(trace_evt.thread_id, 1234);
    }

    #[test]
    fn test_ptrace_event_to_trace_stopped_signal() {
        let adapter = NativeAdapter::new();
        let ptrace_evt = PtraceEvent::Stopped {
            pid: 1234,
            signal: 11, // SIGSEGV
            signal_name: "SIGSEGV".into(),
        };

        let trace_evt = adapter
            .ptrace_event_to_trace_event(&ptrace_evt, 2, 2000)
            .expect("should convert");

        assert_eq!(trace_evt.event_type, EventType::SignalDelivered);
        match &trace_evt.data {
            EventData::Signal {
                signal_number,
                signal_name,
            } => {
                assert_eq!(*signal_number, 11);
                assert_eq!(signal_name, "SIGSEGV");
            }
            _ => panic!("Expected Signal data"),
        }
    }

    #[test]
    fn test_ptrace_event_to_trace_syscall() {
        let adapter = NativeAdapter::new();

        let ptrace_evt = PtraceEvent::Syscall {
            pid: 5678,
            syscall_nr: 1,
            is_entry: true,
        };
        let trace_evt = adapter
            .ptrace_event_to_trace_event(&ptrace_evt, 3, 3000)
            .expect("should convert");
        assert_eq!(trace_evt.event_type, EventType::SyscallEnter);
        assert_eq!(trace_evt.thread_id, 5678);

        let ptrace_evt_exit = PtraceEvent::Syscall {
            pid: 5678,
            syscall_nr: 1,
            is_entry: false,
        };
        let trace_evt_exit = adapter
            .ptrace_event_to_trace_event(&ptrace_evt_exit, 4, 4000)
            .expect("should convert");
        assert_eq!(trace_evt_exit.event_type, EventType::SyscallExit);
    }

    #[test]
    fn test_ptrace_event_to_trace_exited() {
        let adapter = NativeAdapter::new();
        let ptrace_evt = PtraceEvent::Exited {
            pid: 9999,
            exit_code: 42,
        };

        let trace_evt = adapter
            .ptrace_event_to_trace_event(&ptrace_evt, 10, 10000)
            .expect("should convert");

        assert_eq!(trace_evt.event_type, EventType::Custom);
        match &trace_evt.data {
            EventData::Custom { name, data_json } => {
                assert_eq!(name, "process_exit");
                assert!(data_json.contains("42"));
            }
            _ => panic!("Expected Custom data"),
        }
    }

    #[test]
    fn test_ptrace_event_to_trace_signaled() {
        let adapter = NativeAdapter::new();
        let ptrace_evt = PtraceEvent::Signaled {
            pid: 7777,
            signal: 9,
            signal_name: "SIGKILL".into(),
            core_dumped: false,
        };

        let trace_evt = adapter
            .ptrace_event_to_trace_event(&ptrace_evt, 11, 11000)
            .expect("should convert");

        assert_eq!(trace_evt.event_type, EventType::SignalDelivered);
        assert_eq!(trace_evt.thread_id, 7777);
    }

    #[test]
    fn test_ptrace_event_to_trace_ptrace_event_clone() {
        let adapter = NativeAdapter::new();
        let ptrace_evt = PtraceEvent::PtraceEvent {
            pid: 5555,
            event_code: 3, // PTRACE_EVENT_CLONE
            new_pid: None,
        };

        let trace_evt = adapter
            .ptrace_event_to_trace_event(&ptrace_evt, 12, 12000)
            .expect("should convert");

        assert_eq!(trace_evt.event_type, EventType::ThreadCreate);
    }

    #[test]
    fn test_ptrace_event_to_trace_registers() {
        let adapter = NativeAdapter::new();
        let regs = RegisterState {
            rax: 42,
            rip: 0x400500,
            ..Default::default()
        };
        let ptrace_evt = PtraceEvent::Registers {
            pid: 3333,
            regs: regs.clone(),
        };

        let trace_evt = adapter
            .ptrace_event_to_trace_event(&ptrace_evt, 13, 13000)
            .expect("should convert");

        assert_eq!(trace_evt.event_type, EventType::Custom);
        assert_eq!(trace_evt.location.address, 0x400500);
        match &trace_evt.data {
            EventData::Registers(r) => {
                assert_eq!(r.rax, 42);
            }
            _ => panic!("Expected Registers data"),
        }
    }

    #[test]
    fn test_build_session() {
        let adapter = NativeAdapter::new();
        let config = CaptureConfig::new("test.rs");
        let session = adapter.build_session(1234, &config);

        assert_eq!(session.pid, 1234);
        assert_eq!(session.language, Language::Rust);
        assert_eq!(session.state, chronos_domain::SessionState::Active);
    }

    #[test]
    fn test_build_session_unknown_language() {
        let adapter = NativeAdapter::new();
        let config = CaptureConfig::new("binary_without_extension");
        let session = adapter.build_session(1234, &config);

        assert_eq!(session.language, Language::Unknown);
    }
}
