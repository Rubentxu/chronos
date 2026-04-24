//! MCP tool wrappers for the sandbox client.
//!
//! Provides typed wrappers around all MCP tool methods exposed by the Chronos server.

use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::sleep;

use super::error::McpSandboxError;
use super::process::{McpReader, McpWriter};
use super::rpc::RpcClient;
use super::types::*;

// ============================================================================
// McpSession
// ============================================================================

/// A session connected to an MCP server.
///
/// This holds the RPC client for communicating with the server.
pub struct McpSession {
    rpc_client: RpcClient,
}

impl McpSession {
    /// Create a new MCP session from process handles.
    ///
    /// This spawns the MCP server process, connects via stdio, and sends
    /// the MCP protocol initialization handshake.
    pub async fn new(writer: McpWriter, reader: McpReader) -> Result<Self, McpSandboxError> {
        let mut rpc_client = RpcClient::new(writer, reader);
        rpc_client.initialize().await?;
        Ok(Self { rpc_client })
    }

    /// Shutdown the session by killing the server.
    pub async fn shutdown(self) -> Result<(), McpSandboxError> {
        // The RpcClient holds the handles, drop them to release resources
        // The process will be cleaned up when McpProcess is dropped
        Ok(())
    }

    /// Get the path to a compiled C fixture binary.
    ///
    /// Searches in the following order:
    /// 1. OUT_DIR environment variable (set by build script)
    /// 2. Relative to the test binary location (for when running via `cargo test`)
    pub fn fixture_path(name: &str) -> Option<PathBuf> {
        // Try OUT_DIR from build script
        if let Ok(out_dir) = std::env::var("OUT_DIR") {
            let path = PathBuf::from(out_dir).join(name);
            if path.exists() {
                return Some(path);
            }
        }
        // Try relative to test binary
        if let Ok(exe) = std::env::current_exe() {
            if let Some(dir) = exe.parent() {
                // Navigate up from target/debug/deps/ to target/debug/
                let path = dir.join(name);
                if path.exists() {
                    return Some(path);
                }
                // Try going up one more level
                if let Some(parent) = dir.parent() {
                    let path = parent.join(name);
                    if path.exists() {
                        return Some(path);
                    }
                }
            }
        }
        None
    }

    // =========================================================================
    // Probe Tools (Task 2.1)
    // =========================================================================

    /// Probe start — begins capturing execution of a target program.
    ///
    /// Sends `{"method": "probe_start", "params": {...}}` and returns the session_id.
    pub async fn probe_start(&mut self, target: &str) -> Result<String, McpSandboxError> {
        self.probe_start_with_params(target, true, 50000).await
    }

