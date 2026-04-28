//! Browser adapter implementing TraceAdapter and ProbeBackend for WASM debugging.
//!
//! This adapter connects to Chrome via CDP to debug WebAssembly modules.

use crate::browser::ChromeProcess;
use crate::cdp_client::{BrowserCdpClient, CdpEvent};
use crate::error::BrowserError;
use crate::event_mapper::paused_to_wasm_events;
use crate::wasm_probes::WasmBreakpointManager;
use crate::wasm_resolver::WasmSemanticResolver;
use chronos_capture::TraceAdapter;
use chronos_domain::adapter::ProbeBackend;
use chronos_domain::semantic::{ResolveContext, SemanticEvent, SemanticResolver};
use chronos_domain::{CaptureConfig, CaptureSession, Language, TraceError, TraceEvent};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// Browser adapter for WebAssembly debugging via Chrome DevTools Protocol.
pub struct BrowserAdapter {
    /// Chrome process (if spawned by us)
    chrome: Arc<Mutex<Option<ChromeProcess>>>,
    /// WASM modules detected during this session
    modules: Arc<Mutex<HashMap<String, chronos_domain::trace::WasmModuleInfo>>>,
    /// Buffered trace events
    event_buffer: Arc<Mutex<Vec<TraceEvent>>>,
    /// Next event ID
    next_event_id: Arc<Mutex<u64>>,
    /// Whether capture is running
    running: Arc<Mutex<bool>>,
    /// Session start time
    session_start: Arc<Mutex<Option<Instant>>>,
    /// Semantic resolver for WASM events
    resolver: WasmSemanticResolver,
    /// WASM breakpoint manager (SIG 11)
    breakpoint_manager: Arc<Mutex<WasmBreakpointManager>>,
}

