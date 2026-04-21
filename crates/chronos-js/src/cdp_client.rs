//! CDP WebSocket client for communicating with Node.js debugger.

use crate::error::JsAdapterError;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::{connect_async, tungstenite::Message};
use tracing::{debug, error, warn};
use serde::Deserialize;
use serde_json::Value;

/// CDP message ID counter
#[allow(dead_code)]
static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[allow(dead_code)]
fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

/// CDP event types we care about
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "method", content = "params")]
pub enum CdpEventType {
    #[serde(rename = "Debugger.paused")]
    DebuggerPaused(DebuggerPausedParams),
    #[serde(rename = "Debugger.resumed")]
    DebuggerResumed,
    #[serde(rename = "Runtime.executionContextCreated")]
    RuntimeExecutionContextCreated(ExecutionContextCreatedParams),
    #[serde(rename = "Runtime.consoleAPICalled")]
    RuntimeConsoleApiCalled(ConsoleApiCalledParams),
    #[serde(rename = "Runtime.exceptionThrown")]
    RuntimeExceptionThrown(ExceptionThrownParams),
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
    pub call_frames: Vec<CallFrame>,
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

#[derive(Debug, Clone, Deserialize)]
pub struct ConsoleApiCalledParams {
    #[serde(rename = "type")]
    pub type_: String,
    pub args: Vec<RemoteObject>,
    pub timestamp: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExceptionThrownParams {
    pub exception_details: ExceptionDetails,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExceptionDetails {
    pub exception_id: i64,
    pub text: String,
}

/// A call frame from CDP
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFrame {
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

/// A scope in the call frame
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

/// CDP response to a command
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

/// Wrapper around CDP event for our use
#[derive(Debug, Clone)]
pub enum CdpEvent {
    DebuggerPaused {
        reason: String,
        call_frames: Vec<CallFrame>,
        hit_breakpoints: Vec<String>,
    },
    DebuggerResumed,
    RuntimeExecutionContextCreated {
        context_id: i64,
        origin: String,
        name: String,
    },
    ConsoleApiCalled {
        type_: String,
        args: Vec<RemoteObject>,
    },
    ExceptionThrown {
        text: String,
    },
    InspectorDetached,
    Other,
}

/// CDP WebSocket client
pub struct CdpClient {
    _ws_write: mpsc::Sender<String>,
    next_id: Arc<std::sync::atomic::AtomicU64>,
    event_tx: broadcast::Sender<CdpEvent>,
}

impl CdpClient {
    /// Connect to a CDP WebSocket endpoint
    pub async fn connect(url: &str) -> Result<Self, JsAdapterError> {
        let (ws_stream, _) = connect_async(url)
            .await
            .map_err(|e| JsAdapterError::WebSocketFailed(e.to_string()))?;

        let (mut write, mut read) = ws_stream.split();

        let next_id = Arc::new(std::sync::atomic::AtomicU64::new(1));
        let (event_tx, _event_rx) = broadcast::channel(100);
        let event_tx_clone = event_tx.clone();
        let (ws_write, mut ws_read) = mpsc::channel::<String>(100);

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
                                                        CdpEventType::RuntimeExecutionContextCreated(params) => {
                                                            CdpEvent::RuntimeExecutionContextCreated {
                                                                context_id: params.context.id,
                                                                origin: params.context.origin,
                                                                name: params.context.name,
                                                            }
                                                        }
                                                        CdpEventType::RuntimeConsoleApiCalled(params) => {
                                                            CdpEvent::ConsoleApiCalled {
                                                                type_: params.type_,
                                                                args: params.args,
                                                            }
                                                        }
                                                        CdpEventType::RuntimeExceptionThrown(params) => {
                                                            CdpEvent::ExceptionThrown {
                                                                text: params.exception_details.text,
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
            _ws_write: ws_write,
            next_id,
            event_tx,
        })
    }

    /// Send a CDP command and wait for response
    pub async fn send_command(&self, method: &str, params: Value) -> Result<Value, JsAdapterError> {
        let id = self.next_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        let msg = serde_json::json!({
            "id": id,
            "method": method,
            "params": params
        });

        debug!("CDP send: {} id={}", method, id);

        // For MVP, we assume success and return empty result
        // Real implementation would wait for the response with matching ID
        let _ = self._ws_write.send(msg.to_string()).await;

        // Return success with empty result for now
        Ok(serde_json::json!({}))
    }

    /// Subscribe to CDP events
    pub fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.event_tx.subscribe()
    }

    /// Enable the debugger domain
    pub async fn debugger_enable(&self) -> Result<(), JsAdapterError> {
        self.send_command("Debugger.enable", serde_json::json!({}))
            .await?;
        Ok(())
    }

    /// Enable the runtime domain
    pub async fn runtime_enable(&self) -> Result<(), JsAdapterError> {
        self.send_command("Runtime.enable", serde_json::json!({}))
            .await?;
        Ok(())
    }

    /// Resume execution
    pub async fn debugger_resume(&self) -> Result<(), JsAdapterError> {
        self.send_command("Debugger.resume", serde_json::json!({}))
            .await?;
        Ok(())
    }

    /// Step over
    pub async fn debugger_step_over(&self) -> Result<(), JsAdapterError> {
        self.send_command("Debugger.stepOver", serde_json::json!({}))
            .await?;
        Ok(())
    }

    /// Step into
    pub async fn debugger_step_into(&self) -> Result<(), JsAdapterError> {
        self.send_command("Debugger.stepInto", serde_json::json!({}))
            .await?;
        Ok(())
    }

    /// Step out
    pub async fn debugger_step_out(&self) -> Result<(), JsAdapterError> {
        self.send_command("Debugger.stepOut", serde_json::json!({}))
            .await?;
        Ok(())
    }

    /// Get properties of a remote object
    pub async fn runtime_get_properties(&self, object_id: &str) -> Result<Vec<Property>, JsAdapterError> {
        let result = self.send_command(
            "Runtime.getProperties",
            serde_json::json!({
                "objectId": object_id,
                "ownProperties": true,
                "generatePreview": true
            }),
        )
        .await?;

        let descriptors = result.get("result").and_then(|r| r.as_array()).cloned().unwrap_or_default();
        let properties: Vec<Property> = descriptors
            .into_iter()
            .filter_map(|d| serde_json::from_value(d).ok())
            .collect();
        Ok(properties)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Property {
    pub name: String,
    pub value: Option<RemoteObject>,
    pub is_owned: Option<bool>,
}

/// Mock CDP client for testing
#[cfg(test)]
pub struct MockCdpClient {
    events: broadcast::Sender<CdpEvent>,
}

#[cfg(test)]
impl MockCdpClient {
    pub fn new() -> Self {
        let (events, _) = broadcast::channel(100);
        Self { events }
    }

    pub fn emit_paused(&self, reason: String, call_frames: Vec<CallFrame>, hit_breakpoints: Vec<String>) {
        let _ = self.events.send(CdpEvent::DebuggerPaused {
            reason,
            call_frames,
            hit_breakpoints,
        });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }
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
                "hitBreakpoints": ["script.js:10"],
                "callFrames": [{
                    "callFrameId": "1",
                    "functionName": "foo",
                    "url": "script.js",
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
                assert_eq!(params.call_frames[0].function_name, "foo");
            }
            _ => panic!("Expected DebuggerPaused"),
        }
    }

    #[tokio::test]
    async fn test_mock_cdp_client() {
        let mock = MockCdpClient::new();
        let mut rx = mock.subscribe();

        let frames = vec![CallFrame {
            call_frame_id: "1".to_string(),
            function_name: "test".to_string(),
            function_location: None,
            url: "test.js".to_string(),
            line_number: 1,
            column_number: 0,
            scope_chain: vec![],
        }];

        mock.emit_paused("breakpoint".to_string(), frames.clone(), vec![]);

        match rx.recv().await {
            Ok(CdpEvent::DebuggerPaused { reason, call_frames, .. }) => {
                assert_eq!(reason, "breakpoint");
                assert_eq!(call_frames.len(), 1);
            }
            _ => panic!("Expected DebuggerPaused"),
        }
    }
}