    /// Probe start with custom parameters — begins capturing execution of a target program.
    ///
    /// This variant allows overriding trace_syscalls and bus_capacity for testing edge cases.
    pub async fn probe_start_with_params(
        &mut self,
        program: &str,
        trace_syscalls: bool,
        bus_capacity: usize,
    ) -> Result<String, McpSandboxError> {
        let params = serde_json::json!({
            "program": program,
            "trace_syscalls": trace_syscalls,
            "bus_capacity": bus_capacity
        });

        let response = self.rpc_client.call_tool("probe_start", params).await?;

        let result: ProbeStartResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.session_id)
    }

    /// Probe start raw — returns the raw JSON response for edge case testing.
    ///
    /// Use this when you need to inspect the full response including error cases.
    pub async fn probe_start_raw(
        &mut self,
        program: &str,
    ) -> Result<serde_json::Value, McpSandboxError> {
        let params = serde_json::json!({
            "program": program,
            "trace_syscalls": true,
            "bus_capacity": 50000
        });

        self.rpc_client.call_tool("probe_start", params).await
    }

    /// Probe stop — ends a probe session.
    pub async fn probe_stop(&mut self, session_id: &str) -> Result<ProbeStopResponse, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id
        });

        let response = self.rpc_client.call_tool("probe_stop", params).await?;

        let result: ProbeStopResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }

    /// Probe drain — retrieves events collected since last drain.
    pub async fn probe_drain(&mut self, session_id: &str) -> Result<Vec<SemanticEvent>, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "limit": 1000,
            "offset": 0
        });

        let response = self.rpc_client.call_tool("probe_drain", params).await?;

        let result: ProbeDrainResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.events)
    }

    /// Probe drain raw — returns the full ProbeDrainResponse for edge case inspection.
    pub async fn probe_drain_raw(&mut self, session_id: &str) -> Result<ProbeDrainResponse, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "limit": 1000,
            "offset": 0
        });

        let response = self.rpc_client.call_tool("probe_drain", params).await?;

        let result: ProbeDrainResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }

    /// Probe inject — inject a uprobe into a running process.
    pub async fn probe_inject(
        &mut self,
        session_id: &str,
        binary_path: &str,
        symbol_name: &str,
    ) -> Result<(), McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "binary_path": binary_path,
            "symbol_name": symbol_name
        });

        let _response = self.rpc_client.call_tool("probe_inject", params).await?;
        // probe_inject returns ProbeInjectResponse, we don't need to parse it fully
        Ok(())
    }

    /// Probe inject raw — returns the full JSON response for edge case inspection.
    ///
    /// Use this when you need to inspect the full response including error cases.
    pub async fn probe_inject_raw(
        &mut self,
        session_id: &str,
        binary_path: &str,
        symbol_name: &str,
    ) -> Result<serde_json::Value, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "binary_path": binary_path,
            "symbol_name": symbol_name
        });

        self.rpc_client.call_tool("probe_inject", params).await
    }

    /// Session snapshot — freeze a live probe and build query indices without stopping it.
    ///
    /// This makes the session queryable while the probe continues collecting events.
    pub async fn session_snapshot(
        &mut self,
        session_id: &str,
    ) -> Result<SessionSnapshotResponse, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id
        });

        let response = self.rpc_client.call_tool("session_snapshot", params).await?;

        let result: SessionSnapshotResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }

    /// Session snapshot raw — returns the full JSON response for edge case inspection.
    pub async fn session_snapshot_raw(
        &mut self,
        session_id: &str,
    ) -> Result<serde_json::Value, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id
        });

        self.rpc_client.call_tool("session_snapshot", params).await
    }

    // =========================================================================
    // Tripwire Tools (Task 2.2)
    // =========================================================================

    /// Tripwire create — sets a watchpoint that triggers on condition.
    pub async fn tripwire_create(
        &mut self,
        config: TripwireCreateParams,
    ) -> Result<String, McpSandboxError> {
        let params = serde_json::to_value(config)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        let response = self.rpc_client.call_tool("tripwire_create", params).await?;

        let result: TripwireCreateResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.tripwire_id)
    }

    /// Tripwire list — lists all active tripwires.
    pub async fn tripwire_list(&mut self) -> Result<Vec<TripwireInfo>, McpSandboxError> {
        let params = serde_json::json!({});

        let response = self.rpc_client.call_tool("tripwire_list", params).await?;

        let result: TripwireListResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.active_tripwires)
    }

    /// Tripwire delete — removes a tripwire by ID.
    pub async fn tripwire_delete(&mut self, tripwire_id: &str) -> Result<(), McpSandboxError> {
        let params = serde_json::json!({
            "tripwire_id": tripwire_id
        });

        let _response = self.rpc_client.call_tool("tripwire_delete", params).await?;
        Ok(())
    }

    /// Tripwire query — queries tripwire state without draining fired events.
    pub async fn tripwire_query(&mut self) -> Result<Vec<TripwireInfo>, McpSandboxError> {
        let params = serde_json::json!({});

        let response = self.rpc_client.call_tool("tripwire_query", params).await?;

        let result: TripwireQueryResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.active_tripwires)
    }

    // =========================================================================
    // Query Tools (Task 2.3)
    // =========================================================================

    /// Query events — queries events from a completed session.
    pub async fn query_events(
        &mut self,
        session_id: &str,
        filter: QueryFilter,
    ) -> Result<Vec<TraceEvent>, McpSandboxError> {
        let mut params = serde_json::json!({
            "session_id": session_id,
            "limit": filter.limit,
            "offset": filter.offset
        });

        if let Some(event_types) = filter.event_types {
            params["event_types"] = serde_json::json!(event_types);
        }
        if let Some(thread_id) = filter.thread_id {
            params["thread_id"] = serde_json::json!(thread_id);
        }
        if let Some(ts_start) = filter.timestamp_start {
            params["timestamp_start"] = serde_json::json!(ts_start);
        }
        if let Some(ts_end) = filter.timestamp_end {
            params["timestamp_end"] = serde_json::json!(ts_end);
        }
        if let Some(pattern) = filter.function_pattern {
            params["function_pattern"] = serde_json::json!(pattern);
        }

        let response = self.rpc_client.call_tool("query_events", params).await?;

        let result: QueryEventsResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.events)
    }

    /// Get event — retrieves detailed information about a specific trace event.
    pub async fn get_event(
        &mut self,
        session_id: &str,
        event_id: u64,
    ) -> Result<serde_json::Value, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "event_id": event_id
        });

        let response = self.rpc_client.call_tool("get_event", params).await?;

        // get_event returns the raw event as JSON
        Ok(response)
    }

    /// Get call stack — reconstructs the call stack at a specific event.
    pub async fn get_call_stack(
        &mut self,
        session_id: &str,
        event_id: u64,
    ) -> Result<Vec<StackFrame>, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "event_id": event_id
        });

        let response = self.rpc_client.call_tool("get_call_stack", params).await?;

        let result: GetCallStackResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.frames)
    }

    /// List threads — lists all thread IDs in the trace.
    pub async fn list_threads(&mut self, session_id: &str) -> Result<Vec<ThreadInfo>, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id
        });

        let response = self.rpc_client.call_tool("list_threads", params).await?;

        let result: ListThreadsResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        let threads: Vec<ThreadInfo> = result
            .thread_ids
            .into_iter()
            .map(|tid| ThreadInfo { thread_id: tid })
            .collect();

        Ok(threads)
    }

    /// Get execution summary — top-level execution overview.
    pub async fn get_execution_summary(
        &mut self,
        session_id: &str,
    ) -> Result<ExecutionSummaryResponse, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id
        });

        let response = self.rpc_client.call_tool("get_execution_summary", params).await?;

        let result: ExecutionSummaryResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }

    /// Debug call graph — build call graph for a session.
    pub async fn debug_call_graph(
        &mut self,
        session_id: &str,
        max_depth: usize,
    ) -> Result<CallGraphResponse, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "max_depth": max_depth
        });

        let response = self.rpc_client.call_tool("debug_call_graph", params).await?;

        let result: CallGraphResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }

    /// State diff — compare program state between two timestamps.
    pub async fn state_diff(
        &mut self,
        session_id: &str,
        timestamp_a: u64,
        timestamp_b: u64,
    ) -> Result<StateDiffResponse, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "timestamp_a": timestamp_a,
            "timestamp_b": timestamp_b
        });

        let response = self.rpc_client.call_tool("state_diff", params).await?;

        let result: StateDiffResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }

    // =========================================================================
    // Debug/Analysis Tools (Task 2.4)
    // =========================================================================

    /// Debug find crash — identifies the crash point in a trace.
    pub async fn debug_find_crash(
        &mut self,
        session_id: &str,
    ) -> Result<Option<CrashInfo>, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id
        });

        let response = self.rpc_client.call_tool("debug_find_crash", params).await?;

        let result: DebugFindCrashResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        if result.crash_found {
            Ok(Some(CrashInfo {
                session_id: result.session_id,
                crash_found: true,
                signal: result.signal,
                event_id: result.event_id,
                timestamp_ns: result.timestamp_ns,
                thread_id: result.thread_id,
                call_stack_depth: result.call_stack_depth,
                call_stack: result.call_stack,
                note: result.note,
            }))
        } else {
            Ok(None)
        }
    }

    /// Debug detect races — detects data races in the trace.
    pub async fn debug_detect_races(
        &mut self,
        session_id: &str,
    ) -> Result<Vec<RaceReport>, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "threshold_ns": 100
        });

        let response = self.rpc_client.call_tool("debug_detect_races", params).await?;

        let result: DebugDetectRacesResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.races)
    }

    /// Inspect causality — inspects the full causal history of a memory address.
    pub async fn inspect_causality(
        &mut self,
        session_id: &str,
        address: u64,
    ) -> Result<CausalityReport, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "address": address,
            "limit": 100
        });

        let response = self.rpc_client.call_tool("inspect_causality", params).await?;

        let result: InspectCausalityResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        let report = CausalityReport {
            session_id: result.session_id,
            address: result.address,
            mutation_count: result.mutation_count,
            mutations: result.mutations,
            note: result.note,
        };

        Ok(report)
    }

    /// Debug get saliency scores — computes saliency scores for functions.
    pub async fn debug_get_saliency_scores(
        &mut self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<SaliencyScore>, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "limit": limit
        });

        let response = self.rpc_client.call_tool("debug_get_saliency_scores", params).await?;

        let result: DebugGetSaliencyScoresResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.scores)
    }

    /// Debug expand hotspot — returns top-N hottest functions.
    pub async fn debug_expand_hotspot(
        &mut self,
        session_id: &str,
        top_n: usize,
    ) -> Result<HotspotDetail, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "top_n": top_n
        });

        let response = self.rpc_client.call_tool("debug_expand_hotspot", params).await?;

        // The response contains a hotspot_functions array, we return the first one
        // or aggregate them
        let result: DebugExpandHotspotResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        // Aggregate all hotspot functions into a single HotspotDetail
        let total_calls: u64 = result.hotspot_functions.iter().map(|f| f.call_count).sum();
        let total_cycles: Option<u64> = result
            .hotspot_functions
            .iter()
            .filter_map(|f| f.total_cycles)
            .reduce(|a, b| a.saturating_add(b));

        Ok(HotspotDetail {
            function: format!("{} functions", result.hotspot_functions.len()),
            call_count: total_calls,
            total_cycles,
            avg_cycles_per_call: None,
        })
    }

    // =========================================================================
    // Session/Persistence Tools (Task 2.5)
    // =========================================================================

    /// Save session — saves an in-memory session to persistent storage.
    pub async fn save_session(
        &mut self,
        session_id: &str,
        name: &str,
    ) -> Result<SaveSessionResponse, McpSandboxError> {
        self.save_session_with_language(session_id, "native", name).await
    }

    /// Save session with custom language tag — saves an in-memory session with specified language.
    pub async fn save_session_with_language(
        &mut self,
        session_id: &str,
        language: &str,
        target: &str,
    ) -> Result<SaveSessionResponse, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "language": language,
            "target": target
        });

        let response = self.rpc_client.call_tool("save_session", params).await?;

        let result: SaveSessionResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }

    /// Load session — loads a session from persistent storage.
    pub async fn load_session(
        &mut self,
        session_id: &str,
    ) -> Result<SessionInfo, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id
        });

        let response = self.rpc_client.call_tool("load_session", params).await?;

        let result: LoadSessionResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        let info = SessionInfo {
            session_id: result.session_id,
            language: result.language,
            target: result.target,
            event_count: result.event_count,
            duration_ms: result.duration_ms,
            created_at: result.created_at,
            hint: result.hint,
        };

        Ok(info)
    }

    /// List sessions — lists all saved sessions.
    pub async fn list_sessions(&mut self) -> Result<Vec<SessionInfo>, McpSandboxError> {
        let params = serde_json::json!({});

        let response = self.rpc_client.call_tool("list_sessions", params).await?;

        let result: ListSessionsResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        let sessions: Vec<SessionInfo> = result
            .sessions
            .into_iter()
            .map(|s| SessionInfo {
                session_id: s.session_id,
                language: s.language,
                target: s.target,
                event_count: s.event_count,
                duration_ms: s.duration_ms,
                created_at: s.created_at,
                hint: None,
            })
            .collect();

        Ok(sessions)
    }

    /// Delete session — deletes a session from persistent storage.
    pub async fn delete_session(&mut self, session_id: &str) -> Result<(), McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id
        });

        let _response = self.rpc_client.call_tool("delete_session", params).await?;
        Ok(())
    }

    /// Compare sessions — compares two sessions and reports differences.
    pub async fn compare_sessions(
        &mut self,
        session_a: &str,
        session_b: &str,
    ) -> Result<CompareReport, McpSandboxError> {
        let params = serde_json::json!({
            "session_a": session_a,
            "session_b": session_b
        });

        let response = self.rpc_client.call_tool("compare_sessions", params).await?;

        let result: CompareSessionsResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        let report = CompareReport {
            session_a_id: result.session_a_id,
            session_b_id: result.session_b_id,
            only_in_a_count: result.only_in_a_count,
            only_in_b_count: result.only_in_b_count,
            total_a: result.total_a,
            total_b: result.total_b,
            common_count: result.common_count,
            similarity_pct: result.similarity_pct,
            timing_delta_ms: result.timing_delta_ms,
            summary: result.summary,
        };

        Ok(report)
    }

    /// Performance regression audit — compares performance between two sessions.
    pub async fn performance_regression_audit(
        &mut self,
        baseline: &str,
        target: &str,
    ) -> Result<RegressionReport, McpSandboxError> {
        let params = serde_json::json!({
            "baseline_session_id": baseline,
            "target_session_id": target,
            "top_n": 20
        });

        let response = self.rpc_client.call_tool("performance_regression_audit", params).await?;

        let result: PerformanceRegressionAuditResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        let report = RegressionReport {
            baseline_session_id: result.baseline_session_id,
            target_session_id: result.target_session_id,
            regressions: result.regressions,
            improvements: result.improvements,
            functions_analyzed: result.functions_analyzed,
            total_call_delta: result.total_call_delta,
            summary: result.summary,
        };

        Ok(report)
    }

    // =========================================================================
    // Extended Debug/Analysis Tools (Phase 6.1)
    // =========================================================================

    /// Debug get registers — retrieves CPU register state at a specific event.
    pub async fn debug_get_registers(
        &mut self,
        session_id: &str,
        event_id: u64,
    ) -> Result<Registers, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "event_id": event_id
        });

        let response = self.rpc_client.call_tool("debug_get_registers", params).await?;

        let result: DebugGetRegistersResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(Registers {
            session_id: session_id.to_string(),
            event_id: result.event_id,
            registers: result.registers,
        })
    }

    /// Debug get variables — retrieves variables in scope at a specific frame.
    pub async fn debug_get_variables(
        &mut self,
        session_id: &str,
        event_id: u64,
    ) -> Result<Vec<VariableInfo>, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "event_id": event_id
        });

        let response = self.rpc_client.call_tool("debug_get_variables", params).await?;

        let result: DebugGetVariablesResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.variables)
    }

    /// Debug get memory — reads raw memory at an address as of a specific timestamp.
    pub async fn debug_get_memory(
        &mut self,
        session_id: &str,
        address: u64,
        size: usize,
    ) -> Result<Vec<u8>, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "address": address,
            "size": size
        });

        let response = self.rpc_client.call_tool("debug_get_memory", params).await?;

        let result: DebugGetMemoryResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.data)
    }

    /// Debug diff — compares process state between two event IDs.
    pub async fn debug_diff(
        &mut self,
        session_id: &str,
        event_a: u64,
        event_b: u64,
    ) -> Result<DiffResult, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "event_id_a": event_a,
            "event_id_b": event_b
        });

        let response = self.rpc_client.call_tool("debug_diff", params).await?;

        let result: DebugDiffResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(DiffResult {
            session_id: result.session_id,
            event_a_id: result.event_a_id,
            event_b_id: result.event_b_id,
            registers_diff: result.registers_diff,
            memory_diff: result.memory_diff,
            summary: result.summary,
        })
    }

    /// Evaluate expression — evaluates an arithmetic expression using local variables.
    pub async fn evaluate_expression(
        &mut self,
        session_id: &str,
        expression: &str,
    ) -> Result<serde_json::Value, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "expression": expression
        });

        let response = self.rpc_client.call_tool("evaluate_expression", params).await?;

        let result: EvaluateExpressionResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result.result)
    }

    // =========================================================================
    // Tier 2 Tools
    // =========================================================================

    /// Debug analyze memory — analyze all memory accesses to an address range.
    pub async fn debug_analyze_memory(
        &mut self,
        session_id: &str,
        start_address: u64,
        end_address: u64,
        start_ts: u64,
        end_ts: u64,
    ) -> Result<DebugAnalyzeMemoryResponse, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "start_address": start_address,
            "end_address": end_address,
            "start_ts": start_ts,
            "end_ts": end_ts
        });

        let response = self.rpc_client.call_tool("debug_analyze_memory", params).await?;

        let result: DebugAnalyzeMemoryResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }

    /// Forensic memory audit — full audit trail for a specific address.
    pub async fn forensic_memory_audit(
        &mut self,
        session_id: &str,
        address: u64,
        limit: usize,
    ) -> Result<ForensicMemoryAuditResponse, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "address": address,
            "limit": limit
        });

        let response = self.rpc_client.call_tool("forensic_memory_audit", params).await?;

        let result: ForensicMemoryAuditResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }

    /// Debug find variable origin — trace writes to a variable.
    pub async fn debug_find_variable_origin(
        &mut self,
        session_id: &str,
        variable_name: &str,
        limit: usize,
    ) -> Result<DebugFindVariableOriginResponse, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id,
            "variable_name": variable_name,
            "limit": limit
        });

        let response = self.rpc_client.call_tool("debug_find_variable_origin", params).await?;

        let result: DebugFindVariableOriginResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }

    /// Drop session — remove a session from memory without affecting persistent storage.
    pub async fn drop_session(&mut self, session_id: &str) -> Result<DropSessionResponse, McpSandboxError> {
        let params = serde_json::json!({
            "session_id": session_id
        });

        let response = self.rpc_client.call_tool("drop_session", params).await?;

        let result: DropSessionResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }

    // =========================================================================
    // Error Recovery and Timeout Handling (Phase 6.2)
    // =========================================================================

    /// Call with custom timeout — sends an RPC call with a specified timeout.
    ///
    /// This allows individual tool calls to have their own timeout rather than
    /// using the default 30-second timeout.
    pub async fn call_with_timeout(
        &mut self,
        method: &str,
        params: serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, McpSandboxError> {
        self.rpc_client
            .call_with_timeout(method, params, timeout)
            .await
    }

    /// Probe drain with retry — retrieves events with exponential backoff retry.
    ///
    /// Retries up to 3 times with exponential backoff (100ms, 200ms, 400ms)
    /// if the server returns empty results, which can happen during high load.
    pub async fn probe_drain_with_retry(
        &mut self,
        session_id: &str,
        max_retries: u32,
    ) -> Result<Vec<SemanticEvent>, McpSandboxError> {
        let mut retry_count = 0;
        let base_delay = Duration::from_millis(100);
        let params = serde_json::json!({
            "session_id": session_id,
            "limit": 1000,
            "offset": 0
        });

        loop {
            let response = self.rpc_client.call_tool("probe_drain", params.clone()).await?;

            let result: ProbeDrainResponse = serde_json::from_value(response)
                .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

            // If we got events or reached the retry limit, return
            if !result.events.is_empty() || retry_count >= max_retries {
                return Ok(result.events);
            }

            // Exponential backoff before retry
            retry_count += 1;
            let delay = base_delay * 2u32.pow(retry_count - 1);
            tracing::debug!(
                session_id = session_id,
                retry = retry_count,
                delay_ms = delay.as_millis(),
                "probe_drain returned empty, retrying"
            );
            sleep(delay).await;
        }
    }

    /// Session health check — pings the server to verify it's responsive.
    pub async fn session_health_check(&mut self) -> Result<HealthCheckResponse, McpSandboxError> {
        let params = serde_json::json!({});

        let response = self.rpc_client.call_tool("health_check", params).await?;

        let result: HealthCheckResponse = serde_json::from_value(response)
            .map_err(|e| McpSandboxError::RpcError(e.to_string()))?;

        Ok(result)
    }
}

