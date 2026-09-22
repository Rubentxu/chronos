//! M6.2 lift: W3C ingest adapter (header → [`IngestOutcome`]).
//!
//! This lifts `/home/rubentxu/m6-spikes/m6.2-otlp-adapter/` (the adapter
//! half, not the HTTP server half) into the canonical product tree.
//!
//! What is lifted (pure functions, no I/O):
//!   - [`Headers`] — minimal case-insensitive header container.
//!   - [`IngestOutcome`] — the 4-outcome contract (Recorded / InternalOnly
//!     / BadTraceparent / BadTracestate).
//!   - [`adapter`] — header → outcome mapping.
//!   - [`render_response`] — outcome → minimal HTTP/1.1 wire-format
//!     response (so any HTTP server implementation can write it).
//!
//! What is NOT lifted (deliberately deferred, see
//! `docs/milestones/M6-CLOSE.md` §5 honest limitations):
//!   - The TCP server (`serve_on_port`) — the product's HTTP transport is
//!     an operator's choice (axum/hyper/warp/actix), per ADR-0033 §2.2
//!     "telemetry blueprint = forward-compatible wire contracts, not
//!     transport implementations".
//!
//! The 11 spike tests at `/home/rubentxu/m6-spikes/m6.2-otlp-adapter/tests/cases.rs`
//! have header-only cases that ARE covered here; the 4 HTTP-server cases
//! remain spike-only fixtures until a transport is wired.

use super::parse::{parse_tracestate, TraceparentParseError, TracestateParseError};
use super::{OtlpInvocationId, RecordedInvocation};

/// What happened when the adapter ingested a set of headers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestOutcome {
    /// A valid `traceparent` (and optional valid `tracestate`) was attached
    /// to a fresh internal id.
    Recorded(RecordedInvocation),
    /// No `traceparent` was supplied; the invocation is internal-only.
    InternalOnly(RecordedInvocation),
    /// `traceparent` was present but failed to parse (with the raw value and
    /// the underlying error for diagnostics).
    BadTraceparent {
        /// The raw header value that failed to parse.
        raw: String,
        /// The parser error.
        error: TraceparentParseError,
    },
    /// `tracestate` was present but failed to parse (with the raw value
    /// and the underlying error). `traceparent` was either absent or
    /// parsed successfully.
    BadTracestate {
        /// The raw header value that failed to parse.
        raw: String,
        /// The parser error.
        error: TracestateParseError,
    },
}

impl IngestOutcome {
    /// HTTP status code associated with this outcome (200 / 400).
    pub fn status_code(&self) -> u16 {
        match self {
            Self::Recorded(_) | Self::InternalOnly(_) => 200,
            Self::BadTraceparent { .. } | Self::BadTracestate { .. } => 400,
        }
    }

    /// `true` iff the outcome is `Recorded` or `InternalOnly`.
    pub fn is_ok(&self) -> bool {
        matches!(self, Self::Recorded(_) | Self::InternalOnly(_))
    }
}

/// Minimal header container, case-insensitive lookup.
///
/// Only used by the adapter. Real HTTP transports (axum/hyper/warp) own
/// their own header representations; this type exists so the adapter can
/// be unit-tested without an HTTP server.
#[derive(Debug, Default, Clone)]
pub struct Headers {
    /// Header entries in insertion order.
    pub entries: Vec<(String, String)>,
}

impl Headers {
    /// Parse a header block (RFC 7230 §3.2 — `\r\n`-separated `k: v` lines)
    /// into a [`Headers`] container. Empty lines and lines without a colon
    /// are silently skipped.
    pub fn parse_block(block: &str) -> Self {
        let mut entries = Vec::new();
        for line in block.split("\r\n") {
            if line.is_empty() {
                continue;
            }
            if let Some(idx) = line.find(':') {
                let k = line[..idx].trim().to_ascii_lowercase();
                let v = line[idx + 1..].trim().to_string();
                entries.push((k, v));
            }
        }
        Self { entries }
    }

    /// Look up a header value (case-insensitive).
    pub fn get(&self, key: &str) -> Option<&str> {
        let key = key.to_ascii_lowercase();
        for (k, v) in &self.entries {
            if k == &key {
                return Some(v.as_str());
            }
        }
        None
    }
}

