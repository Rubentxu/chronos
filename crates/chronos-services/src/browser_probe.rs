//! Browser/WASM probe service — live Chrome-CDP-based probes
//! (`browser_probe_*` tool family).
//!
//! This module owns the long-lived state associated with a live browser
//! probe: [`BrowserProbeSession`] (carrying the [`BrowserAdapter`], the
//! underlying [`CaptureSession`], and the target URL). The MCP-server tool
//! functions in `chronos-mcp` are thin wrappers that delegate here.
//!
//! The native probe service lives in [`crate::probe`].

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chronos_browser::BrowserAdapter;
use chronos_domain::adapter::ProbeBackend;
use chronos_domain::{CaptureConfig, CaptureSession, Language, TraceEvent};
use tokio::sync::Mutex as TokioMutex;
use tracing::info;
use uuid::Uuid;

use crate::error::ServiceError;
use crate::output::DrainedBrowserEventDto;

/// A live browser probe session.
///
/// Owns the [`BrowserAdapter`] driving the CDP session, the underlying
/// [`CaptureSession`] returned by `start_probe_async`, and the target URL.
/// Stored in the `Server`'s `live_browser_probes` HashMap keyed by
/// `session_id`.
pub struct BrowserProbeSession {
    /// The browser adapter driving the CDP session.
    pub adapter: Arc<BrowserAdapter>,
    /// The capture session returned by `start_capture`.
    pub session: CaptureSession,
    /// Session ID for this browser probe.
    pub session_id: String,
    /// Target URL being debugged.
    pub url: String,
}

impl std::fmt::Debug for BrowserProbeSession {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BrowserProbeSession")
            .field("session_id", &self.session_id)
            .field("url", &self.url)
            .finish()
    }
}

/// Borrowed handle to the [`Server`](crate)-level state required by the
/// browser probe service. Mirrors `ProbeContext` from the native probe service.
pub struct BrowserProbeContext<'a> {
    /// session_id → [`BrowserProbeSession`].
    pub live_browser_probes:
        &'a Arc<Mutex<HashMap<String, BrowserProbeSession>>>,
    /// session_id of the currently active probe; set on `start`.
    pub active_session: &'a TokioMutex<Option<String>>,
}

// ============================================================================
// Input / output types
// ============================================================================

/// Input for [`BrowserProbeService::start`].
#[derive(Debug, Clone)]
pub struct BrowserProbeStartInput {
    pub url: String,
    pub headless: bool,
    pub chrome_path: Option<String>,
}

/// Result of [`BrowserProbeService::start`].
#[derive(Debug, Clone)]
pub struct BrowserProbeStartResult {
    pub session_id: String,
    pub url: String,
}

/// Input for [`BrowserProbeService::stop`].
#[derive(Debug, Clone)]
pub struct BrowserProbeStopInput {
    pub session_id: String,
}

/// Result of [`BrowserProbeService::stop`].
///
/// `raw_events` are returned to the server wrapper so it can call
/// `build_and_store_engine(&session_id, raw_events, language).await` after the
/// service returns. The service intentionally does NOT call
/// `build_and_store_engine` itself — that method owns query-engine state and
/// lives on the `Server` impl.
#[derive(Debug, Clone)]
pub struct BrowserProbeStopResult {
    pub session_id: String,
    pub url: String,
    pub total_events: usize,
    pub raw_events: Vec<TraceEvent>,
    pub language: Language,
}

/// Input for [`BrowserProbeService::drain`].
#[derive(Debug, Clone)]
pub struct BrowserProbeDrainInput {
    pub session_id: String,
    pub offset: usize,
    pub limit: usize,
}

/// Result of [`BrowserProbeService::drain`].
#[derive(Debug, Clone)]
pub struct BrowserProbeDrainResult {
    pub session_id: String,
    pub total_buffered: usize,
    pub returned: usize,
    pub offset: usize,
    pub limit: usize,
    pub events: Vec<DrainedBrowserEventDto>,
}

// ============================================================================
// Service
// ============================================================================

/// Stateless service that owns the browser-probe lifecycle.
///
/// Methods take a [`BrowserProbeContext`] referencing the shared state held
/// by the `Server`. This keeps `BrowserProbeService` free of any `Arc<Self>`
/// cycle.
pub struct BrowserProbeService;