// ============================================================================
// Additional Types for Return Values
// ============================================================================

/// Causality report for memory address inspection.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CausalityReport {
    pub session_id: String,
    pub address: String,
    pub mutation_count: usize,
    pub mutations: Vec<CausalityMutation>,
    pub note: Option<String>,
}

/// Compare report for session comparison.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CompareReport {
    pub session_a_id: String,
    pub session_b_id: String,
    pub only_in_a_count: usize,
    pub only_in_b_count: usize,
    pub total_a: usize,
    pub total_b: usize,
    pub common_count: usize,
    pub similarity_pct: f64,
    pub timing_delta_ms: Option<i64>,
    pub summary: String,
}

impl CompareReport {
    /// Returns true if the comparison report indicates divergences between sessions.
    ///
    /// A report has divergences if there are events only in one session
    /// or if the similarity is below 95%.
    pub fn has_divergences(&self) -> bool {
        // If there are events only in one session, that's a divergence
        if self.only_in_a_count > 0 || self.only_in_b_count > 0 {
            return true;
        }
        // If similarity is below 95%, consider it a divergence
        if self.similarity_pct < 95.0 {
            return true;
        }
        false
    }
}

/// Regression report for performance comparison.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RegressionReport {
    pub baseline_session_id: String,
    pub target_session_id: String,
    pub regressions: Vec<FunctionRegressionEntry>,
    pub improvements: Vec<FunctionRegressionEntry>,
    pub functions_analyzed: usize,
    pub total_call_delta: i64,
    pub summary: String,
}

