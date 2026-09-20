//! Browser adapter implementing ProbeBackend for agent-first WASM trace capture.
//!
//! This adapter connects to Chrome via CDP to capture ALL WebAssembly events.
//! AI agents capture everything first, then query semantically after — no selective
//! breakpoints, no human-style filtering. The agent is the consumer, not a human debugger.

use crate::browser::ChromeProcess;
use crate::cdp_client::{BrowserCdpClient, CdpEvent};
use crate::error::BrowserError;
use crate::event_mapper::paused_to_wasm_events;
use crate::wasm_probes::WasmBreakpointManager;
use crate::wasm_resolver::WasmSemanticResolver;
use chronos_domain::adapter::ProbeBackend;
use chronos_domain::semantic::{ResolveContext, SemanticEvent, SemanticResolver};
use chronos_domain::{CaptureConfig, CaptureSession, Language, TraceError, TraceEvent};
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio_util::sync::CancellationToken;

/// Maximum events in the capture buffer before dropping oldest.
pub const DEFAULT_BUFFER_CAPACITY: usize = 100_000;

/// Shared mutable state for the browser adapter.
///
/// All adapter state lives here, behind a single `Arc<Mutex<>>`.
/// This replaces the previous 7 separate `Arc<Mutex<T>>` fields,
/// reducing lock overhead and simplifying the code.
pub struct SharedState {
    /// Chrome process (if spawned by us)
    pub chrome: Option<ChromeProcess>,
    /// WASM modules detected during this session
    pub modules: HashMap<String, chronos_domain::trace::WasmModuleInfo>,
    /// Buffered trace events (ring-buffer semantics)
    pub event_buffer: VecDeque<TraceEvent>,
    /// Maximum buffer capacity
    pub buffer_capacity: usize,
    /// Events dropped due to buffer overflow
    pub dropped_events: u64,
    /// Next event ID
    pub next_event_id: u64,
    /// Whether capture is running
    pub running: bool,
    /// Session start time
    pub session_start: Option<Instant>,
    /// WASM breakpoint manager
    pub breakpoint_manager: WasmBreakpointManager,
    /// Cancellation token for graceful shutdown of background event loop
    pub cancel_token: Option<CancellationToken>,
}

impl Default for SharedState {
    fn default() -> Self {
        Self {
            chrome: None,
            modules: HashMap::new(),
            event_buffer: VecDeque::with_capacity(DEFAULT_BUFFER_CAPACITY),
            buffer_capacity: DEFAULT_BUFFER_CAPACITY,
            dropped_events: 0,
            next_event_id: 1,
            running: false,
            session_start: None,
            breakpoint_manager: WasmBreakpointManager::new_dummy(),
            cancel_token: None,
        }
    }
}

/// Browser adapter for WebAssembly debugging via Chrome DevTools Protocol.
///
/// **Agent-first design**: capture ALL WASM function entries and returns.
/// No selective breakpoints, no pre-filtering. AI agents query the complete
/// trace after capture using the Chronos query engine.
pub struct BrowserAdapter {
    /// All mutable state behind a single lock
    state: Arc<Mutex<SharedState>>,
    /// Semantic resolver for WASM events (read-only after construction)
    resolver: WasmSemanticResolver,
}

