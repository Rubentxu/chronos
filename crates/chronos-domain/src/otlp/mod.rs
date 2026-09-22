//! M6.1 lift: W3C Trace Context types for inbound/outbound correlation.
//!
//! This module mirrors the durable M6.1 spike (`/home/rubentxu/m6-spikes/m6.1-skeleton/`)
//! but uses **parallel** identifiers so it does NOT touch the product's
//! `InvocationId` (chronos-domain::trace::event::InvocationId, which is
//! UUID v7 time-ordered per ADR-0004 §M6.1). The two are kept distinct by
//! design — see `OtlpInvocationId` vs the existing `InvocationId`.
//!
//! Architectural decision: unify or stay parallel is deferred. See
//! `docs/milestones/M6-CLOSE.md` §5 "honest limitations".

pub mod ingest;
pub mod parse;

use uuid::Uuid;

/// External-trace invocation id (W3C §3.2 — UUID v4 random per spike contract).
///
/// Deliberately distinct from `chronos_domain::trace::event::InvocationId`
/// (UUID v7 time-ordered). They serve different roles:
///
/// - `chronos_domain::trace::event::InvocationId` — internal Chronos work-unit
///   id, sortable by capture time (used for analytics, replay, perturbation).
/// - `otlp::OtlpInvocationId` — opaque correlation id minted at the W3C
///   ingest boundary; only carried inside `RecordedInvocation`.
///
/// ADR-0004 §M6.2 forbids wall-clock in M6.* identifiers, so this stays
/// UUID v4 (random) and never leaks the capture timestamp.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OtlpInvocationId(Uuid);

impl OtlpInvocationId {
    /// Mint a fresh `OtlpInvocationId` (UUID v4 random).
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    /// Borrow the underlying [`Uuid`].
    pub fn as_uuid(&self) -> Uuid {
        self.0
    }
}

impl Default for OtlpInvocationId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for OtlpInvocationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.hyphenated())
    }
}

/// 16-byte trace identifier (W3C §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraceId([u8; 16]);

impl TraceId {
    /// Borrow the raw 16 bytes.
    pub fn bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// `true` iff every byte is zero (invalid per W3C §3.2.2.3).
    pub fn is_all_zero(&self) -> bool {
        self.0 == [0u8; 16]
    }

    /// Construct from an explicit byte array (used by [`parse::parse_traceparent`]
    /// and advanced integration tests that build synthetic contexts).
    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }
}

/// 8-byte span identifier (W3C §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpanId(pub [u8; 8]);

impl SpanId {
    /// Borrow the raw 8 bytes.
    pub fn bytes(&self) -> &[u8; 8] {
        &self.0
    }

    /// `true` iff every byte is zero (invalid per W3C §3.2.2.4).
    pub fn is_all_zero(&self) -> bool {
        self.0 == [0u8; 8]
    }

    /// Construct from an explicit byte array (used by [`parse::parse_traceparent`]).
    pub fn from_bytes(bytes: [u8; 8]) -> Self {
        Self(bytes)
    }
}

/// W3C trace flags. Bit 0 is the "sampled" flag (W3C §3.2.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TraceFlags(u8);

impl TraceFlags {
    /// `true` iff the sampled flag is set.
    pub fn sampled(&self) -> bool {
        self.0 & 0x01 != 0
    }

    /// Raw flag bits (8 bits, only bit 0 has defined semantics today).
    pub fn bits(&self) -> u8 {
        self.0
    }
}

/// Vendor list-key state (W3C §3.3.1).
///
/// Stores `(vendor, value)` pairs in insertion order. Max 32 entries
/// (W3C §3.3.1.1). Vendor charset: lowercase alphanum + `-_./*`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Tracestate {
    entries: Vec<(String, String)>,
}

impl Tracestate {
    /// Borrow the underlying `(vendor, value)` pairs.
    pub fn entries(&self) -> &[(String, String)] {
        &self.entries
    }

    /// Number of vendor entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` iff there are no vendor entries.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl std::fmt::Display for Tracestate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for (k, v) in &self.entries {
            if !first {
                write!(f, ",")?;
            }
            first = false;
            write!(f, "{}={}", k, v)?;
        }
        Ok(())
    }
}

