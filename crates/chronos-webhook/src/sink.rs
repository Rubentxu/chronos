//! HTTPS-driven implementation of [`chronos_domain::ports::NotificationSink`].
//!
//! The implementation owns a dedicated multi-thread tokio runtime so the
//! synchronous [`NotificationSink::deliver`] method can drive the
//! async reqwest client without depending on the caller's runtime
//! (which is often synchronous, e.g. a CLI flag handler).
//!
//! The HTTP delivery loop is the one that used to live in
//! `chronos-domain/src/tripwire/webhook.rs`: exponential backoff
//! retries (1s, 2s, 4s by default), 5-second per-attempt timeout, up
//! to 3 attempts. The behaviour is intentionally identical so existing
//! receivers keep working; the difference is that the code lives in an
//! infrastructure crate and is reachable only via the
//! [`NotificationSink`] port.

use std::sync::Arc;
use std::time::Duration;

use chronos_domain::{NotificationDeliveryError, NotificationRequest, NotificationSink};
use serde::Serialize;
use thiserror::Error;
use tokio::runtime::{Handle, Runtime};
use tokio::time::sleep;
use tracing::{info, warn};

/// Maximum number of delivery attempts before giving up.
const MAX_ATTEMPTS: usize = 3;
/// Initial backoff delay; doubled on each retry (1s → 2s → 4s).
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);
/// Per-attempt HTTP timeout (connection + read).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(5);

/// Tunable knobs for the HTTP webhook sink.
#[derive(Debug, Clone)]
pub struct WebhookConfig {
    /// Maximum number of attempts (including the first one).
    pub max_attempts: usize,
    /// Initial backoff delay between attempts.
    pub initial_backoff: Duration,
    /// Per-attempt HTTP timeout (connection + read).
    pub request_timeout: Duration,
    /// When true, a non-empty `request.target.id` overrides the sink's
    /// default URL for that one delivery.
    pub allow_target_override: bool,
}

impl Default for WebhookConfig {
    fn default() -> Self {
        Self {
            max_attempts: MAX_ATTEMPTS,
            initial_backoff: INITIAL_BACKOFF,
            request_timeout: REQUEST_TIMEOUT,
            allow_target_override: true,
        }
    }
}

/// Adapter-internal errors. These are the *transport* failures; the
/// adapter maps them into [`NotificationDeliveryError`] variants for
/// the domain.
#[derive(Debug, Error)]
pub enum WebhookError {
    /// The reqwest client could not be built (TLS, system proxy, …).
    #[error("failed to build HTTP client: {0}")]
    ClientBuild(String),
    /// The runtime handle for the dedicated webhook runtime is no
    /// longer usable (the runtime has been shut down).
    #[error("webhook runtime unavailable: {0}")]
    RuntimeUnavailable(String),
}

/// The HTTPS-driven implementation of [`NotificationSink`].
///
/// The sink owns its own multi-thread tokio runtime so the synchronous
/// `deliver` method can drive the async reqwest client safely from any
/// caller context. The runtime is shut down when the sink is dropped.
#[derive(Debug)]
pub struct HttpWebhookSink {
    default_url: Arc<str>,
    client: Arc<reqwest::Client>,
    runtime: Option<Runtime>,
    handle: Handle,
    config: WebhookConfig,
}

impl HttpWebhookSink {
    /// Build a sink that posts to `default_url`. The sink owns its
    /// own multi-thread tokio runtime for HTTP delivery.
    pub fn new(default_url: &str) -> Result<Self, WebhookError> {
        Self::with_config(default_url, WebhookConfig::default())
    }

    /// Build a sink with an explicit configuration (custom timeouts,
    /// attempt caps, …).
    pub fn with_config(default_url: &str, config: WebhookConfig) -> Result<Self, WebhookError> {
        let client = reqwest::Client::builder()
            .timeout(config.request_timeout)
            .build()
            .map_err(|e| WebhookError::ClientBuild(e.to_string()))?;
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(1)
            .enable_all()
            .thread_name("chronos-webhook")
            .build()
            .map_err(|e| WebhookError::RuntimeUnavailable(e.to_string()))?;
        let handle = runtime.handle().clone();
        Ok(Self {
            default_url: Arc::from(default_url.to_string()),
            client: Arc::new(client),
            runtime: Some(runtime),
            handle,
            config,
        })
    }