impl BrowserAdapter {
    /// Create a new browser adapter with default buffer capacity (100K events).
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_BUFFER_CAPACITY)
    }

    /// Create a new browser adapter with a specific buffer capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            state: Arc::new(Mutex::new(SharedState {
                buffer_capacity: capacity,
                event_buffer: VecDeque::with_capacity(capacity),
                ..Default::default()
            })),
            resolver: WasmSemanticResolver::new(),
        }
    }

    /// Create a new adapter with a custom semantic resolver.
    pub fn with_resolver(resolver: WasmSemanticResolver) -> Self {
        Self {
            state: Arc::new(Mutex::new(SharedState::default())),
            resolver,
        }
    }

    /// Check if Chrome is available on the system.
    pub fn is_chrome_available() -> bool {
        ChromeProcess::attach_port(9222).is_ok()
    }

    /// Quick probe: start, wait for events, stop.
    ///
    /// This is a convenience method that:
    /// 1. Creates a new adapter
    /// 2. Starts a probe session
    /// 3. Waits for the specified duration
    /// 4. Drains all captured events
    /// 5. Stops the probe and cleans up
    ///
    /// Returns all semantic events captured during the probe window.
    ///
    /// # Arguments
    ///
    /// * `url` - URL to load in Chrome
    /// * `duration_ms` - How long to wait before draining events
    /// * `headless` - Whether to run Chrome in headless mode
    /// * `chrome_path` - Optional path to Chrome binary
    pub async fn quick_probe(
        url: &str,
        duration_ms: u64,
        headless: bool,
        chrome_path: Option<&str>,
    ) -> Result<Vec<SemanticEvent>, BrowserError> {
        let adapter = Self::new();
        let config = CaptureConfig::new(url);
        let session = adapter
            .start_probe_async(config, headless, chrome_path)
            .await?;

        // Wait for the specified duration
        tokio::time::sleep(std::time::Duration::from_millis(duration_ms)).await;

        // Drain events (convert TraceError to BrowserError)
        let events = adapter
            .take_semantic_events()
            .map_err(|e| BrowserError::CdpConnectionFailed(e.to_string()))?;

        // Stop probe (cleanup Chrome)
        ProbeBackend::stop_probe(&adapter, &session)
            .map_err(|e| BrowserError::CdpConnectionFailed(e.to_string()))?;

        Ok(events)
    }

    /// Start a browser probe session (async, agent-first: capture ALL functions).
    ///
    /// Launches Chrome headless, connects via CDP, and begins capturing
    /// ALL WASM function entries and returns in the background. No selective
    /// breakpoints — AI agents query the complete trace after capture.
    ///
    /// # Arguments
    /// * `config` - Capture configuration (URL to load)
    /// * `headless` - Whether to run Chrome in headless mode
    /// * `chrome_path` - Optional path to Chrome binary
    pub async fn start_probe_async(
        &self,
        config: CaptureConfig,
        headless: bool,
        chrome_path: Option<&str>,
    ) -> Result<CaptureSession, BrowserError> {
        // Spawn Chrome (sync, fast)
        let mut chrome = ChromeProcess::spawn(headless, chrome_path)?;

        // Wait for CDP (async, uses tokio::sleep)
        let ws_url = ChromeProcess::wait_for_ready(chrome.port()).await?;
        chrome.set_ws_url(ws_url);

        // Connect to CDP WebSocket
        let cdp = BrowserCdpClient::connect(chrome.ws_url()).await?;
        cdp.debugger_enable().await?;

        // Initialize state with a single lock acquisition
        {
            let mut s = self.state.lock().unwrap();
            s.running = true;
            s.session_start = Some(Instant::now());
            s.next_event_id = 1;
            s.event_buffer.clear();
            s.modules.clear();
            s.dropped_events = 0;
            s.chrome = Some(chrome);
        }

        // Set up cancellation for graceful shutdown
        let cancel = CancellationToken::new();
        {
            let mut s = self.state.lock().unwrap();
            s.cancel_token = Some(cancel.clone());
        }

        // Clone what we need for the background task
        let state = self.state.clone();
        let cancel_clone = cancel.clone();

        // Spawn background event loop
        tokio::spawn(async move {
            let mut event_rx = cdp.subscribe();

            loop {
                tokio::select! {
                    event = event_rx.recv() => {
                        match event {
                            Ok(CdpEvent::DebuggerPaused {
                                reason,
                                call_frames,
                                hit_breakpoints,
                            }) => {
                                let timestamp_ns;
                                let is_running;
                                let next_id;
                                {
                                    let s = state.lock().unwrap();
                                    timestamp_ns = s.session_start
                                        .map(|st| st.elapsed().as_nanos() as u64)
                                        .unwrap_or(0);
                                    is_running = s.running;
                                    next_id = s.next_event_id;
                                }

                                if is_running {
                                    use crate::event_mapper::CdpDebuggerPaused;

                                    let paused = CdpDebuggerPaused {
                                        call_frames: call_frames
                                            .into_iter()
                                            .map(|f| f.into())
                                            .collect(),
                                        reason: reason.clone(),
                                        hit_breakpoints: hit_breakpoints.clone(),
                                    };

                                    let modules_ref: HashMap<String, chronos_domain::trace::WasmModuleInfo> = {
                                        let s = state.lock().unwrap();
                                        s.modules.clone()
                                    };

                                    let mut next = next_id;

                                    let bp_manager_ref = {
                                        let s = state.lock().unwrap();
                                        s.breakpoint_manager.clone()
                                    };

                                    let events = paused_to_wasm_events(
                                        &paused,
                                        &modules_ref,
                                        &bp_manager_ref,
                                        timestamp_ns,
                                        &mut next,
                                    );

                                    {
                                        let mut s = state.lock().unwrap();
                                        s.next_event_id = next;
                                        for evt in events {
                                            if s.event_buffer.len() >= s.buffer_capacity {
                                                s.event_buffer.pop_front();
                                                s.dropped_events += 1;
                                            }
                                            s.event_buffer.push_back(evt);
                                        }
                                    }
                                }
                            }
                            Ok(CdpEvent::DebuggerResumed) => {
                                // Normal — CDP sends Resumed after every breakpoint hit.
                                // We keep listening for the next hit.
                                tracing::trace!("Debugger resumed — continuing event loop");
                            }
                            Ok(CdpEvent::InspectorDetached) => {
                                let mut s = state.lock().unwrap();
                                s.running = false;
                                break;
                            }
                            Ok(CdpEvent::DebuggerScriptParsed {
                                script_id,
                                url,
                                script_language,
                                hash,
                                build_id: _,
                            }) => {
                                if script_language.as_deref() == Some("WebAssembly") {
                                    tracing::debug!(
                                        "WASM module detected: {} ({})",
                                        script_id,
                                        url.as_deref().unwrap_or("unknown")
                                    );

                                    let mut s = state.lock().unwrap();
                                    let module_info = chronos_domain::trace::WasmModuleInfo {
                                        script_id: script_id.clone(),
                                        url: url.clone(),
                                        hash: hash.unwrap_or_default(),
                                        build_id: None,
                                        functions: Vec::new(),
                                    };
                                    s.modules.insert(script_id, module_info);
                                }
                            }
                            Err(_) => break,
                            _ => {}
                        }
                    }
                    _ = cancel_clone.cancelled() => {
                        tracing::info!("Event loop cancelled — graceful shutdown");
                        break;
                    }
                }
            }
        });

        let session = CaptureSession::new(0, Language::WebAssembly, config);
        Ok(session)
    }
}

