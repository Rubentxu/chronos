//! CDP WebSocket client for browser-based WASM debugging.
//!
//! This client connects to Chrome's DevTools Protocol to debug WebAssembly modules.

use crate::error::BrowserError;
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, warn};

// ============================================================================
// CDP Event Types
// ============================================================================

/// CDP event types we care about for WASM debugging
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum CdpEventType {
    #[serde(rename = "Debugger.paused")]
    DebuggerPaused(DebuggerPausedParams),

    #[serde(rename = "Debugger.resumed")]
    DebuggerResumed,

    #[serde(rename = "Debugger.scriptParsed")]
    DebuggerScriptParsed(ScriptParsedParams),

    #[serde(rename = "Runtime.executionContextCreated")]
    RuntimeExecutionContextCreated(ExecutionContextCreatedParams),

    #[serde(rename = "Inspector.detached")]
    InspectorDetached,

    /// Catch-all for other events
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebuggerPausedParams {
    pub reason: String,
    #[serde(default, rename = "hitBreakpoints")]
    pub hit_breakpoints: Vec<String>,
    #[serde(rename = "callFrames")]
    pub call_frames: Vec<WasmCallFrame>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptParsedParams {
    pub script_id: String,
    pub url: Option<String>,
    pub script_language: Option<String>,
    #[serde(default)]
    pub hash: Option<String>,
    #[serde(default)]
    pub build_id: Option<String>,
    #[serde(default)]
    pub debug_symbols: Option<DebugSymbols>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebugSymbols {
    #[serde(rename = "type")]
    pub type_: String,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionContextCreatedParams {
    pub context: ExecutionContext,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionContext {
    pub id: i64,
    pub origin: String,
    pub name: String,
}

/// A call frame from CDP for WASM
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WasmCallFrame {
    pub call_frame_id: String,
    pub function_name: String,
    pub function_location: Option<Location>,
    pub url: String,
    pub line_number: u32,
    pub column_number: u32,
    pub scope_chain: Vec<Scope>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Location {
    pub script_id: String,
    pub line_number: u32,
    pub column_number: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Scope {
    #[serde(rename = "type")]
    pub type_: String,
    pub object: RemoteObject,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RemoteObject {
    #[serde(rename = "type")]
    pub type_: String,
    pub subtype: Option<String>,
    pub class_name: Option<String>,
    pub value: Option<Value>,
    pub description: Option<String>,
    pub object_id: Option<String>,
}

/// CDP response wrapper
#[derive(Debug, Clone, Deserialize)]
pub struct CdpResponse {
    pub id: u64,
    #[serde(default)]
    pub result: Value,
    #[serde(default)]
    pub error: Option<CdpError>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CdpError {
    pub code: i64,
    pub message: String,
}

impl From<WasmCallFrame> for crate::event_mapper::CdpCallFrame {
    fn from(frame: WasmCallFrame) -> Self {
        Self {
            function_name: frame.function_name,
            location: crate::event_mapper::CdpLocation {
                script_id: frame.function_location.as_ref().map(|l| l.script_id.clone()).unwrap_or_default(),
                line_number: frame.function_location.as_ref().map(|l| l.line_number as i64).unwrap_or(0),
                column_number: frame.function_location.as_ref().and_then(|l| l.column_number.map(|c| c as i64)),
            },
            scope_chain: frame.scope_chain.into_iter().map(|s| {
                crate::event_mapper::CdpScope {
                    scope_type: s.type_,
                    object: Some(crate::event_mapper::CdpRemoteObject {
                        type_: s.object.type_,
                        subtype: s.object.subtype,
                        class_name: s.object.class_name,
                        value: s.object.value,
                        description: s.object.description,
                        object_id: s.object.object_id,
                    }),
                }
            }).collect(),
        }
    }
}

/// Wrapper around CDP event for our use
#[derive(Debug, Clone)]
pub enum CdpEvent {
    DebuggerPaused {
        reason: String,
        call_frames: Vec<WasmCallFrame>,
        hit_breakpoints: Vec<String>,
    },
    DebuggerResumed,
    DebuggerScriptParsed {
        script_id: String,
        url: Option<String>,
        script_language: Option<String>,
        hash: Option<String>,
        build_id: Option<String>,
    },
    RuntimeExecutionContextCreated {
        context_id: i64,
        origin: String,
        name: String,
    },
    InspectorDetached,
    Other,
}

// ============================================================================
// BrowserCdpClient
// ============================================================================

/// CDP WebSocket client for browser-based debugging
pub struct BrowserCdpClient {
    ws_write: mpsc::Sender<String>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    event_tx: broadcast::Sender<CdpEvent>,
    pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>>,
}

impl BrowserCdpClient {
    /// Connect to a CDP WebSocket endpoint
    pub async fn connect(url: &str) -> Result<Self, BrowserError> {
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| BrowserError::CdpConnectionFailed(e.to_string()))?;

        let (mut write, mut read) = ws_stream.split();

        let next_id = Arc::new(std::sync::atomic::AtomicU64::new(1));
        let (event_tx, _) = broadcast::channel(100);
        let event_tx_clone = event_tx.clone();
        let (ws_write, mut ws_read) = mpsc::channel::<String>(100);
        let pending: Arc<Mutex<HashMap<u64, oneshot::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let pending_clone = pending.clone();

        // Spawn message loop
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Handle incoming WebSocket messages
                    msg = read.next() => {
                        match msg {
                            Some(Ok(Message::Text(text))) => {
                                debug!("CDP recv: {}", &text[..text.len().min(200)]);
                                match serde_json::from_str::<Value>(&text) {
                                    Ok(json) => {
                                        // Check if it's an event or response
                                        if json.get("method").is_some() {
                                            // It's an event
                                            match serde_json::from_value::<CdpEventType>(json.clone()) {
                                                Ok(event) => {
                                                    let cdp_event = match event {
                                                        CdpEventType::DebuggerPaused(params) => {
                                                            CdpEvent::DebuggerPaused {
                                                                reason: params.reason,
                                                                call_frames: params.call_frames,
                                                                hit_breakpoints: params.hit_breakpoints,
                                                            }
                                                        }
                                                        CdpEventType::DebuggerResumed => CdpEvent::DebuggerResumed,
                                                        CdpEventType::DebuggerScriptParsed(params) => {
                                                            CdpEvent::DebuggerScriptParsed {
                                                                script_id: params.script_id,
                                                                url: params.url,
                                                                script_language: params.script_language,
                                                                hash: params.hash,
                                                                build_id: params.build_id,
                                                            }
                                                        }
                                                        CdpEventType::RuntimeExecutionContextCreated(params) => {
                                                            CdpEvent::RuntimeExecutionContextCreated {
                                                                context_id: params.context.id,
                                                                origin: params.context.origin,
                                                                name: params.context.name,
                                                            }
                                                        }
                                                        CdpEventType::InspectorDetached => CdpEvent::InspectorDetached,
                                                        CdpEventType::Other => CdpEvent::Other,
                                                    };
                                                    let _ = event_tx_clone.send(cdp_event);
                                                }
                                                Err(e) => {
                                                    warn!("Failed to parse CDP event: {}", e);
                                                }
                                            }
                                        } else if let Some(id) = json.get("id").and_then(|v| v.as_u64()) {
                                            // It's a response - check if anyone is waiting
                                            let mut pending = pending_clone.lock().await;
                                            if let Some(sender) = pending.remove(&id) {
                                                let _ = sender.send(json.clone());
                                            }
                                        }
                                    }
                                    Err(e) => {
                                        warn!("Failed to parse CDP message: {}", e);
                                    }
                                }
                            }
                            Some(Ok(Message::Close(_))) | None => {
                                debug!("CDP WebSocket closed");
                                break;
                            }
                            Some(Err(e)) => {
                                error!("CDP WebSocket error: {}", e);
                                break;
                            }
                            _ => {}
                        }
                    }
                    // Handle outgoing messages
                    msg = ws_read.recv() => {
                        if let Some(text) = msg {
                            if let Err(e) = write.send(Message::Text(text)).await {
                                error!("Failed to send CDP message: {}", e);
                                break;
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            ws_write,
            next_id,
            event_tx,
            pending,
        })
    }

    /// Send a CDP command and wait for response
    pub async fn send_command(&self, method: &str, params: Value) -> Result<Value, BrowserError> {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let msg = serde_json::json!({
            "id": id,
            "method": method,
            "params": params
        });

        debug!("CDP send: {} id={}", method, id);

        // Create a oneshot channel for this request
        let (tx, rx) = oneshot::channel::<Value>();

        // Register the response handler
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        // Send the message
        self.ws_write
            .send(msg.to_string())
            .await
            .map_err(|_| BrowserError::CdpConnectionFailed("Failed to send message".into()))?;

        // Wait for response with timeout
        let response = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .map_err(|_| BrowserError::Timeout("CDP command timeout".into()))?
            .map_err(|_| BrowserError::CdpConnectionFailed("Channel closed".into()))?;

        // Check for CDP error in response
        if let Some(error_obj) = response.get("error") {
            let error_msg = error_obj
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown error");
            return Err(BrowserError::CdpCommandError {
                method: method.to_string(),
                message: error_msg.to_string(),
            });
        }

        Ok(response)
    }

    /// Subscribe to CDP events
    pub fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.event_tx.subscribe()
    }

    /// Enable the debugger domain
    pub async fn debugger_enable(&self) -> Result<(), BrowserError> {
        self.send_command("Debugger.enable", serde_json::json!({}))
            .await?;
        Ok(())
    }

    /// Disable the debugger domain
    pub async fn debugger_disable(&self) -> Result<(), BrowserError> {
        self.send_command("Debugger.disable", serde_json::json!({}))
            .await?;
        Ok(())
    }

    /// Resume execution
    pub async fn debugger_resume(&self) -> Result<(), BrowserError> {
        self.send_command("Debugger.resume", serde_json::json!({}))
            .await?;
        Ok(())
    }

    /// Set a breakpoint in a script
    pub async fn set_breakpoint(
        &self,
        script_id: &str,
        line_number: u32,
        column_number: Option<u32>,
    ) -> Result<String, BrowserError> {
        let mut params = serde_json::json!({
            "scriptId": script_id,
            "lineNumber": line_number,
        });

        if let Some(col) = column_number {
            params["columnNumber"] = serde_json::json!(col);
        }

        let response = self.send_command("Debugger.setBreakpoint", params).await?;

        let breakpoint_id = response
            .get("result")
            .and_then(|r| r.get("breakpointId"))
            .and_then(|b| b.as_str())
            .ok_or_else(|| BrowserError::BreakpointError("Failed to get breakpoint ID".into()))?
            .to_string();

        Ok(breakpoint_id)
    }

    /// Remove a breakpoint
    pub async fn remove_breakpoint(&self, breakpoint_id: &str) -> Result<(), BrowserError> {
        self.send_command(
            "Debugger.removeBreakpoint",
            serde_json::json!({
                "breakpointId": breakpoint_id
            }),
        )
        .await?;
        Ok(())
    }

    /// Disassemble a WASM module to get function body offsets
    pub async fn disassemble_wasm_module(
        &self,
        script_id: &str,
    ) -> Result<Vec<WasmDisassemblyEntry>, BrowserError> {
        let response = self
            .send_command(
                "Debugger.disassembleWasmModule",
                serde_json::json!({
                    "scriptId": script_id
                }),
            )
            .await?;

        let result = response
            .get("result")
            .ok_or_else(|| BrowserError::BreakpointError("No result in disassembly response".into()))?;

        // Parse the function body offsets from the response
        // Format: { functionBodyOffsets: [start1, end1, start2, end2, ...], ... }
        let offsets = result
            .get("functionBodyOffsets")
            .and_then(|o| o.as_array())
            .cloned()
            .ok_or_else(|| {
                BrowserError::BreakpointError("No functionBodyOffsets in response".into())
            })?;

        let mut entries = Vec::new();
        let mut iter = offsets.iter();

        while let (Some(start), Some(end)) = (iter.next(), iter.next()) {
            let start = start.as_u64().unwrap_or(0) as u32;
            let end = end.as_u64().unwrap_or(0) as u32;
            entries.push(WasmDisassemblyEntry { start, end });
        }

        Ok(entries)
    }

    /// Get script source
    pub async fn get_script_source(&self, script_id: &str) -> Result<String, BrowserError> {
        let response = self
            .send_command(
                "Debugger.getScriptSource",
                serde_json::json!({
                    "scriptId": script_id
                }),
            )
            .await?;

        let source = response
            .get("result")
            .and_then(|r| r.get("scriptSource"))
            .and_then(|s| s.as_str())
            .ok_or_else(|| BrowserError::BreakpointError("Failed to get script source".into()))?
            .to_string();

        Ok(source)
    }

    /// Evaluate expression in runtime context
    pub async fn runtime_evaluate(
        &self,
        expression: &str,
    ) -> Result<RemoteObject, BrowserError> {
        let response = self
            .send_command(
                "Runtime.evaluate",
                serde_json::json!({
                    "expression": expression,
                    "returnByValue": false
                }),
            )
            .await?;

        let result = response
            .get("result")
            .ok_or_else(|| BrowserError::CdpCommandError {
                method: "Runtime.evaluate".into(),
                message: "No result".into(),
            })?;

        let obj: RemoteObject = serde_json::from_value(result.clone())
            .map_err(|e| BrowserError::Json(e.to_string()))?;

        Ok(obj)
    }
}

/// Result of WASM module disassembly
#[derive(Debug, Clone)]
pub struct WasmDisassemblyEntry {
    pub start: u32,
    pub end: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdp_event_debugger_paused_deserialize() {
        let json = r#"{
            "method": "Debugger.paused",
            "params": {
                "reason": "breakpoint",
                "hitBreakpoints": ["wasm.js:10"],
                "callFrames": [{
                    "callFrameId": "1",
                    "functionName": "add",
                    "url": "wasm.js",
                    "lineNumber": 10,
                    "columnNumber": 2,
                    "scopeChain": []
                }]
            }
        }"#;

        let event: CdpEventType = serde_json::from_str(json).unwrap();
        match event {
            CdpEventType::DebuggerPaused(params) => {
                assert_eq!(params.reason, "breakpoint");
                assert_eq!(params.call_frames.len(), 1);
                assert_eq!(params.call_frames[0].function_name, "add");
            }
            _ => panic!("Expected DebuggerPaused"),
        }
    }

    #[test]
    fn test_cdp_event_script_parsed_deserialize() {
        let json = r#"{
            "method": "Debugger.scriptParsed",
            "params": {
                "scriptId": "123",
                "url": "wasm.wasm",
                "scriptLanguage": "WebAssembly",
                "hash": "abc123"
            }
        }"#;

        let event: CdpEventType = serde_json::from_str(json).unwrap();
        match event {
            CdpEventType::DebuggerScriptParsed(params) => {
                assert_eq!(params.script_id, "123");
                assert_eq!(params.script_language.as_deref(), Some("WebAssembly"));
                assert_eq!(params.hash.as_deref(), Some("abc123"));
            }
            _ => panic!("Expected DebuggerScriptParsed"),
        }
    }

    #[test]
    fn test_cdp_response_deserialize_success() {
        let json = r#"{
            "id": 42,
            "result": {
                "breakpointId": "bp-1"
            }
        }"#;

        let response: CdpResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, 42);
        assert!(response.error.is_none());
    }

    #[test]
    fn test_cdp_response_deserialize_error() {
        let json = r#"{
            "id": 42,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        }"#;

        let response: CdpResponse = serde_json::from_str(json).unwrap();
        assert_eq!(response.id, 42);
        assert!(response.error.is_some());
        let error = response.error.unwrap();
        assert_eq!(error.code, -32601);
        assert_eq!(error.message, "Method not found");
    }
}
