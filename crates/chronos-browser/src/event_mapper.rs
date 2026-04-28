//! Event conversion from CDP Debugger.paused to WASM TraceEvents.
//!
//! Converts CDP JSON events into traceable WASM events following the
//! pattern established in chronos-js/src/adapter.rs.

use crate::wasm_probes::WasmBreakpointManager;
use chronos_domain::trace::{EventData, EventType, SourceLocation, TraceEvent, WasmEventKind, WasmFunctionInfo, WasmModuleInfo};
use serde::Deserialize;
use std::collections::HashMap;

/// CDP Debugger.paused event structure for WASM debugging.
#[derive(Debug, Deserialize)]
pub struct CdpDebuggerPaused {
    #[serde(rename = "callFrames")]
    pub call_frames: Vec<CdpCallFrame>,
    pub reason: String,
    #[serde(default, rename = "hitBreakpoints")]
    pub hit_breakpoints: Vec<String>,
}

/// A call frame from CDP for WASM functions.
#[derive(Debug, Deserialize)]
pub struct CdpCallFrame {
    #[serde(rename = "functionName")]
    pub function_name: String,
    pub location: CdpLocation,
    #[serde(default, rename = "scopeChain")]
    pub scope_chain: Vec<CdpScope>,
}

/// Location in the script.
#[derive(Debug, Deserialize)]
pub struct CdpLocation {
    #[serde(rename = "scriptId")]
    pub script_id: String,
    #[serde(rename = "lineNumber")]
    pub line_number: i64,
    #[serde(default, rename = "columnNumber")]
    pub column_number: Option<i64>,
}

/// A scope in the call frame.
#[derive(Debug, Deserialize)]
pub struct CdpScope {
    #[serde(rename = "type")]
    pub scope_type: String,
    #[serde(default)]
    pub object: Option<CdpRemoteObject>,
}

/// Remote object reference in CDP.
#[derive(Debug, Deserialize)]
pub struct CdpRemoteObject {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub class_name: Option<String>,
    #[serde(default)]
    pub value: Option<serde_json::Value>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub object_id: Option<String>,
}

/// Convert a CDP Debugger.paused event to WASM TraceEvents.
///
/// This function follows the pattern from `chronos-js/src/adapter.rs:paused_to_trace_events()`.
///
/// # Arguments
/// * `paused` - The parsed CDP debugger paused event
/// * `modules` - Map of script_id -> WASM module info
/// * `breakpoint_manager` - Manager for WASM breakpoints
/// * `timestamp_ns` - Timestamp in nanoseconds since session start
/// * `next_event_id` - Next available event ID (will be incremented)
///
/// # Returns
/// Vector of TraceEvents for each call frame, or empty vec if not a WASM breakpoint.
pub fn paused_to_wasm_events(
    paused: &CdpDebuggerPaused,
    modules: &HashMap<String, WasmModuleInfo>,
    breakpoint_manager: &WasmBreakpointManager,
    timestamp_ns: u64,
    next_event_id: &mut u64,
) -> Vec<TraceEvent> {
    let mut events = Vec::new();

    // Extract body_offset from the first call frame for position-based return detection
    let first_body_offset = paused
        .call_frames
        .first()
        .map(|f| f.location.line_number as u32);

    // Determine the WASM event kind based on the paused reason
    let event_kind = match paused.reason.as_str() {
        "breakpoint" => {
            // Check if this is an entry or return breakpoint
            // by looking at hit_breakpoints
            if let Some(hit_bp) = paused.hit_breakpoints.first() {
                // If the breakpoint is in the function_breakpoints map,
                // it's an entry breakpoint; otherwise it might be a return
                if let Some(probe) = breakpoint_manager.get_function_for_breakpoint(hit_bp) {
                    // Agent-first: detect return/entry by breakpoint position,
                    // not by toolchain-specific __return__ naming convention.
                    //
                    // For the first call frame, use body_offset proximity:
                    //   - Near body_end   → Return breakpoint
                    //   - Near body_start → Entry breakpoint
                    //
                    // Fallback: __return__ prefix for legacy toolchains.
                    let body_len = probe.function.body_end.saturating_sub(probe.function.body_start);
                    let threshold = (body_len as f64 * 0.03) as u32; // 3% proximity
                    let body_offset = first_body_offset.unwrap_or(0);
                    let is_return = if body_len > 0 {
                        let dist_to_end = probe.function.body_end.saturating_sub(body_offset);
                        let dist_to_start = body_offset.saturating_sub(probe.function.body_start);
                        dist_to_end <= threshold.max(1) && dist_to_end < dist_to_start
                    } else {
                        false
                    };

                    if is_return {
                        WasmEventKind::Return
                    } else if probe.function.name.as_ref().is_some_and(|n| n.starts_with("__return__")) {
                        // Legacy fallback: Emscripten __return__ prefix
                        tracing::debug!(
                            "Using legacy __return__ detection for function {:?} — consider recompiling with position metadata",
                            probe.function.name
                        );
                        WasmEventKind::Return
                    } else {
                        WasmEventKind::Entry
                    }
                } else {
                    // Unknown breakpoint type, treat as breakpoint
                    WasmEventKind::Breakpoint
                }
            } else {
                WasmEventKind::Breakpoint
            }
        }
        "exception" => WasmEventKind::Exception,
        other => WasmEventKind::Other(other.to_string()),
    };

    // Process each call frame
    for frame in &paused.call_frames {
        // Get the script_id from the location
        let script_id = &frame.location.script_id;

        // Look up the WASM module for this script
        let module = match modules.get(script_id) {
            Some(m) => m,
            None => continue, // Not a WASM module we track
        };

        // Find the function info based on the location (line_number is the body offset for WASM)
        let body_offset = frame.location.line_number as u32;
        let function_info = find_function_by_offset(module, body_offset);

        let function_index = function_info
            .map(|f| f.function_index as u32)
            .unwrap_or(0);

        let function_name = function_info
            .and_then(|f| f.name.clone())
            .or_else(|| {
                if frame.function_name.is_empty() {
                    None
                } else {
                    Some(frame.function_name.clone())
                }
            });

        // Extract locals from scope chain where type == "wasm-expression-stack"
        let locals = extract_wasm_locals(&frame.scope_chain);

        let event_id = *next_event_id;
        *next_event_id += 1;

        let location = SourceLocation {
            file: module.url.clone(),
            function: function_name.clone(),
            ..Default::default()
        };

        let data = EventData::WasmFrame {
            function_index,
            function_name,
            body_offset,
            module_url: module.url.clone(),
            locals,
            event_kind: event_kind.clone(),
        };

        events.push(TraceEvent {
            event_id,
            timestamp_ns,
            thread_id: 1, // WASM is single-threaded in MVP
            event_type: EventType::BreakpointHit,
            location,
            data,
        });
    }

    events
}