// ============================================================================
// McpTestClient
// ============================================================================

/// Test client for MCP sandbox.
///
/// Provides a high-level API for spawning MCP servers and creating sessions.
pub struct McpTestClient;

impl McpTestClient {
    /// Start a new MCP test session by spawning the server.
    ///
    /// Uses CHRONOS_MCP_PATH env var if set, otherwise tries to find the binary:
    /// 1. CARGO_BIN_EXE_chronos-mcp env var (set by cargo test when using dev-dependency)
    /// 2. ../../target/debug/chronos-mcp relative to test binary (test binaries are in target/debug/deps/)
    /// 3. "chronos-mcp" in PATH
    pub async fn start() -> Result<McpSession, McpSandboxError> {
        let mcp_path = std::env::var("CHRONOS_MCP_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                // Try CARGO_BIN_EXE_chronos-mcp first (set when using dev-dependency)
                if let Ok(cargo_bin) = std::env::var("CARGO_BIN_EXE_chronos-mcp") {
                    let path = PathBuf::from(cargo_bin);
                    if path.exists() {
                        return path;
                    }
                }
                // Try relative path from test binary location
                // Test binary is at target/debug/deps/<test> so we need to go up 2 levels
                let exe_path = std::env::current_exe().ok();
                let relative = exe_path
                    .as_ref()
                    .and_then(|p| p.parent())
                    .and_then(|p| p.parent())
                    .map(|p| p.join("chronos-mcp"));
                if let Some(ref path) = relative {
                    if path.exists() {
                        return path.clone();
                    }
                }
                PathBuf::from("chronos-mcp")
            });
        Self::start_path(&mcp_path).await
    }

    /// Start a new MCP test session by spawning the server at the given path.
    pub async fn start_path(mcp_path: &Path) -> Result<McpSession, McpSandboxError> {
        let (_process, stdin, reader) = crate::client::process::factory::start(mcp_path).await?;
        let session = McpSession::new(stdin, reader).await?;
        Ok(session)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_filter_default() {
        let filter = QueryFilter::default();
        assert_eq!(filter.limit, 100);
        assert!(filter.event_types.is_none());
        assert!(filter.thread_id.is_none());
    }

    #[test]
    fn test_tripwire_condition_serialization() {
        let condition = TripwireConditionType::FunctionName {
            pattern: "process_*".to_string(),
        };
        let json = serde_json::to_string(&condition).unwrap();
        assert!(json.contains("function_name"));
        assert!(json.contains("process_*"));
    }

    #[tokio::test]
    async fn test_client_creation() {
        // This test verifies that the types can be instantiated
        // Actual server spawning requires a real MCP binary
        let _client = McpTestClient;
    }
}