/// Failure modes for [`adapter`]. Used by tests and any caller that wants
/// to surface the precise reason behind a [`IngestOutcome::BadTraceparent`]
/// or [`IngestOutcome::BadTracestate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IngestError {
    /// `traceparent` failed to parse.
    Traceparent {
        /// Raw value.
        raw: String,
        /// Underlying parser error.
        error: TraceparentParseError,
    },
    /// `tracestate` failed to parse.
    Tracestate {
        /// Raw value.
        raw: String,
        /// Underlying parser error.
        error: TracestateParseError,
    },
}

impl std::fmt::Display for IngestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Traceparent { raw, error } => {
                write!(f, "traceparent '{}' invalid: {}", raw, error)
            }
            Self::Tracestate { raw, error } => {
                write!(f, "tracestate '{}' invalid: {}", raw, error)
            }
        }
    }
}

impl std::error::Error for IngestError {}

/// Internal helper that combines parsing of `traceparent` (required if
/// present) with optional `tracestate` parsing.
fn ingest_traceparent(
    raw_traceparent: Option<&str>,
    raw_tracestate: Option<&str>,
) -> Result<super::ExternalTraceContext, IngestError> {
    let mut etc = match raw_traceparent {
        None => super::ExternalTraceContext::root(),
        Some(s) => super::parse::parse_traceparent(s).map_err(|e| IngestError::Traceparent {
            raw: s.to_string(),
            error: e,
        })?,
    };
    if let Some(s) = raw_tracestate {
        if !s.is_empty() {
            let ts = parse_tracestate(s).map_err(|e| IngestError::Tracestate {
                raw: s.to_string(),
                error: e,
            })?;
            etc = etc.with_tracestate(ts);
        }
    }
    Ok(etc)
}

/// Adapter contract: headers → outcome.
///
/// Pure function — no I/O, no time, no global state. Mints a fresh
/// [`OtlpInvocationId`] for every call. The caller is responsible for
/// choosing transport (HTTP loopback, stdin pipe, etc.).
pub fn adapter(headers: &Headers) -> IngestOutcome {
    let raw_tp = headers.get("traceparent");
    let raw_ts = headers.get("tracestate");
    let invocation = OtlpInvocationId::new();
    match ingest_traceparent(raw_tp, raw_ts) {
        Ok(etc) => {
            if etc.is_root() {
                IngestOutcome::InternalOnly(RecordedInvocation::internal_only(invocation))
            } else {
                IngestOutcome::Recorded(invocation.record_external(&etc))
            }
        }
        Err(IngestError::Traceparent { raw, error }) => {
            IngestOutcome::BadTraceparent { raw, error }
        }
        Err(IngestError::Tracestate { raw, error }) => IngestOutcome::BadTracestate { raw, error },
    }
}

/// Render an [`IngestOutcome`] as a minimal HTTP/1.1 response. The body
/// is plain text with one of:
///
///   - `recorded\n<rec>\n` for [`IngestOutcome::Recorded`]
///   - `internal-only\n<rec>\n` for [`IngestOutcome::InternalOnly`]
///   - `bad-traceparent\nraw: <raw>\nerror: <msg>\n` for [`IngestOutcome::BadTraceparent`]
///   - `bad-tracestate\nraw: <raw>\nerror: <msg>\n` for [`IngestOutcome::BadTracestate`]
///
/// Content-Length is correct for the body length, Connection is `close`
/// (one request per connection — same as the spike's TCP server).
pub fn render_response(outcome: &IngestOutcome) -> String {
    let status = outcome.status_code();
    let reason = match status {
        200 => "OK",
        400 => "Bad Request",
        _ => "OK",
    };
    let body = match outcome {
        IngestOutcome::Recorded(r) => format!("recorded\n{}\n", r.to_recorded()),
        IngestOutcome::InternalOnly(r) => format!("internal-only\n{}\n", r.to_recorded()),
        IngestOutcome::BadTraceparent { raw, error } => {
            format!("bad-traceparent\nraw: {}\nerror: {}\n", raw, error)
        }
        IngestOutcome::BadTracestate { raw, error } => {
            format!("bad-tracestate\nraw: {}\nerror: {}\n", raw, error)
        }
    };
    format!(
        "HTTP/1.1 {} {}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        status, reason, body.len(), body
    )
}
