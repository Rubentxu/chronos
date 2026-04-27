//! WASM function mapping and breakpoint management.
//!
//! Handles mapping of WASM function indices to breakpoints and manages
//! the breakpoint state for WASM debugging sessions.

use crate::cdp_client::BrowserCdpClient;
use crate::error::BrowserError;
use chronos_domain::trace::WasmFunctionInfo;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Information about a function probe
#[derive(Debug, Clone)]
pub struct WasmFunctionProbe {
    /// The WASM function info
    pub function: WasmFunctionInfo,
    /// The CDP breakpoint ID
    pub breakpoint_id: String,
    /// The script ID this probe is set in
    pub script_id: String,
}

/// WASM breakpoint manager
///
/// Manages breakpoints for WASM functions across all loaded modules.
pub struct WasmBreakpointManager {
    /// Map of breakpoint_id -> function probe info
    breakpoints: HashMap<String, WasmFunctionProbe>,
    /// Map of function_index -> breakpoint_id (for quick lookup)
    function_breakpoints: HashMap<String, HashMap<usize, String>>,
    /// CDP client for sending commands (None for dummy/test instances)
    cdp: Option<Arc<Mutex<BrowserCdpClient>>>,
}

impl WasmBreakpointManager {
    /// Create a new WASM breakpoint manager
    pub fn new(cdp: Arc<Mutex<BrowserCdpClient>>) -> Self {
        Self {
            breakpoints: HashMap::new(),
            function_breakpoints: HashMap::new(),
            cdp: Some(cdp),
        }
    }

    /// Create a dummy WASM breakpoint manager for testing or event conversion
    /// without an active CDP connection. Methods that require CDP will return errors.
    pub fn new_dummy() -> Self {
        Self {
            breakpoints: HashMap::new(),
            function_breakpoints: HashMap::new(),
            cdp: None,
        }
    }

    /// Probe all functions in a WASM module
    ///
    /// Sets breakpoints at the start of each function body.
    pub async fn probe_module(
        &mut self,
        script_id: &str,
        functions: &[WasmFunctionInfo],
    ) -> Result<(), BrowserError> {
        for func in functions {
            self.probe_function(script_id, func).await?;
        }
        Ok(())
    }

    /// Probe a single WASM function
    ///
    /// Sets a breakpoint at the function body's start offset.
    ///
    /// # WASM Byte Offset Encoding
    ///
    /// CDP uses a special encoding for WASM offsets: the `line_number` in location
    /// represents the byte offset within the WASM module's code section, NOT a
    /// source line number. This is why we set breakpoints using `body_start` as
    /// the line number and 0 as the column — CDP interprets this as:
    /// - line_number = byte offset in WASM code section
    /// - column_number = 0 (always, for WASM)
    pub async fn probe_function(
        &mut self,
        script_id: &str,
        func: &WasmFunctionInfo,
    ) -> Result<String, BrowserError> {
        // Check if we already have a breakpoint for this function
        let script_breakpoints = self.function_breakpoints.entry(script_id.to_string()).or_default();
        if let Some(existing_bp) = script_breakpoints.get(&func.function_index) {
            return Ok(existing_bp.clone());
        }

        let cdp = self.cdp.as_ref().ok_or_else(|| BrowserError::BreakpointError("No CDP connection".into()))?.lock().await;

        // Set breakpoint at the function body's start
        // For WASM, we use line 0 with the function body offset as column
        let breakpoint_id = cdp
            .set_breakpoint(script_id, func.body_start, Some(0))
            .await?;

        // Store the breakpoint info
        let probe = WasmFunctionProbe {
            function: func.clone(),
            breakpoint_id: breakpoint_id.clone(),
            script_id: script_id.to_string(),
        };

        self.breakpoints.insert(breakpoint_id.clone(), probe);
        script_breakpoints.insert(func.function_index, breakpoint_id.clone());

        info!(
            "Set breakpoint {} for function {} in script {}",
            breakpoint_id,
            func.function_index,
            script_id
        );

        Ok(breakpoint_id)
    }

    /// Remove all breakpoints
    pub async fn remove_all(&mut self) -> Result<(), BrowserError> {
        if let Some(cdp) = &self.cdp {
            let cdp = cdp.lock().await;
            for (breakpoint_id, _) in &self.breakpoints {
                cdp.remove_breakpoint(breakpoint_id).await?;
            }
        }

        self.breakpoints.clear();
        self.function_breakpoints.clear();

        debug!("Removed all WASM breakpoints");
        Ok(())
    }

    /// Get the function probe info for a breakpoint
    pub fn get_function_for_breakpoint(&self, breakpoint_id: &str) -> Option<&WasmFunctionProbe> {
        self.breakpoints.get(breakpoint_id)
    }

    /// Get all active breakpoints
    pub fn get_breakpoints(&self) -> &HashMap<String, WasmFunctionProbe> {
        &self.breakpoints
    }

    /// Get the number of active breakpoints
    pub fn breakpoint_count(&self) -> usize {
        self.breakpoints.len()
    }

    /// Check if any breakpoints are set
    pub fn has_breakpoints(&self) -> bool {
        !self.breakpoints.is_empty()
    }

    /// Remove a specific breakpoint
    pub async fn remove_breakpoint(&mut self, breakpoint_id: &str) -> Result<(), BrowserError> {
        if let Some(probe) = self.breakpoints.remove(breakpoint_id) {
            if let Some(cdp) = &self.cdp {
                let cdp = cdp.lock().await;
                cdp.remove_breakpoint(breakpoint_id).await?;
            }

            // Also remove from function_breakpoints
            if let Some(script_bps) = self.function_breakpoints.get_mut(&probe.script_id) {
                script_bps.remove(&probe.function.function_index);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_function_probe() {
        let probe = WasmFunctionProbe {
            function: WasmFunctionInfo {
                function_index: 0,
                name: Some("add".to_string()),
                body_start: 0,
                body_end: 100,
                breakpoint_id: None,
            },
            breakpoint_id: "bp-1".to_string(),
            script_id: "script-1".to_string(),
        };

        assert_eq!(probe.breakpoint_id, "bp-1");
        assert_eq!(probe.script_id, "script-1");
        assert_eq!(probe.function.function_index, 0);
    }

    #[test]
    fn test_wasm_breakpoint_manager_empty() {
        let manager = WasmBreakpointManager::new_dummy();

        assert!(!manager.has_breakpoints());
        assert_eq!(manager.breakpoint_count(), 0);
        assert!(manager.get_function_for_breakpoint("nonexistent").is_none());
    }
}
