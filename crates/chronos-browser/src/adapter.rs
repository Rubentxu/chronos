//! Browser adapter implementing TraceAdapter and ProbeBackend for WASM debugging.
//!
//! This adapter connects to Chrome via CDP to debug WebAssembly modules.

use crate::browser::ChromeProcess;
use crate::cdp_client::{BrowserCdpClient, CdpEvent};
use crate::event_mapper::{paused_to_wasm_events, CdpDebuggerPaused};
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
        }
    }

    /// Check if Chrome is available on the system.
    pub fn is_chrome_available() -> bool {
        ChromeProcess::attach_port(9222).is_ok()
    }

    /// Internal helper to convert CDP call frames to our internal format.
    fn convert_paused_event(
        call_frames: Vec<crate::cdp_client::WasmCallFrame>,
        reason: String,
        hit_breakpoints: Vec<String>,
    ) -> CdpDebuggerPaused {
        CdpDebuggerPaused {
            call_frames: call_frames
                .into_iter()
                .map(|f| crate::event_mapper::CdpCallFrame {
                    function_name: f.function_name,
                    location: crate::event_mapper::CdpLocation {
                        script_id: f.function_location.as_ref().map(|l| l.script_id.clone()).unwrap_or_default(),
                        line_number: f.function_location.as_ref().map(|l| l.line_number as i64).unwrap_or(0),
                        column_number: f.function_location.as_ref().and_then(|l| l.column_number.map(|c| c as i64)),
                    },
                    scope_chain: f.scope_chain
                        .into_iter()
                        .map(|s| crate::event_mapper::CdpScope {
                            scope_type: s.type_,
                            object: Some(crate::event_mapper::CdpRemoteObject {
                                type_: s.object.type_,
                                subtype: s.object.subtype,
                                class_name: s.object.class_name,
                                value: s.object.value,
                                description: s.object.description,
                                object_id: s.object.object_id,
                            }),
                        })
                        .collect(),
                })
                .collect(),
            reason,
            hit_breakpoints,
        }
    }
}

impl Default for BrowserAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceAdapter for BrowserAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        // Spawn Chrome headless and connect
        let chrome = ChromeProcess::spawn(true).map_err(|e| {
            TraceError::CaptureFailed(format!("Failed to spawn Chrome: {}", e))
        })?;

        let rt = tokio::runtime::Runtime::new().map_err(|e| {
            TraceError::CaptureFailed(format!("Failed to create runtime: {}", e))
        })?;

        // Connect to CDP WebSocket
        let cdp = rt.block_on(async {
            BrowserCdpClient::connect(chrome.ws_url())
                .await
                .map_err(|e| TraceError::CaptureFailed(format!("CDP connection failed: {}", e)))
        })?;

        // Enable debugger
        rt.block_on(async {
            cdp.debugger_enable().await.map_err(|e| {
                TraceError::CaptureFailed(format!("Failed to enable debugger: {}", e))
            })
        })?;

        // Create session
        let session = CaptureSession::new(0, Language::WebAssembly, config.clone());

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

        // Clone arcs for the background task
        let event_buffer = self.event_buffer.clone();
        let running = self.running.clone();
        let next_event_id = self.next_event_id.clone();
        let session_start = self.session_start.clone();
        let modules = self.modules.clone();

        // Spawn background task to handle CDP events
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            let mut event_rx = rt.block_on(async { cdp.subscribe() });

            rt.block_on(async {
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
                                let paused = BrowserAdapter::convert_paused_event(call_frames, reason.clone(), hit_breakpoints.clone());

                                let modules_ref: HashMap<String, chronos_domain::trace::WasmModuleInfo> = {
                                    let m = modules.lock().unwrap();
                                    m.clone()
                                };

                                // Create a minimal breakpoint manager for event conversion
                                // Note: We can't access the real CDP client here, so we use a placeholder approach
                                let mut next_id = {
                                    let id = next_event_id.lock().unwrap();
                                    *id
                                };

                                // For now, create a dummy breakpoint manager
                                // The event conversion will use default breakpoint types
                                let dummy_bp_manager = crate::wasm_probes::WasmBreakpointManager::new_dummy();

                                let events = paused_to_wasm_events(
                                    &paused,
                                    &modules_ref,
                                    &dummy_bp_manager,
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
                            let mut r = running.lock().unwrap();
                            *r = false;
                            break;
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
        });

        Ok(session)
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
        "browser-cdp"
    }
}

impl ProbeBackend for BrowserAdapter {
    fn is_available(&self) -> bool {
        Self::is_chrome_available()
    }

    fn name(&self) -> &str {
        "browser-cdp"
    }

    fn drain_events(&self) -> Result<Vec<SemanticEvent>, TraceError> {
        let event_buffer = self.event_buffer.lock().unwrap();

        let ctx = ResolveContext {
            pid: 0,
            binary_path: None,
        };

        let semantic_events: Vec<SemanticEvent> = event_buffer
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
        assert_eq!(chronos_capture::TraceAdapter::name(&adapter), "browser-cdp");
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
        assert_eq!(chronos_capture::TraceAdapter::name(&adapter), "browser-cdp");
    }
}