    fn pick_url<'a>(&'a self, request: &'a NotificationRequest) -> &'a str {
        if self.config.allow_target_override {
            if let Some(target) = &request.target {
                if !target.id.is_empty() {
                    return target.id.as_str();
                }
            }
        }
        &self.default_url
    }

    fn run_delivery(
        &self,
        url: &str,
        envelope: WebhookEnvelope<'_>,
    ) -> Result<(), NotificationDeliveryError> {
        let client = self.client.clone();
        let config = self.config.clone();
        let url_owned = url.to_string();

        // The runtime lives on; if we have shut it down, fail loudly
        // rather than panicking inside `block_on`.
        let _guard =
            self.runtime
                .as_ref()
                .ok_or_else(|| NotificationDeliveryError::Unavailable {
                    detail: "webhook runtime has been shut down".to_string(),
                })?;

        self.handle.block_on(async move {
            let mut delay = config.initial_backoff;
            for attempt in 0..config.max_attempts {
                let res = client.post(&url_owned).json(&envelope).send().await;
                match res {
                    Ok(resp) if resp.status().is_success() => {
                        info!(attempt = attempt + 1, "webhook delivered");
                        return Ok(());
                    }
                    Ok(resp) if resp.status().is_client_error() => {
                        return Err(NotificationDeliveryError::Configuration {
                            detail: format!("HTTP {}", resp.status().as_u16()),
                        });
                    }
                    Ok(resp) => {
                        warn!(
                            attempt = attempt + 1,
                            status = resp.status().as_u16(),
                            "webhook non-success"
                        );
                    }
                    Err(e) => {
                        warn!(attempt = attempt + 1, error = %e, "webhook failed");
                    }
                }
                if attempt + 1 < config.max_attempts {
                    sleep(delay).await;
                    delay *= 2;
                }
            }
            Err(NotificationDeliveryError::Transient {
                detail: format!("all {} attempts failed", config.max_attempts),
            })
        })
    }
}

impl Drop for HttpWebhookSink {
    fn drop(&mut self) {
        if let Some(rt) = self.runtime.take() {
            // Best-effort shutdown. shutdown_timeout returns () in the
            // tokio version this crate is built against; the call still
            // bounds the wait so a stuck worker cannot hang Drop.
            rt.shutdown_timeout(Duration::from_secs(1));
        }
    }
}

impl NotificationSink for HttpWebhookSink {
    fn deliver(&self, request: &NotificationRequest) -> Result<(), NotificationDeliveryError> {
        let url = self.pick_url(request);
        let envelope = WebhookEnvelope::from(request);
        self.run_delivery(url, envelope)
    }
}

/// Wire envelope. This is the JSON shape the receiver will see. The
/// `extras` bytes are passed through verbatim so the domain layer
/// never has to know JSON.
#[derive(Debug, Serialize)]
struct WebhookEnvelope<'a> {
    kind: &'a str,
    tripwire_id: &'a str,
    condition: &'a str,
    fired_at_ns: u64,
    extras: Option<&'a [u8]>,
}

