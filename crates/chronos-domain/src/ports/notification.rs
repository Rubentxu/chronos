//! `NotificationSink` — the domain's outbound port for event delivery.
//!
//! The domain declares this contract; infrastructure crates (HTTP webhooks,
//! message queues, log forwarders, etc.) implement it. The domain knows
//! nothing about HTTP, retries, timeouts, or transport-specific payloads.
//!
//! See `crates/chronos-webhook` for the canonical driven adapter that
//! implements this port over HTTPS with exponential-backoff retries.

use thiserror::Error;

/// A request to deliver a domain event to an external observer.
///
/// The struct is transport-neutral: the adapter is responsible for
/// mapping these fields into whatever envelope the transport expects
/// (JSON over HTTP, protobuf over gRPC, syslog, etc.).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NotificationRequest {
    /// Stable identifier of the notification category. Today only
    /// `tripwire-fired` exists, but the field is open to other domain
    /// events (e.g. `property-violated`) without breaking adapters.
    pub kind: String,
    /// Stable identifier of the fired tripwire (when `kind == "tripwire-fired"`).
    pub tripwire_id: String,
    /// Human-readable description of the condition that produced the event.
    pub condition_description: String,
    /// Monotonic timestamp (nanoseconds since the trace capture start).
    pub timestamp_ns: u64,
    /// Optional target hint for adapters that support per-target routing
    /// (e.g. webhook URLs). Adapters that do not need it may ignore.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<NotificationTarget>,
    /// Optional free-form payload for adapter-specific needs (kept as a
    /// pre-serialized byte slice so the domain does not have to depend
    /// on any JSON crate; adapters interpret the bytes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extras: Option<Vec<u8>>,
}

/// Optional per-request routing hint. Adapters that do not have
/// per-target configuration can ignore it (the default null sink does).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct NotificationTarget {
    /// Opaque identifier of the target as known to the adapter
    /// (e.g. a webhook URL string, a queue name, etc.).
    pub id: String,
}

/// Errors a sink can surface back to the domain layer.
///
/// Variants are transport-neutral; the adapter implementation maps its
/// own failure modes (HTTP status, DNS error, timeout, etc.) into
/// these categories. The domain only needs to know whether delivery
/// *might* succeed on retry, *definitely* failed, or *configuration*
/// was wrong.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NotificationDeliveryError {
    /// The adapter could not deliver the notification right now, but
    /// the failure is potentially transient (network, timeout, 5xx).
    /// Callers may retry.
    #[error("transient delivery failure: {detail}")]
    Transient {
        /// Adapter-supplied detail (no transport-specific jargon).
        detail: String,
    },
    /// The adapter rejected the request because of a configuration
    /// error (invalid URL, missing credentials, malformed target).
    /// Retrying without changing configuration will not help.
    #[error("configuration error: {detail}")]
    Configuration {
        /// Adapter-supplied detail.
        detail: String,
    },
    /// The adapter is shutting down or otherwise unavailable for an
    /// indefinite period. Callers should drop the notification or
    /// route to a fallback sink.
    #[error("adapter unavailable: {detail}")]
    Unavailable {
        /// Adapter-supplied detail.
        detail: String,
    },
}

/// The domain's outbound port.
///
/// Driven adapters implement this trait; the composition root injects
/// the chosen implementation into the application. The trait is
/// **synchronous**: the adapter decides how to schedule work (blocking,
/// thread pool, runtime handle, etc.).
///
/// Implementations must be:
/// - **Idempotent-friendly**: a `Transient` error should not imply
///   "the notification was lost"; callers may retry.
/// - **Send + Sync**: the domain layer is shared across threads.
pub trait NotificationSink: Send + Sync {
    /// Attempt to deliver one notification. The semantics of partial
    /// failure (e.g. multi-target fan-out) are adapter-defined; for the
    /// default webhook adapter, the request has exactly one target.
    fn deliver(
        &self,
        request: &NotificationRequest,
    ) -> Result<(), NotificationDeliveryError>;
}

/// The default no-op sink. Useful for tests, for the
/// `--no-notifications` runtime flag, and for environments that have
/// not configured any outbound sink yet.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullNotificationSink;

impl NotificationSink for NullNotificationSink {
    fn deliver(
        &self,
        _request: &NotificationRequest,
    ) -> Result<(), NotificationDeliveryError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn null_sink_accepts_any_request() {
        let sink = NullNotificationSink;
        let req = NotificationRequest {
            kind: "tripwire-fired".to_string(),
            tripwire_id: "tripwire-1".to_string(),
            condition_description: "Signal(11)".to_string(),
            timestamp_ns: 1_700_000_000_000_000_000,
            target: None,
            extras: None,
        };
        assert!(sink.deliver(&req).is_ok());
    }

    #[test]
    fn notification_request_roundtrip_via_serde_json() {
        let req = NotificationRequest {
            kind: "tripwire-fired".to_string(),
            tripwire_id: "tripwire-42".to_string(),
            condition_description: "FunctionName(process_*)".to_string(),
            timestamp_ns: 42,
            target: Some(NotificationTarget {
                id: "https://example.com/webhook".to_string(),
            }),
            extras: Some(b"{\"session_id\":\"abc\"}".to_vec()),
        };
        let json = serde_json::to_string(&req).expect("serialize");
        let back: NotificationRequest = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, req);
    }

    #[test]
    fn transient_error_is_distinct_from_configuration_and_unavailable() {
        // The variants must be distinguishable so callers can pick the
        // right retry policy. The textual content of `detail` is owned
        // by the adapter (and may legitimately surface transport
        // diagnostics), so the contract here is variant-level, not
        // string-level.
        let transient = NotificationDeliveryError::Transient {
            detail: "upstream 503".to_string(),
        };
        let configuration = NotificationDeliveryError::Configuration {
            detail: "invalid url".to_string(),
        };
        let unavailable = NotificationDeliveryError::Unavailable {
            detail: "shutdown".to_string(),
        };

        // Different variants are not equal even if the detail matched.
        assert_ne!(transient, configuration);
        assert_ne!(transient, unavailable);
        assert_ne!(configuration, unavailable);

        // The variant name appears in the Display output so log
        // readers can triage failures.
        assert!(transient.to_string().contains("transient"));
        assert!(configuration.to_string().contains("configuration"));
        assert!(unavailable.to_string().contains("unavailable"));
    }
}