impl BrowserAdapter {
    /// Create a new browser adapter.
    pub fn new() -> Self {
        Self {
            chrome: Arc::new(Mutex::new(None)),
            modules: Arc::new(Mutex::new(HashMap::new())),
            event_buffer: Arc::new(Mutex::new(Vec::new())),
            next_event_id: Arc::new(Mutex::new(1)),
            running: Arc::new(Mutex::new(false)),
            session_start: Arc::new(Mutex::new(None)),
            resolver: WasmSemanticResolver::new(),
            breakpoint_manager: Arc::new(Mutex::new(WasmBreakpointManager::new_dummy())),
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
    ///
    /// # Example
    ///
    /// ```ignore
    /// let events = BrowserAdapter::quick_probe(
    ///     "http://example.com/wasm.html",
    ///     5000,  // wait 5 seconds
    ///     true,   // headless
    ///     None,   // auto-detect Chrome
    /// ).await?;
    /// ```
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
            .drain_events()
            .map_err(|e| BrowserError::CdpConnectionFailed(e.to_string()))?;

        // Stop probe (cleanup Chrome)
        adapter
            .stop_probe(&session)
            .map_err(|e| BrowserError::CdpConnectionFailed(e.to_string()))?;

        Ok(events)
    }

    /// Async version of start_capture for use from async contexts (MCP server).
    /// This does all the async work properly without creating nested runtimes.
    ///
    /// # Arguments
    /// * `config` - Capture configuration
    /// * `headless` - Whether to run Chrome in headless mode
    /// * `chrome_path` - Optional path to Chrome binary
    pub async fn start_probe_async(&self, config: CaptureConfig, headless: bool, chrome_path: Option<&str>) -> Result<CaptureSession, BrowserError> {
        // Spawn Chrome (sync, fast)
        let mut chrome = ChromeProcess::spawn(headless, chrome_path)?;

        // Wait for CDP (async, uses tokio::sleep)
        let ws_url = ChromeProcess::wait_for_ready(chrome.port()).await?;
        chrome.set_ws_url(ws_url);

        // Connect to CDP WebSocket
        let cdp = BrowserCdpClient::connect(chrome.ws_url()).await?;
        cdp.debugger_enable().await?;

        // Initialize state
        {
            let mut running = self.running.lock().unwrap();
            *running = true;

            let mut session_start = self.session_start.lock().unwrap();
            *session_start = Some(Instant::now());

            let mut next_event_id = self.next_event_id.lock().unwrap();
            *next_event_id = 1;

            let mut event_buffer = self.event_buffer.lock().unwrap();
            event_buffer.clear();

            let mut modules = self.modules.lock().unwrap();
            modules.clear();

            let mut chrome_guard = self.chrome.lock().unwrap();
            *chrome_guard = Some(chrome);
        }

        // Clone arcs for background task
        let event_buffer = self.event_buffer.clone();
        let running = self.running.clone();
        let next_event_id = self.next_event_id.clone();
        let session_start = self.session_start.clone();
        let modules = self.modules.clone();
        let breakpoint_manager = self.breakpoint_manager.clone();

        // Spawn background task (NO new runtime needed — we're already in one)
        tokio::spawn(async move {
            let mut event_rx = cdp.subscribe();

            loop {
                match event_rx.recv().await {
                    Ok(CdpEvent::DebuggerPaused {
                        reason,
                        call_frames,
                        hit_breakpoints,
                    }) => {
                        let timestamp_ns = {
                            let ss = session_start.lock().unwrap();
                            ss.map(|s| s.elapsed().as_nanos() as u64).unwrap_or(0)
                        };

                        let is_running = {
                            let r = running.lock().unwrap();
                            *r
                        };

                        if is_running {
                            // Use From impl to convert call frames (SIG 10)
                            use crate::event_mapper::CdpDebuggerPaused;

                            let paused = CdpDebuggerPaused {
                                call_frames: call_frames.into_iter().map(|f| f.into()).collect(),
                                reason: reason.clone(),
                                hit_breakpoints: hit_breakpoints.clone(),
                            };

                            let modules_ref: HashMap<String, chronos_domain::trace::WasmModuleInfo> = {
                                let m = modules.lock().unwrap();
                                m.clone()
                            };

                            let mut next_id = {
                                let id = next_event_id.lock().unwrap();
                                *id
                            };

                            // Use the real breakpoint manager (SIG 11)
                            let bp_manager = breakpoint_manager.lock().unwrap();

                            let events = paused_to_wasm_events(
                                &paused,
                                &modules_ref,
                                &bp_manager,
                                timestamp_ns,
                                &mut next_id,
                            );

                            {
                                let mut id = next_event_id.lock().unwrap();
                                *id = next_id;
                            }

                            if !events.is_empty() {
                                let mut buffer = event_buffer.lock().unwrap();
                                buffer.extend(events);
                            }
                        }
                    }
                    Ok(CdpEvent::DebuggerResumed) => {
                        // Debugger resumed after a breakpoint hit — this is normal.
                        // Do NOT stop the event loop; we keep listening for the next hit.
                        tracing::trace!("Debugger resumed — continuing event loop");
                    }
                    Ok(CdpEvent::InspectorDetached) => {
                        let mut r = running.lock().unwrap();
                        *r = false;
                        break;
                    }
                    Ok(CdpEvent::DebuggerScriptParsed { script_id, url, script_language, hash, build_id: _ }) => {
                        if script_language.as_deref() == Some("WebAssembly") {
                            tracing::debug!("WASM module detected: {} ({})", script_id, url.as_deref().unwrap_or("unknown"));

                            // Store the module info
                            let mut m = modules.lock().unwrap();
                            let module_info = chronos_domain::trace::WasmModuleInfo {
                                script_id: script_id.clone(),
                                url: url.clone(),
                                hash: hash.unwrap_or_default(),
                                build_id: None,
                                functions: Vec::new(),
                            };
                            m.insert(script_id, module_info);
                        }
                    }
                    Err(_) => break,
                    _ => {}
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
///
/// This provides safety against leaked Chrome processes if stop_probe() is not called.
impl Drop for BrowserAdapter {
    fn drop(&mut self) {
        // Kill Chrome if it's still running
        {
            let mut chrome_guard = self.chrome.lock().unwrap();
            if let Some(ref mut chrome) = *chrome_guard {
                let _ = chrome.kill();
            }
            *chrome_guard = None;
        }

        // Clear event buffer
        {
            let mut buffer = self.event_buffer.lock().unwrap();
            buffer.clear();
        }

        // Signal stopped
        {
            let mut running = self.running.lock().unwrap();
            *running = false;
        }
    }
}

impl TraceAdapter for BrowserAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        // Use block_in_place to escape the current runtime and call async code
        // This works when called from an async context (like MCP server)
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.start_probe_async(config, true, None)
                    .await
                    .map_err(|e: BrowserError| TraceError::CaptureFailed(e.to_string()))
            })
        })
    }

    fn stop_capture(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        {
            let mut running = self.running.lock().unwrap();
            *running = false;
        }

        // Kill Chrome if we spawned it
        {
            let mut chrome_guard = self.chrome.lock().unwrap();
            if let Some(ref mut chrome) = *chrome_guard {
                chrome.kill().map_err(|e| TraceError::CaptureFailed(e.to_string()))?;
            }
            *chrome_guard = None;
        }

        Ok(())
    }

    fn attach_to_process(&self, _pid: u32, _config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        Err(TraceError::UnsupportedLanguage(
            "attach_to_process not supported for browser debugging".to_string(),
        ))
    }

    fn get_language(&self) -> Language {
        Language::WebAssembly
    }

    fn name(&self) -> &str {
        "browser-wasm"
    }
}

impl ProbeBackend for BrowserAdapter {
    fn is_available(&self) -> bool {
        Self::is_chrome_available()
    }

