//! Standalone WASM module detector.
//!
//! This module provides a standalone WASM module detector that can be used
//! independently from `BrowserAdapter` for module discovery. It listens for
//! `Debugger.scriptParsed` events and maintains a registry of WebAssembly modules.
//!
//! # Example
//!
//! ```ignore
//! use chronos_browser::{BrowserCdpClient, WasmModuleDetector};
//! use std::sync::Arc;
//!
//! async fn detect_modules(cdp: BrowserCdpClient) {
//!     let detector = WasmModuleDetector::new(Arc::new(tokio::sync::Mutex::new(cdp)));
//!     let mut detector = detector;
//!     let events_rx = /* ... */;
//!     detector.start(events_rx).await.ok();
//!     for (id, module) in detector.get_modules() {
//!         println!("WASM module: {} at {:?}", id, module.url);
//!     }
//! }
//! ```
//!
//! The detector can also run in the background via [`detect_in_background`].

use crate::cdp_client::{BrowserCdpClient, CdpEvent};
use crate::error::BrowserError;
use chronos_domain::trace::{WasmFunctionInfo, WasmModuleInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info};

/// WASM module detector
///
/// Listens for `Debugger.scriptParsed` events and maintains a registry
/// of discovered WebAssembly modules.
pub struct WasmModuleDetector {
    /// Map of script_id -> WASM module info
    modules: HashMap<String, WasmModuleInfo>,
    /// CDP client for sending commands (None for dummy/test instances)
    cdp: Option<Arc<tokio::sync::Mutex<BrowserCdpClient>>>,
}

impl WasmModuleDetector {
    /// Create a new WASM module detector
    pub fn new(cdp: Arc<tokio::sync::Mutex<BrowserCdpClient>>) -> Self {
        Self {
            modules: HashMap::new(),
            cdp: Some(cdp),
        }
    }

    /// Process CDP events and collect WASM modules
    pub async fn start(&mut self, mut events_rx: broadcast::Receiver<CdpEvent>) -> Result<(), BrowserError> {
        loop {
            match events_rx.recv().await {
                Ok(event) => {
                    match event {
                        CdpEvent::DebuggerScriptParsed {
                            script_id,
                            url,
                            script_language,
                            hash,
                            build_id,
                        } => {
                            // Check if this is a WebAssembly module
                            if script_language.as_deref() == Some("WebAssembly") {
                                info!("Detected WASM module: {} ({})", script_id, url.as_deref().unwrap_or("unknown"));

                                let mut module_info = WasmModuleInfo {
                                    script_id: script_id.clone(),
                                    url,
                                    hash: hash.unwrap_or_default(),
                                    build_id,
                                    functions: Vec::new(),
                                };

                                // Try to disassemble the module to get function info
                                if let Ok(functions) = self.disassemble_module(&script_id).await {
                                    module_info.functions = functions;
                                }

                                self.modules.insert(script_id, module_info);
                            }
                        }
                        CdpEvent::InspectorDetached | CdpEvent::DebuggerResumed => {
                            debug!("Debugger detached or resumed, stopping WASM detection");
                            break;
                        }
                        _ => {}
                    }
                }
                Err(broadcast::error::RecvError::Closed) => {
                    debug!("Event channel closed, stopping WASM detection");
                    break;
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    debug!("Missed some events, continuing...");
                }
            }
        }

        Ok(())
    }

    /// Disassemble a WASM module to get function information
    async fn disassemble_module(&self, script_id: &str) -> Result<Vec<WasmFunctionInfo>, BrowserError> {
        let cdp = self.cdp.as_ref().ok_or_else(|| BrowserError::CdpConnectionFailed("No CDP connection".into()))?.lock().await;
        let entries = cdp.disassemble_wasm_module(script_id).await?;

        let functions: Vec<WasmFunctionInfo> = entries
            .into_iter()
            .enumerate()
            .map(|(idx, entry)| WasmFunctionInfo {
                function_index: idx,
                name: None, // Names will be resolved separately
                body_start: entry.start,
                body_end: entry.end,
                breakpoint_id: None,
            })
            .collect();

        Ok(functions)
    }

    /// Get all detected WASM modules
    pub fn get_modules(&self) -> &HashMap<String, WasmModuleInfo> {
        &self.modules
    }

    /// Get a specific WASM module by script ID
    pub fn get_module(&self, script_id: &str) -> Option<&WasmModuleInfo> {
        self.modules.get(script_id)
    }

    /// Get the number of detected modules
    pub fn module_count(&self) -> usize {
        self.modules.len()
    }

    /// Check if any WASM modules have been detected
    pub fn has_modules(&self) -> bool {
        !self.modules.is_empty()
    }

    /// Spawn a background task that detects WASM modules.
    ///
    /// Returns a `JoinHandle` that can be used to await the result.
    /// The detector will listen for WASM module events and collect them.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let cdp = Arc::new(tokio::sync::Mutex::new(cdp_client));
    /// let handle = WasmModuleDetector::detect_in_background(cdp.clone());
    ///
    /// // ... use Chrome to load WASM modules ...
    ///
    /// let modules = handle.await.ok();
    /// ```
    pub fn detect_in_background(
        cdp: Arc<tokio::sync::Mutex<BrowserCdpClient>>,
    ) -> tokio::task::JoinHandle<HashMap<String, WasmModuleInfo>> {
        let cdp_for_detector = cdp.clone();
        tokio::spawn(async move {
            // First subscribe to get the receiver
            let events_rx = {
                let cdp = cdp_for_detector.lock().await;
                cdp.subscribe()
            };

            // Create detector with the CDP client
            let mut detector = WasmModuleDetector::new(cdp_for_detector);

            // Run detection
            let _ = detector.start(events_rx).await;

            // Return detected modules
            detector.get_modules().clone()
        })
    }

    /// Get all function info across all modules
    pub fn get_all_functions(&self) -> Vec<(String, WasmFunctionInfo)> {
        let mut result: Vec<(String, WasmFunctionInfo)> = Vec::new();
        for (script_id, module) in &self.modules {
            for func in &module.functions {
                result.push((script_id.clone(), func.clone()));
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_module_info_empty() {
        let detector = WasmModuleDetector {
            modules: HashMap::new(),
            cdp: None,
        };

        assert!(!detector.has_modules());
        assert_eq!(detector.module_count(), 0);
        assert!(detector.get_module("nonexistent").is_none());
    }

    #[test]
    fn test_wasm_function_info() {
        let func_info = WasmFunctionInfo {
            function_index: 0,
            name: Some("add".to_string()),
            body_start: 0,
            body_end: 100,
            breakpoint_id: None,
        };

        assert_eq!(func_info.function_index, 0);
        assert_eq!(func_info.name.as_deref(), Some("add"));
        assert_eq!(func_info.body_start, 0);
        assert_eq!(func_info.body_end, 100);
        assert!(func_info.breakpoint_id.is_none());
    }
}