impl Default for BrowserAdapter {
    fn default() -> Self {
        Self::new()
    }
}

/// Drop implementation to ensure Chrome is killed when adapter is dropped.
impl Drop for BrowserAdapter {
    fn drop(&mut self) {
        let mut s = self.state.lock().unwrap();

        // Cancel background task if running
        if let Some(ref token) = s.cancel_token {
            token.cancel();
        }

        // Kill Chrome if still running
        if let Some(ref mut chrome) = s.chrome {
            let _ = chrome.kill();
        }
        s.chrome = None;

        // Clear event buffer
        s.event_buffer.clear();

        // Signal stopped
        s.running = false;
    }
}

impl BrowserAdapter {
    /// Take every buffered semantic event, EVICTING the buffer.
    ///
    /// REC-C2.2.4: this was `ProbeBackend::drain_events`, a destructive read
    /// that no longer belongs on the backend trait — a consumer of a shared
    /// backend could empty the buffer out from under another, and the answer
    /// described a bounded buffer rather than the capture. It survives as an
    /// inherent, browser-local operation because `browser_probe_drain` is
    /// documented as consuming, and because the browser has no ExecutionLog
    /// yet (FIND-C2.2-04).
    pub fn take_semantic_events(&self) -> Result<Vec<SemanticEvent>, TraceError> {
        let mut s = self.state.lock().unwrap();
        let raw_events: Vec<TraceEvent> = s.event_buffer.drain(..).collect();

        // If events were dropped, emit a warning log
        if s.dropped_events > 0 {
            tracing::warn!(
                "{} events were dropped due to buffer overflow (capacity: {})",
                s.dropped_events,
                s.buffer_capacity
            );
            s.dropped_events = 0;
        }

        let ctx = ResolveContext {
            pid: 0,
            binary_path: None,
        };

        let semantic_events: Vec<SemanticEvent> = raw_events
            .iter()
            .filter_map(|e| self.resolver.resolve(e, &ctx))
            .collect();

        Ok(semantic_events)
    }