impl<'a> From<&'a NotificationRequest> for WebhookEnvelope<'a> {
    fn from(req: &'a NotificationRequest) -> Self {
        Self {
            kind: &req.kind,
            tripwire_id: &req.tripwire_id,
            condition: &req.condition_description,
            fired_at_ns: req.timestamp_ns,
            extras: req.extras.as_deref(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    use http_body_util::Full;
    use hyper::body::Bytes;
    use hyper::server::conn::http1;
    use hyper::service::service_fn;
    use hyper::{Request, Response};
    use hyper_util::rt::TokioIo;
    use tokio::net::TcpListener;

    /// Spawn a tiny hyper-based server that returns a fixed status code
    /// and counts the requests it received.
    fn spawn_status_server(status_code: u16) -> (u16, Arc<AtomicUsize>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
        let port = listener.local_addr().expect("local_addr").port();
        listener.set_nonblocking(true).expect("nonblocking");

        let counter = Arc::new(AtomicUsize::new(0));
        let counter_for_handler = counter.clone();

        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("rt");
            rt.block_on(async move {
                let listener = TcpListener::from_std(listener).expect("listener");
                loop {
                    let (stream, _) = match listener.accept().await {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    let counter = counter_for_handler.clone();
                    let status = status_code;
                    let io = TokioIo::new(stream);
                    tokio::spawn(async move {
                        let svc = service_fn(move |_req: Request<hyper::body::Incoming>| {
                            counter.fetch_add(1, Ordering::SeqCst);
                            async move {
                                Ok::<_, std::convert::Infallible>(
                                    Response::builder()
                                        .status(status)
                                        .body(Full::new(Bytes::new()))
                                        .unwrap(),
                                )
                            }
                        });
                        let _ = http1::Builder::new().serve_connection(io, svc).await;
                    });
                }
            });
        });
        // Give the listener thread a moment to start accepting.
        std::thread::sleep(Duration::from_millis(50));
        (port, counter)
    }

    fn fixture_request() -> NotificationRequest {
        NotificationRequest {
            kind: "tripwire-fired".to_string(),
            tripwire_id: "tripwire-1".to_string(),
            condition_description: "Signal(11)".to_string(),
            timestamp_ns: 1_700_000_000_000_000_000,
            target: None,
            extras: None,
        }
    }

    #[test]
    fn successful_delivery_makes_one_request() {
        let (port, counter) = spawn_status_server(200);
        let url = format!("http://127.0.0.1:{}/hook", port);
        let sink = HttpWebhookSink::new(&url).expect("sink");
        let req = fixture_request();
        sink.deliver(&req).expect("delivery");
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn client_error_4xx_is_configuration_not_transient() {
        let (port, counter) = spawn_status_server(404);
        let url = format!("http://127.0.0.1:{}/hook", port);
        let sink = HttpWebhookSink::new(&url).expect("sink");
        let req = fixture_request();
        let err = sink.deliver(&req).expect_err("4xx must fail");
        assert!(
            matches!(err, NotificationDeliveryError::Configuration { .. }),
            "expected Configuration variant, got {:?}",
            err
        );
        // 4xx is a hard fail — no retry.
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn transient_5xx_exhausts_retries_with_backoff() {
        let (port, counter) = spawn_status_server(503);
        let url = format!("http://127.0.0.1:{}/hook", port);
        // Lower backoff to keep the test fast: 100ms → 200ms → ...
        let cfg = WebhookConfig {
            max_attempts: 3,
            initial_backoff: Duration::from_millis(100),
            request_timeout: Duration::from_secs(1),
            allow_target_override: true,
        };
        let sink = HttpWebhookSink::with_config(&url, cfg).expect("sink");
        let req = fixture_request();
        let start = std::time::Instant::now();
        let err = sink.deliver(&req).expect_err("5xx after retries must fail");
        let elapsed = start.elapsed();
        assert!(
            matches!(err, NotificationDeliveryError::Transient { .. }),
            "expected Transient variant, got {:?}",
            err
        );
        // 100ms + 200ms = ~300ms minimum wall time for 3 attempts.
        assert!(
            elapsed >= Duration::from_millis(300),
            "expected at least 300ms of backoff, got {:?}",
            elapsed
        );
        assert_eq!(counter.load(Ordering::SeqCst), 3);
    }

    #[test]
    fn target_override_is_honoured() {
        let (port_a, counter_a) = spawn_status_server(200);
        let (port_b, counter_b) = spawn_status_server(200);
        let url_a = format!("http://127.0.0.1:{}/hook", port_a);
        let url_b = format!("http://127.0.0.1:{}/hook", port_b);
        let sink = HttpWebhookSink::new(&url_a).expect("sink");
        let mut req = fixture_request();
        req.target = Some(chronos_domain::NotificationTarget { id: url_b.clone() });
        sink.deliver(&req).expect("delivery");
        assert_eq!(counter_a.load(Ordering::SeqCst), 0);
        assert_eq!(counter_b.load(Ordering::SeqCst), 1);
    }
}