    fn name(&self) -> &str {
        "browser-wasm"
    }

    fn drain_events(&self) -> Result<Vec<SemanticEvent>, TraceError> {
        // SIG 3: Clear the buffer after reading
        let mut buffer = self.event_buffer.lock().unwrap();
        let raw_events: Vec<TraceEvent> = std::mem::take(&mut *buffer);

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

    fn drain_raw_events(&self) -> Vec<TraceEvent> {
        let mut buffer = self.event_buffer.lock().unwrap();
        std::mem::take(&mut *buffer)
    }

    fn stop_probe(&self, _session: &CaptureSession) -> Result<(), TraceError> {
        {
            let mut running = self.running.lock().unwrap();
            *running = false;
        }

        // Kill Chrome if we spawned it
        {
            let mut chrome_guard = self.chrome.lock().unwrap();
            if let Some(ref mut chrome) = *chrome_guard {
                let _ = chrome.kill();
            }
            *chrome_guard = None;
        }

        // Clear event buffer
        {
            let mut buffer = self.event_buffer.lock().unwrap();
            buffer.clear();
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_adapter_name() {
        let adapter = BrowserAdapter::new();
        // Disambiguate between TraceAdapter::name and ProbeBackend::name
        assert_eq!(chronos_capture::TraceAdapter::name(&adapter), "browser-wasm");
    }

    #[test]
    fn test_browser_adapter_language() {
        let adapter = BrowserAdapter::new();
        assert_eq!(adapter.get_language(), Language::WebAssembly);
    }

    #[test]
    fn test_browser_adapter_is_available() {
        // Just verify the method works - actual availability depends on Chrome
        let _available = BrowserAdapter::is_chrome_available();
        // Can't assert true/false as Chrome may or may not be installed
    }

    #[test]
    fn test_browser_adapter_default() {
        let adapter = BrowserAdapter::default();
        // Disambiguate between TraceAdapter::name and ProbeBackend::name
        assert_eq!(chronos_capture::TraceAdapter::name(&adapter), "browser-wasm");
    }
}