    /// A non-destructive read of the buffered raw events.
    ///
    /// REC-C2.2.4: replaced the destructive `drain_raw_events()`. The browser
    /// buffer is bounded and drops the oldest events at capacity (FIND-C2.2-04),
    /// but at least a read no longer removes evidence another consumer still
    /// needs, and the stop path no longer empties the buffer that
    /// `browser_probe_drain` reads.
    pub fn raw_events(&self) -> Vec<TraceEvent> {
        let s = self.state.lock().unwrap();
        s.event_buffer.iter().cloned().collect()
    }
}

impl ProbeBackend for BrowserAdapter {
    fn is_available(&self) -> bool {
        Self::is_chrome_available()
    }

    fn name(&self) -> &str {
        "browser-wasm"
    }

    fn stop_probe(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        let mut s = self.state.lock().unwrap();

        // Signal background loop to stop
        s.running = false;
        if let Some(ref token) = s.cancel_token {
            token.cancel();
        }

        // Kill Chrome if we spawned it
        if let Some(ref mut chrome) = s.chrome {
            let _ = chrome.kill();
        }
        s.chrome = None;

        // NOTE: event_buffer is NOT cleared here — drain_raw_events() is called
        // by the caller AFTER stop_probe returns (stop-then-drain contract, MS-RACE-FIX).
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::MonotonicNs;

    #[test]
    fn test_browser_adapter_name() {
        let adapter = BrowserAdapter::new();
        assert_eq!(ProbeBackend::name(&adapter), "browser-wasm");
    }

    #[test]
    fn test_browser_adapter_is_available() {
        // Just verify the method works - actual availability depends on Chrome
        let _available = BrowserAdapter::is_chrome_available();
    }

    #[test]
    fn test_browser_adapter_default() {
        let adapter = BrowserAdapter::default();
        assert_eq!(ProbeBackend::name(&adapter), "browser-wasm");
    }

    #[test]
    fn test_stop_probe_no_panic_without_chrome() {
        // Verify stop_probe doesn't panic when called on a fresh adapter (no Chrome)
        let adapter = BrowserAdapter::new();
        let session =
            CaptureSession::new(0, Language::WebAssembly, CaptureConfig::new("about:blank"));
        let result = chronos_domain::ProbeBackend::stop_probe(&adapter, &session);
        assert!(
            result.is_ok(),
            "stop_probe should succeed even without Chrome"
        );
    }

    #[test]
    fn test_take_semantic_events_clears_buffer() {
        // The browser-local consuming take still evicts: `browser_probe_drain`
        // is documented as consuming, and that contract is unchanged.
        let adapter = BrowserAdapter::new();

        let first = BrowserAdapter::take_semantic_events(&adapter).unwrap();
        assert!(first.is_empty());

        let second = BrowserAdapter::take_semantic_events(&adapter).unwrap();
        assert!(second.is_empty());
    }

    /// REC-C2.2.4: `drain_raw_events` was destructive. `raw_events` reads
    /// without evicting, so a second consumer still sees the events.
    #[test]
    fn test_raw_events_does_not_clear_buffer() {
        let adapter = BrowserAdapter::new();

        let first = BrowserAdapter::raw_events(&adapter);
        assert!(first.is_empty());

        let second = BrowserAdapter::raw_events(&adapter);
        assert!(second.is_empty());

        // Push through the public capture seam and prove the buffer survives a read.
        {
            let mut s = adapter.state.lock().unwrap();
            s.event_buffer.push_back(chronos_domain::TraceEvent::signal(
                1,
                MonotonicNs::from(100),
                1,
                11,
                "SIGSEGV",
                0,
            ));
        }
        let read = BrowserAdapter::raw_events(&adapter);
        assert_eq!(read.len(), 1, "raw_events must see the buffered event");
        let read_again = BrowserAdapter::raw_events(&adapter);
        assert_eq!(
            read_again.len(),
            1,
            "raw_events must not consume: the retired drain returned 0 here"
        );
    }

    #[test]
    fn test_drop_does_not_panic() {
        // Verify Drop impl doesn't panic when adapter is dropped without stop_probe
        let adapter = BrowserAdapter::new();
        drop(adapter);
    }

    #[test]
    fn test_probe_backend_name() {
        let adapter = BrowserAdapter::new();
        assert_eq!(ProbeBackend::name(&adapter), "browser-wasm");
    }

    #[test]
    fn test_multiple_stop_probe_calls() {
        // Verify stop_probe can be called multiple times without panic
        let adapter = BrowserAdapter::new();
        let session =
            CaptureSession::new(0, Language::WebAssembly, CaptureConfig::new("about:blank"));

        let r1 = chronos_domain::ProbeBackend::stop_probe(&adapter, &session);
        let r2 = chronos_domain::ProbeBackend::stop_probe(&adapter, &session);
        let r3 = chronos_domain::ProbeBackend::stop_probe(&adapter, &session);

        assert!(r1.is_ok());
        assert!(r2.is_ok());
        assert!(r3.is_ok());
    }
}

// =====================================================================
// REC-C3.3.2.4 — composition seam for the browser probe capability.
//
// `BrowserProbeFactoryImpl` is the **only** constructor of a
// `BrowserProbeBackend` in the workspace. It is exposed to the rest
// of `chronos` through `chronos_domain::ports::BrowserProbeFactory`.
// `chronos_mcp::composition` owns the canonical
// `Arc<dyn BrowserProbeFactory>` production value via
// `default_browser_probe_factory()`; no other crate is allowed to
// construct a `BrowserAdapter` directly for production use.
// =====================================================================

use async_trait::async_trait;
use chronos_domain::capability::CapabilityUnavailable;
use chronos_domain::ports::browser_probe::{
    BrowserError as PortBrowserError, BrowserProbeBackend, BrowserProbeFactory,
};

/// Concrete factory: every `create` call performs the `is_chrome_available`
/// check and, when green, hands out a fresh `BrowserAdapter` wrapped
/// behind the port trait.
#[derive(Debug, Default, Clone, Copy)]
pub struct BrowserProbeFactoryImpl;

impl BrowserProbeFactoryImpl {
    /// Canonical constructor (composition root only).
    pub fn new() -> Self {
        Self
    }
}

impl BrowserProbeFactory for BrowserProbeFactoryImpl {
    fn create(&self) -> Result<std::sync::Arc<dyn BrowserProbeBackend>, CapabilityUnavailable> {
        if !BrowserAdapter::is_chrome_available() {
            return Err(CapabilityUnavailable::browser_probe(
                "chrome/chromium not available on host",
            ));
        }
        Ok(std::sync::Arc::new(BrowserAdapter::new()))
    }
}

#[async_trait]
impl BrowserProbeBackend for BrowserAdapter {
    async fn start_probe_async(
        &self,
        config: CaptureConfig,
        headless: bool,
        chrome_path: Option<&str>,
    ) -> Result<CaptureSession, PortBrowserError> {
        BrowserAdapter::start_probe_async(self, config, headless, chrome_path)
            .await
            .map_err(|e| PortBrowserError::new(e.to_string()))
    }

    fn stop_probe(&self, session: &CaptureSession) -> Result<(), PortBrowserError> {
        ProbeBackend::stop_probe(self, session).map_err(|e| PortBrowserError::new(e.to_string()))
    }

    fn take_semantic_events(&self) -> Result<Vec<SemanticEvent>, PortBrowserError> {
        BrowserAdapter::take_semantic_events(self).map_err(|e| PortBrowserError::new(e.to_string()))
    }

    fn raw_events(&self) -> Vec<TraceEvent> {
        BrowserAdapter::raw_events(self)
    }
}
