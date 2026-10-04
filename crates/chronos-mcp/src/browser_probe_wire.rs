//! `browser_probe_wire` — the three browser/WASM probe tools, off `server.rs`.
//!
//! ## Why this module exists
//!
//! `server.rs` had grown to 8.580 lines carrying 44 tools, and the roadmap
//! question (R4.1) was whether `ChronosServer` could be split into owned
//! groups. The first characterization said no — it claimed the service contexts
//! cut across every group, so no context was extractable as described. **That
//! was wrong**, and re-measuring the actual *constructions* per handler found
//! that each context belongs to one group: 14 `ProbeContext` uses across 14
//! handlers, 6 `SessionsContext` in 6, 5 `CounterexampleContext` in 5, and 3
//! `BrowserProbeContext` in 3. Only 11 handlers of 28 touch more than one
//! context, in four defined combinations.
//!
//! This is the first extraction, and deliberately the safest one: the browser
//! group is 3 handlers, 3 `BrowserProbeContext` constructions, zero
//! cross-group context, and no handler of another group appears in it.
//!
//! ## What this is NOT
//!
//! **No logic moved here.** `BrowserProbeService` (`chronos_services::
//! browser_probe`) already owns start/stop/drain; these three functions were
//! always thin wiring: build the context, call the service, map the result to
//! the wire shape. Re-implementing any of that would be the duplication the
//! repo forbids, so this module is only the wiring plus the output mapping,
//! which is genuinely the server's job because it is wire-shaped data.
//!
//! ## The toolset guard and the content helpers stay at the call site
//!
//! All three handlers call `self.toolset_guard(...)` first. That stays on the
//! server, not here: the guard reads server state, and the server is what the
//! toolset decision is about (see the R4.3 note in `server.rs` — the current
//! asymmetry is deliberate and pinned by
//! `chronos-sandbox/tests/toolset_declaration_gap.rs`).
//!
//! The `json_content`/`text_content` pair is also the server's, and this module
//! reached for a private copy at first on the grounds that "six lines each, not
//! worth the coupling". **That reasoning was wrong, and measurably so**: the
//! copy used `value.to_string()` where the original used
//! `to_string_pretty(...).unwrap_or_default()`, so moving three handlers into a
//! new module silently changed the JSON those tools put on the wire — a pure
//! refactor with an output change, and no test in the crate noticed. The
//! helpers are now `pub(crate)` and imported. The repo rule already said it:
//! compose on the canonical, do not reimplement.

use chronos_services::browser_probe::{
    BrowserProbeDrainInput, BrowserProbeService, BrowserProbeStartInput, BrowserProbeStopInput,
};
use rmcp::model::CallToolResult;
use serde_json::json;

use crate::server::{json_content, text_content, ChronosServer};
use crate::tools_params::{
    BrowserProbeDrainParams, BrowserProbeStartParams, BrowserProbeStopParams,
};

/// Start a browser debugging session. Launches Chrome headless, connects via
/// CDP, detects WASM modules, and sets breakpoints.
pub(crate) async fn browser_probe_start_impl(
    server: &ChronosServer,
    params: rmcp::handler::server::wrapper::Parameters<BrowserProbeStartParams>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let this = server;
    if let Some(err) = this.toolset_guard("browser_probe_start") {
        return Ok(err);
    }
    let params = params.0;
    let ctx = chronos_services::browser_probe::BrowserProbeContext {
        live_browser_probes: &this.live_browser_probes,
        active_session: &this.active_session,
        factory: &this.browser_probe_factory,
    };

    match BrowserProbeService::start(
        &ctx,
        BrowserProbeStartInput {
            url: params.url,
            headless: params.headless,
            chrome_path: params.chrome_path,
        },
    )
    .await
    {
        Ok(result) => {
            let output = json!({
                "session_id": result.session_id,
                "status": "running",
                "url": result.url,
                "hint": "Use browser_probe_drain to read WASM events, browser_probe_stop to finalize."
            });
            Ok(CallToolResult::success(json_content(&output)))
        }
        Err(e) => Ok(CallToolResult::error(text_content(e.to_string()))),
    }
}

/// Stop a browser probe session. Drains remaining events, disconnects CDP, and
/// kills the Chrome process.
pub(crate) async fn browser_probe_stop_impl(
    server: &ChronosServer,
    params: rmcp::handler::server::wrapper::Parameters<BrowserProbeStopParams>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let this = server;
    if let Some(err) = this.toolset_guard("browser_probe_stop") {
        return Ok(err);
    }
    let params = params.0;

    let ctx = chronos_services::browser_probe::BrowserProbeContext {
        live_browser_probes: &this.live_browser_probes,
        active_session: &this.active_session,
        factory: &this.browser_probe_factory,
    };

    match BrowserProbeService::stop(
        &ctx,
        BrowserProbeStopInput {
            session_id: params.session_id,
        },
    )
    .await
    {
        Ok(result) => {
            // Stopping is where a browser session becomes a QUERYABLE
            // session, so its raw events have to be indexed before the
            // tool answers. This is the one place the browser group reaches
            // outside itself, and it is why the group is "extractable" and
            // not "isolated" — `build_and_store_engine` lives on the server
            // and is `pub(crate)`, so this compiles across the seam.
            if result.total_events > 0 {
                this.build_and_store_engine(&result.session_id, result.raw_events, result.language)
                    .await;
            }

            let output = json!({
                "session_id": result.session_id,
                "status": "stopped",
                "url": result.url,
                "total_events": result.total_events,
                "hint": "Session is now queryable. Use query_events, get_call_stack, etc."
            });
            Ok(CallToolResult::success(json_content(&output)))
        }
        Err(e) => Ok(CallToolResult::error(text_content(e.to_string()))),
    }
}

/// Drain buffered WASM events from a running browser probe.
pub(crate) async fn browser_probe_drain_impl(
    server: &ChronosServer,
    params: rmcp::handler::server::wrapper::Parameters<BrowserProbeDrainParams>,
) -> Result<CallToolResult, rmcp::ErrorData> {
    let this = server;
    if let Some(err) = this.toolset_guard("browser_probe_drain") {
        return Ok(err);
    }
    let params = params.0;

    let ctx = chronos_services::browser_probe::BrowserProbeContext {
        live_browser_probes: &this.live_browser_probes,
        active_session: &this.active_session,
        factory: &this.browser_probe_factory,
    };

    match BrowserProbeService::drain(
        &ctx,
        BrowserProbeDrainInput {
            session_id: params.session_id,
            offset: params.offset,
            limit: params.limit,
        },
    )
    .await
    {
        Ok(result) => {
            let events_json: Vec<serde_json::Value> = result
                .events
                .into_iter()
                .map(|e| {
                    json!({
                        "event_id": e.event_id,
                        "timestamp_ns": e.timestamp_ns,
                        "thread_id": e.thread_id,
                        "language": e.language,
                        "kind": e.kind,
                        "description": e.description,
                    })
                })
                .collect();

            let output = json!({
                "session_id": result.session_id,
                "status": "running",
                "total_buffered": result.total_buffered,
                "returned": result.returned,
                "offset": result.offset,
                "limit": result.limit,
                "events": events_json,
                "hint": "Browser probe is still running. Call browser_probe_drain again for more events, or browser_probe_stop to finalize."
            });
            Ok(CallToolResult::success(json_content(&output)))
        }
        Err(e) => Ok(CallToolResult::error(text_content(e.to_string()))),
    }
}