/// Find a function in the module by its body offset.
fn find_function_by_offset(module: &WasmModuleInfo, offset: u32) -> Option<&WasmFunctionInfo> {
    module.functions.iter().find(|f| {
        offset >= f.body_start && offset < f.body_end
    })
}

/// Extract local variables from WASM expression stack scope.
fn extract_wasm_locals(scope_chain: &[CdpScope]) -> Option<Vec<chronos_domain::value::VariableInfo>> {
    // Look for wasm-expression-stack scope type
    let wasm_scope = scope_chain.iter().find(|s| s.scope_type == "wasm-expression-stack")?;

    let mut locals = Vec::new();

    if let Some(object) = &wasm_scope.object {
        if let Some(value) = &object.value {
            if let Some(arr) = value.as_array() {
                for (i, item) in arr.iter().enumerate() {
                    let name = format!("local_{}", i);
                    let value_str = item.to_string();
                    // Use type from the object if available, otherwise default to "unknown"
                    // WASM byte offset values on the expression stack don't have explicit types
                    let type_str = &object.type_;
                    locals.push(chronos_domain::value::VariableInfo::new(
                        &name,
                        &value_str,
                        type_str,
                        0,
                        chronos_domain::value::VariableScope::Local,
                    ));
                }
            }
        }
    }

    if locals.is_empty() {
        None
    } else {
        Some(locals)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cdp_debugger_paused_deserialize() {
        let json = r#"{
            "callFrames": [
                {
                    "functionName": "add",
                    "location": {
                        "scriptId": "123",
                        "lineNumber": 10,
                        "columnNumber": 0
                    },
                    "scopeChain": [
                        {
                            "type": "wasm-expression-stack",
                            "object": {
                                "type": "object",
                                "value": [1, 2]
                            }
                        }
                    ]
                }
            ],
            "reason": "breakpoint",
            "hitBreakpoints": ["bp-1"]
        }"#;

        let paused: CdpDebuggerPaused = serde_json::from_str(json).unwrap();
        assert_eq!(paused.reason, "breakpoint");
        assert_eq!(paused.call_frames.len(), 1);
        assert_eq!(paused.call_frames[0].function_name, "add");
        assert_eq!(paused.hit_breakpoints, vec!["bp-1"]);
    }

    #[test]
    fn test_find_function_by_offset() {
        let module = WasmModuleInfo {
            script_id: "123".to_string(),
            url: Some("test.wasm".to_string()),
            hash: "abc".to_string(),
            build_id: None,
            functions: vec![
                WasmFunctionInfo {
                    function_index: 0,
                    name: Some("add".to_string()),
                    body_start: 0,
                    body_end: 10,
                    breakpoint_id: None,
                },
                WasmFunctionInfo {
                    function_index: 1,
                    name: Some("sub".to_string()),
                    body_start: 10,
                    body_end: 20,
                    breakpoint_id: None,
                },
            ],
        };

        assert!(find_function_by_offset(&module, 5).is_some());
        assert_eq!(
            find_function_by_offset(&module, 5).unwrap().name.as_deref(),
            Some("add")
        );
        assert_eq!(
            find_function_by_offset(&module, 15).unwrap().name.as_deref(),
            Some("sub")
        );
        assert!(find_function_by_offset(&module, 25).is_none());
    }
}