impl BrowserProbeService {
    /// Start a new browser probe session.
    ///
    /// 1. Checks that Chrome (or Chromium) is available on the host.
    /// 2. Creates a fresh [`BrowserAdapter`].
    /// 3. Calls `start_probe_async` to attach to CDP.
    /// 4. Stores the resulting [`BrowserProbeSession`] in `live_browser_probes`.
    /// 5. Sets `active_session` to the new session id.
    pub async fn start(
        ctx: &BrowserProbeContext<'_>,
        input: BrowserProbeStartInput,
    ) -> Result<BrowserProbeStartResult, ServiceError> {
        if !BrowserAdapter::is_chrome_available() {
            return Err(ServiceError::ChromeUnavailable);
        }

        let session_id = Uuid::new_v4().to_string();
        let adapter = Arc::new(BrowserAdapter::new());

        let config = CaptureConfig::new(&input.url);
        let session = adapter
            .start_probe_async(config, input.headless, input.chrome_path.as_deref())
            .await
            .map_err(|e| ServiceError::BrowserProbeStartFailed(e.to_string()))?;

        info!(
            "Browser probe started for '{}' (session: {})",
            input.url, session_id
        );

        let browser_probe = BrowserProbeSession {
            adapter: adapter.clone(),
            session,
            session_id: session_id.clone(),
            url: input.url.clone(),
        };
        ctx.live_browser_probes
            .lock()
            .unwrap()
            .insert(session_id.clone(), browser_probe);

        {
            let mut active = ctx.active_session.lock().await;
            *active = Some(session_id.clone());
        }

        Ok(BrowserProbeStartResult {
            session_id,
            url: input.url,
        })
    }

    /// Stop a browser probe session.
    ///
    /// 1. Removes the session from `live_browser_probes`.
    /// 2. Drains raw events from the adapter.
    /// 3. Stops the browser probe (best-effort; errors are logged, not propagated).
    /// 4. Returns the raw events + language so the server wrapper can hand them to
    ///    `build_and_store_engine`.
    pub async fn stop(
        ctx: &BrowserProbeContext<'_>,
        input: BrowserProbeStopInput,
    ) -> Result<BrowserProbeStopResult, ServiceError> {
        let browser_probe = {
            let mut probes = ctx.live_browser_probes.lock().unwrap();
            probes.remove(&input.session_id)
        };

        let browser_probe = browser_probe
            .ok_or_else(|| ServiceError::BrowserProbeNotFound(input.session_id.clone()))?;

        let events: Vec<TraceEvent> = browser_probe.adapter.drain_raw_events();
        let total_events = events.len();
        let language = Language::WebAssembly;
        let url = browser_probe.url.clone();

        if let Err(e) = browser_probe.adapter.stop_probe(&browser_probe.session) {
            tracing::warn!(
                "Browser probe stop error for session {}: {}",
                input.session_id,
                e
            );
        }

        info!(
            "Browser probe stopped for '{}' (session: {}, events: {})",
            url, input.session_id, total_events
        );

        Ok(BrowserProbeStopResult {
            session_id: input.session_id,
            url,
            total_events,
            raw_events: events,
            language,
        })
    }

    /// Drain a snapshot of events from a running browser probe.
    ///
    /// `drain_events` is destructive — it consumes the adapter's buffer — so
    /// subsequent calls return fewer events. The legacy `browser_probe_drain`
    /// tool was destructive; this matches that contract.
    pub async fn drain(
        ctx: &BrowserProbeContext<'_>,
        input: BrowserProbeDrainInput,
    ) -> Result<BrowserProbeDrainResult, ServiceError> {
        let adapter = {
            let probes = ctx.live_browser_probes.lock().unwrap();
            match probes.get(&input.session_id) {
                Some(bp) => bp.adapter.clone(),
                None => {
                    return Err(ServiceError::BrowserProbeNotFound(
                        input.session_id.clone(),
                    ))
                }
            }
        };

        let events = adapter
            .drain_events()
            .map_err(|e| ServiceError::BrowserProbeDrainFailed(e.to_string()))?;

        let total = events.len();
        let sliced: Vec<DrainedBrowserEventDto> = events
            .into_iter()
            .skip(input.offset)
            .take(input.limit)
            .map(|e| DrainedBrowserEventDto {
                event_id: e.source_event_id,
                timestamp_ns: e.timestamp_ns,
                thread_id: e.thread_id,
                language: format!("{:?}", e.language),
                kind: format!("{:?}", e.kind),
                description: e.description,
            })
            .collect();

        Ok(BrowserProbeDrainResult {
            session_id: input.session_id,
            total_buffered: total,
            returned: sliced.len(),
            offset: input.offset,
            limit: input.limit,
            events: sliced,
        })
    }
}
