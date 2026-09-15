//! `chronos-webhook` — driven adapter implementing
//! [`chronos_domain::ports::NotificationSink`] over HTTPS.
//!
//! This crate lives outside the domain layer on purpose. It owns every
//! transport concern (reqwest client, retry/backoff, timeouts, HTTPS
//! validation, JSON serialization of the wire envelope). The domain
//! layer exposes the intent via the [`NotificationSink`] port; this
//! adapter is the only place where HTTP lives.
//!
//! The trait method is synchronous in the domain; the adapter owns a
//! dedicated multi-thread tokio runtime internally so `deliver` can
//! drive the async reqwest client from any caller context (sync CLI
//! handler, async MCP worker, …) without coupling to that context's
//! runtime.
//!
//! # Example
//!
//! ```no_run
//! use chronos_domain::{NotificationRequest, NotificationSink};
//! use chronos_webhook::HttpWebhookSink;
//!
//! let sink = HttpWebhookSink::new("https://example.com/webhook").unwrap();
//! let req = NotificationRequest {
//!     kind: "tripwire-fired".to_string(),
//!     tripwire_id: "tripwire-1".to_string(),
//!     condition_description: "Signal(11)".to_string(),
//!     timestamp_ns: 0,
//!     target: None,
//!     extras: None,
//! };
//! let _ = sink.deliver(&req);
//! ```

mod sink;

pub use sink::{HttpWebhookSink, WebhookConfig, WebhookError};
