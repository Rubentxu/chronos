//! Native ptrace probe backend that feeds events to an EventBus in real-time.
//!
//! This backend replaces the "record everything then analyze" model of `CaptureRunner`
//! with a live event bus model. Events are pushed to an `EventBus` ring buffer
//! as they occur, allowing real-time monitoring and querying.

use crate::native_adapter::NativeAdapter;
use crate::ptrace_tracer::{PtraceConfig, PtraceTracer};
use crate::symbol_resolver::SymbolResolver;
use chronos_domain::bus::EventBusHandle;
use chronos_domain::semantic::{ResolveContext, ResolverPipeline, SemanticEvent, SemanticResolver};
use chronos_domain::{
    CaptureConfig, CaptureSession, Language, ProbeBackend, SourceLocation, TraceError,
    TraceEvent,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use tracing::{debug, error, info, warn};

/// Native ptrace probe backend for real-time event bus feeding.
pub struct NativeProbeBackend {
    /// Shared event bus handle.
    event_bus: EventBusHandle,
    /// Language being traced.
    language: Language,
    /// Semantic resolver pipeline.
    resolver_pipeline: ResolverPipeline,
    /// Flag to signal the background thread to stop.
    running: Arc<AtomicBool>,
    /// Handle to the polling thread (if running).
    thread_handle: std::sync::Mutex<Option<thread::JoinHandle<()>>>,
    /// Session ID to PID mapping for cleanup (used by stop_probe for PID tracking).
    session_pids: std::sync::Arc<std::sync::Mutex<HashMap<String, u32>>>,
    /// The PID of the currently traced process (for stop_probe to kill).
    traced_pid: std::sync::Arc<std::sync::Mutex<Option<i32>>>,
}

impl NativeProbeBackend {
    /// Create a new native probe backend with the given event bus handle.
    pub fn new(event_bus: EventBusHandle) -> Self {
        Self {
            event_bus,
            language: Language::C,
            resolver_pipeline: ResolverPipeline::new(),
            running: Arc::new(AtomicBool::new(false)),
            thread_handle: std::sync::Mutex::new(None),
            session_pids: std::sync::Arc::new(std::sync::Mutex::new(HashMap::new())),
            traced_pid: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// Create a new native probe backend with a default event bus.
    pub fn with_default_bus() -> Self {
        let bus = chronos_domain::bus::EventBus::new_shared(10000); // 10k event capacity
        Self::new(bus)
    }

    /// Set the language to trace.
    pub fn with_language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    /// Add a semantic resolver to the pipeline.
    pub fn with_resolver(mut self, resolver: Box<dyn SemanticResolver>) -> Self {
        self.resolver_pipeline.add_resolver(resolver);
        self
    }

    /// Start a probe for a new process.
    ///
    /// Spawns the target binary via `PtraceTracer::launch()` and starts a background
    /// thread that runs the ptrace event loop. Each ptrace event is converted to a
    /// `TraceEvent` and pushed to the `EventBus` in real-time.
    ///
    /// Returns a `CaptureSession` immediately (non-blocking).
    pub fn start_probe(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        let program_path = PathBuf::from(&config.target);

        if !program_path.exists() {
            return Err(TraceError::CaptureFailed(format!(
                "Target binary not found: {}",
                config.target
            )));
        }

        let language = config.language.unwrap_or_else(|| Language::from_path(&config.target));
        let event_bus = self.event_bus.clone();
        let running = self.running.clone();
        let resolver_pipeline = self.resolver_pipeline.clone();

        // Pre-load symbols from the binary
        let symbol_resolver = {
            let mut resolver = SymbolResolver::new();
            match resolver.load_from_binary(&program_path) {
                Ok(()) => {
                    info!(
                        "Loaded {} symbols from {}",
                        resolver.symbol_count(),
                        config.target
                    );
                    Some(resolver)
                }
                Err(e) => {
                    warn!(
                        "Could not load symbols from {}: {}",
                        config.target, e
                    );
                    None
                }
            }
        };

        let ptrace_config = PtraceConfig {
            trace_syscalls: config.capture_syscalls,
            capture_registers: true,
            follow_children: true,
        };

        // Spawn background thread to run the event loop
        running.store(true, Ordering::SeqCst);

        let target = config.target.clone();
        let args = config.args.clone();

        // Shared slot so the thread can publish its PID back for stop_probe to kill.
        let traced_pid_thread = self.traced_pid.clone();

        let handle = thread::Builder::new()
            .name("chronos-native-probe".into())
            .spawn(move || {
                Self::run_probe_loop_with_pid_cb(
                    &target,
                    args,
                    &ptrace_config,
                    &running,
                    symbol_resolver.as_ref(),
                    event_bus,
                    resolver_pipeline,
                    language,
                    move |pid: i32| {
                        *traced_pid_thread.lock().unwrap() = Some(pid);
                    },
                );
            })
            .map_err(|e| TraceError::CaptureFailed(format!("Failed to spawn probe thread: {}", e)))?;

        // Store the handle - we need to get the PID first
        // Since the thread manages its own PID, we'll store a placeholder for now
        // The actual PID tracking happens inside the thread
        *self.thread_handle.lock().unwrap() = Some(handle);

        // Build and return the session (non-blocking)
        // Note: PID will be 0 for spawned processes since the thread manages it
        let session = CaptureSession::new(0, language, config);

        // Note: actual PID will be tracked via session_pids when the thread starts
        // For now we return the session - the background thread has the real PID

        Ok(session)
    }

    /// Attach a probe to an existing process.
    ///
    /// Similar to `start_probe` but uses `PtraceTracer::attach()` instead of `launch()`
    /// to attach to an already-running process.
    pub fn attach_probe(&self, pid: u32, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        let language = config.language.unwrap_or(Language::C);
        let event_bus = self.event_bus.clone();
        let running = self.running.clone();
        let resolver_pipeline = self.resolver_pipeline.clone();

        let ptrace_config = PtraceConfig {
            trace_syscalls: config.capture_syscalls,
            capture_registers: true,
            follow_children: true,
        };

        // Spawn background thread to run the event loop in attach mode
        running.store(true, Ordering::SeqCst);

        let handle = thread::Builder::new()
            .name("chronos-native-probe-attach".into())
            .spawn(move || {
                Self::run_probe_loop_attach(
                    pid,
                    &ptrace_config,
                    &running,
                    event_bus,
                    resolver_pipeline,
                    language,
                );
            })
            .map_err(|e| TraceError::CaptureFailed(format!("Failed to spawn probe thread: {}", e)))?;

        *self.thread_handle.lock().unwrap() = Some(handle);

        let mut session = CaptureSession::new(pid, language, config);
        session.activate();

        Ok(session)
    }

    /// Stop an active probe session.
    ///
    /// Sets the running flag to false and kills the traced process to
    /// interrupt any blocking waitpid. Returns immediately without waiting
    /// for the probe thread to exit (non-blocking).
    pub fn stop_probe(&self, session: &CaptureSession) -> Result<(), TraceError> {
        // Signal the thread to stop
        self.running.store(false, Ordering::SeqCst);

        // Kill the traced process to interrupt blocking waitpid.
        let pid_to_kill = self.traced_pid.lock().unwrap().take();
        if let Some(pid) = pid_to_kill {
            info!("Killing traced process PID {} to stop probe", pid);
            let _ = nix::sys::signal::kill(
                nix::unistd::Pid::from_raw(pid),
                nix::sys::signal::Signal::SIGKILL,
            );
        }

        // Detach the thread handle without joining — the probe thread will exit
        // on its own once it sees running=false and/or the process is dead.
        // We do NOT join here to avoid blocking the MCP server's response path.
        if let Some(handle) = self.thread_handle.lock().unwrap().take() {
            // Spawn a background thread just to join and log the result.
            let session_id = session.session_id.clone();
            std::thread::spawn(move || {
                match handle.join() {
                    Ok(()) => info!("Probe thread exited cleanly for session {}", session_id),
                    Err(_) => warn!("Probe thread panicked during shutdown for {}", session_id),
                }
            });
        }

        Ok(())
    }

    /// Internal wrapper: calls run_probe_loop with a PID callback.
    fn run_probe_loop_with_pid_cb(
        program_path: &str,
        args: Vec<String>,
        ptrace_config: &PtraceConfig,
        running: &Arc<AtomicBool>,
        symbol_resolver: Option<&SymbolResolver>,
        event_bus: EventBusHandle,
        resolver_pipeline: ResolverPipeline,
        language: Language,
        on_pid_launched: impl FnOnce(i32),
    ) {
        Self::run_probe_loop(
            Some((program_path, vec![])),
            ptrace_config,
            program_path,
            args,
            running,
            symbol_resolver,
            event_bus,
            resolver_pipeline,
            language,
            Some(on_pid_launched),
        );
    }

    /// Internal: Run the probe event loop for a spawned process.
    fn run_probe_loop(
        target: Option<(&str, Vec<String>)>,
        ptrace_config: &PtraceConfig,
        program_path: &str,
        args: Vec<String>,
        running: &Arc<AtomicBool>,
        symbol_resolver: Option<&SymbolResolver>,
        event_bus: EventBusHandle,
        resolver_pipeline: ResolverPipeline,
        _language: Language,
        on_pid_launched: Option<impl FnOnce(i32)>,
    ) {
        let mut tracer = PtraceTracer::new(ptrace_config.clone());
        let adapter = NativeAdapter::new();

        // Check running flag before entering launch
        if !running.load(Ordering::Relaxed) {
            return;
        }

        // Launch the target process
        let pid = match target {
            Some(_) => {
                match tracer.launch(PathBuf::from(program_path).as_path(), &args) {
                    Ok(p) => {
                        info!("Probe started for PID {}", p);
                        // Notify caller of the launched PID so stop_probe can kill it.
                        if let Some(cb) = on_pid_launched {
                            cb(p);
                        }
                        p
                    }
                    Err(e) => {
                        error!("Failed to launch {}: {}", program_path, e);
                        return;
                    }
                }
            }
            None => {
                return;
            }
        };

        let mut event_id: u64 = 0;

        // Check running flag before entering main loop
        if !running.load(Ordering::Relaxed) {
            if pid > 0 {
                let _ = tracer.kill(pid);
            }
            return;
        }

        // Main event loop
        while running.load(Ordering::Relaxed) {
            let ptrace_event = match tracer.wait_event() {
                Ok(Some(event)) => event,
                Ok(None) => {
                    // None from blocking waitpid means ECHILD (no more children) OR
                    // the process was killed/exited. Either way, stop the loop.
                    debug!("Probe: no more traced processes, exiting event loop");
                    break;
                }
                Err(e) => {
                    debug!("wait_event error: {}", e);
                    break;
                }
            };

            let timestamp_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;

            // Convert to TraceEvent and push to bus
            if let Some(mut trace_event) = adapter.ptrace_event_to_trace_event(
                &ptrace_event,
                event_id,
                timestamp_ns,
            ) {
                // Resolve symbol if available
                if let Some(resolver) = symbol_resolver {
                    let addr = trace_event.location.address;
                    if addr > 0 {
                        if let Some(sym) = resolver.resolve(addr) {
                            trace_event.location = SourceLocation::new(
                                sym.file.as_deref().unwrap_or(""),
                                sym.line.unwrap_or(0),
                                &sym.name,
                                addr,
                            );
                        }
                    }
                }

                // Push raw event to raw buffer for QueryEngine
                event_bus.push_raw(trace_event.clone());

                // Resolve to semantic event via the pipeline
                let ctx = ResolveContext {
                    pid: pid as u32,
                    binary_path: Some(program_path.to_string()),
                };
                let semantic_event = resolver_pipeline.resolve(&trace_event, &ctx);
                event_bus.push(semantic_event);

                event_id += 1;
            }

            // Continue the traced process
            let event_pid = ptrace_event.pid();
            if event_pid > 0 && !matches!(ptrace_event, crate::ptrace_tracer::PtraceEvent::Exited { .. }) {
                let continue_result = if ptrace_config.trace_syscalls {
                    tracer.syscall_continue(event_pid)
                } else {
                    tracer.continue_execution(event_pid)
                };
                if let Err(e) = continue_result {
                    debug!("Failed to continue PID {}: {}", event_pid, e);
                }
            }
        }

        // Cleanup
        if let Err(e) = tracer.kill(pid) {
            debug!("Failed to kill PID {}: {}", pid, e);
        }

        info!("Probe loop ended for PID {}", pid);
    }

    /// Internal: Run the probe event loop for an attached process.
    fn run_probe_loop_attach(
        pid: u32,
        ptrace_config: &PtraceConfig,
        running: &Arc<AtomicBool>,
        event_bus: EventBusHandle,
        resolver_pipeline: ResolverPipeline,
        _language: Language,
    ) {
        let mut tracer = PtraceTracer::new(ptrace_config.clone());
        let adapter = NativeAdapter::new();

        if let Err(e) = tracer.attach(pid as i32) {
            error!("Failed to attach to PID {}: {}", pid, e);
            return;
        }

        info!("Probe attached to PID {}", pid);

        let mut event_id: u64 = 0;

        // Main event loop
        while running.load(Ordering::Relaxed) {
            let ptrace_event = match tracer.wait_event() {
                Ok(Some(event)) => event,
                Ok(None) => {
                    debug!("No more traced processes");
                    break;
                }
                Err(e) => {
                    debug!("wait_event error: {}", e);
                    break;
                }
            };

            let timestamp_ns = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;

            // Convert to TraceEvent and push to bus
            if let Some(trace_event) = adapter.ptrace_event_to_trace_event(
                &ptrace_event,
                event_id,
                timestamp_ns,
            ) {
                // Push raw event to raw buffer for QueryEngine
                event_bus.push_raw(trace_event.clone());

                // Resolve to semantic event via the pipeline
                let ctx = ResolveContext {
                    pid,
                    binary_path: None,
                };
                let semantic_event = resolver_pipeline.resolve(&trace_event, &ctx);
                event_bus.push(semantic_event);

                event_id += 1;
            }

            // Continue the traced process
            let event_pid = ptrace_event.pid();
            if event_pid > 0 && !matches!(ptrace_event, crate::ptrace_tracer::PtraceEvent::Exited { .. }) {
                let continue_result = if ptrace_config.trace_syscalls {
                    tracer.syscall_continue(event_pid)
                } else {
                    tracer.continue_execution(event_pid)
                };
                if let Err(e) = continue_result {
                    debug!("Failed to continue PID {}: {}", event_pid, e);
                }
            }
        }

        // Cleanup - detach instead of kill for attached processes
        if let Err(e) = tracer.detach(pid as i32) {
            debug!("Failed to detach from PID {}: {}", pid, e);
        }

        info!("Probe loop ended for attached PID {}", pid);
    }
}

impl ProbeBackend for NativeProbeBackend {
    /// Always returns true on Linux (ptrace is available).
    fn is_available(&self) -> bool {
        cfg!(target_os = "linux")
    }

    /// Returns "native-ptrace".
    fn name(&self) -> &str {
        "native-ptrace"
    }

    /// Drain all buffered semantic events from the event bus.
    fn drain_events(&mut self) -> Result<Vec<SemanticEvent>, TraceError> {
        Ok(self.event_bus.snapshot())
    }
}

impl NativeProbeBackend {
    /// Drain all buffered raw trace events from the event bus.
    ///
    /// Used by MCP tools (probe_stop, session_snapshot) to build QueryEngine
    /// which requires the original TraceEvent data.
    pub fn drain_raw_events(&self) -> Vec<TraceEvent> {
        self.event_bus.snapshot_raw()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_probe_backend_creation() {
        let bus = chronos_domain::bus::EventBus::new_shared(100);
        let backend = NativeProbeBackend::new(bus);
        assert_eq!(backend.name(), "native-ptrace");
    }

    #[test]
    fn test_native_probe_backend_is_available() {
        let bus = chronos_domain::bus::EventBus::new_shared(100);
        let backend = NativeProbeBackend::new(bus);
        // Should be true on Linux
        #[cfg(target_os = "linux")]
        assert!(backend.is_available());
    }

    #[test]
    fn test_native_probe_backend_with_language() {
        let bus = chronos_domain::bus::EventBus::new_shared(100);
        let backend = NativeProbeBackend::new(bus)
            .with_language(Language::Rust);
        assert!(backend.is_available());
    }
}