/// External trace context. Constructible only via [`ExternalTraceContext::root`]
/// or [`parse::parse_traceparent`].
///
/// The product-side `chronos_domain::trace::event::InvocationId` is **never**
/// carried inside this type; correlation happens through
/// [`RecordedInvocation`] which pairs an `OtlpInvocationId` with an optional
/// `ExternalTraceContext`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExternalTraceContext {
    trace_id: TraceId,
    span_id: SpanId,
    flags: TraceFlags,
    tracestate: Option<Tracestate>,
}

impl ExternalTraceContext {
    /// A root context (all-zero IDs). Use when no inbound `traceparent` header
    /// is present; the resulting context represents a freshly-started trace.
    pub fn root() -> Self {
        Self {
            trace_id: TraceId([0; 16]),
            span_id: SpanId([0; 8]),
            flags: TraceFlags(0),
            tracestate: None,
        }
    }

    /// Borrow the trace id.
    pub fn trace_id(&self) -> &TraceId {
        &self.trace_id
    }

    /// Borrow the span id.
    pub fn span_id(&self) -> &SpanId {
        &self.span_id
    }

    /// Borrow the trace flags.
    pub fn flags(&self) -> &TraceFlags {
        &self.flags
    }

    /// Borrow the tracestate (None when no vendor list is present).
    pub fn tracestate(&self) -> Option<&Tracestate> {
        self.tracestate.as_ref()
    }

    /// `true` iff both IDs are all-zero (root context).
    pub fn is_root(&self) -> bool {
        self.trace_id.is_all_zero() && self.span_id.is_all_zero()
    }

    /// Attach a vendor list. Returns `self` for builder-style chaining.
    pub fn with_tracestate(mut self, ts: Tracestate) -> Self {
        self.tracestate = Some(ts);
        self
    }

    /// Render the W3C `traceparent` header value (version `00`, 32 hex
    /// trace-id, 16 hex parent-id, 2 hex flags).
    pub fn to_traceparent(&self) -> String {
        format!(
            "00-{}-{}-{}",
            hex_lower(&self.trace_id.0),
            hex_lower(&self.span_id.0),
            hex_lower_byte(self.flags.0)
        )
    }

    /// Render the W3C `tracestate` header value. Empty string when no
    /// vendor list is attached.
    pub fn to_tracestate(&self) -> String {
        match &self.tracestate {
            None => String::new(),
            Some(ts) => ts.to_string(),
        }
    }
}

// Internal constructors used by the `parse` submodule only.
pub fn new_external_context(
    trace_id: TraceId,
    span_id: SpanId,
    flags: TraceFlags,
    tracestate: Option<Tracestate>,
) -> ExternalTraceContext {
    ExternalTraceContext {
        trace_id,
        span_id,
        flags,
        tracestate,
    }
}

pub fn new_trace_id(bytes: [u8; 16]) -> TraceId {
    TraceId(bytes)
}

pub fn new_span_id(bytes: [u8; 8]) -> SpanId {
    SpanId(bytes)
}

pub fn new_trace_flags(bits: u8) -> TraceFlags {
    TraceFlags(bits)
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn hex_lower_byte(b: u8) -> String {
    format!("{:02x}", b)
}

/// Pairing of an internal correlation id with an optional external trace
/// context (W3C).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RecordedInvocation {
    /// Internal correlation id minted at the ingest boundary.
    pub invocation_id: OtlpInvocationId,
    /// External context, present when the inbound request carried a valid
    /// `traceparent` header. `None` for root-only (internal) traces.
    pub external: Option<ExternalTraceContext>,
}

impl OtlpInvocationId {
    /// Pair this id with an external context.
    pub fn record_external(&self, etc: &ExternalTraceContext) -> RecordedInvocation {
        RecordedInvocation {
            invocation_id: *self,
            external: Some(etc.clone()),
        }
    }
}

impl RecordedInvocation {
    /// Build an internal-only pairing (no external context attached).
    pub fn internal_only(invocation_id: OtlpInvocationId) -> Self {
        Self {
            invocation_id,
            external: None,
        }
    }

    /// Serialise as `invocation|traceparent|tracestate`. Stable wire format
    /// matching the M6.1 spike's `to_recorded` so existing observability
    /// tools that consumed the spike continue to work.
    pub fn to_recorded(&self) -> String {
        let invocation = self.invocation_id.to_string();
        let traceparent = self
            .external
            .as_ref()
            .map(|e| e.to_traceparent())
            .unwrap_or_default();
        let tracestate = self
            .external
            .as_ref()
            .map(|e| e.to_tracestate())
            .unwrap_or_default();
        format!("{}|{}|{}", invocation, traceparent, tracestate)
    }
}
