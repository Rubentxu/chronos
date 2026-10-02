//! M6.4 lift: opt-in OTel JSON Lines export with declared limits.
//!
//! Source of truth: ADR-0018. The export contract is
//! `export(store, events, filter, limits) -> ExportResult`. It is the only
//! *complete* export path, and there is no `export_all()`: an event leaves
//! chronos only when the caller named it in an [`OptInFilter`].
//!
//! The opt-in itself lives one level down, in [`export_spans`], which every
//! path goes through — so "no unfiltered export" holds for the seam as well,
//! not just for this wrapper. The product pipeline
//! ([`super::gates::run_service_pipeline`]) calls [`export_spans`] and renders
//! itself because redaction has to act on a span before it becomes text;
//! that composition is deliberate, not a second export path.
//!
//! ## What the spike got wrong about the product
//!
//! The M6.4 spike was written against its own `m6_1`/`m6_3` crates, so two
//! of its assumptions do not hold once the code lands in `chronos-domain`.
//! Both are load-bearing, so both are spelled out here instead of being
//! silently inherited.
//!
//! **1. `ChronosEvent::probe` is the name channel, and it is not
//! `TraceEvent`'s.** `TraceEvent` (`trace::event`) has no probe field; its
//! discriminant is `EventType`. The exporter's input is `ChronosEvent`, the
//! post-processed correlation view, whose `probe: String` is the product's
//! name channel — see [`probe_name`] for how a real `TraceEvent` maps onto it.
//!
//! **2. `ts_micros * 1000` is not Unix time.** ADR-0018 §2.3 states the
//! exporter emits `start_time_unix_nano = ts_micros * 1000` because "the
//! stream producer is responsible for setting the epoch". In the product
//! nothing sets that epoch: `ChronosEvent::ts_micros` is documented as
//! session-monotonic microseconds and `TraceEvent::timestamp_ns` is
//! `MonotonicNs` (nanoseconds since session start), a different clock domain
//! from the wall clock. The product already ruled on this in commit
//! `32fb5ab0` (`chronos-services::session_export::serialize_bundle_otlp_json`
//! sets `start_time_unix_nano = "0"` and carries the real monotonic value in
//! `chronos.timestamp_monotonic_ns`, with `chronos.timestamp_domain` naming
//! the domain so a collector detects it instead of reading 0 as an epoch).
//! Emitting the multiplication into `start_time_unix_nano` would reintroduce
//! exactly that corruption at a second call site, so this exporter keeps the
//! product convention: the canonical field is `0` and the monotonic value is
//! disclosed as an attribute, repeated per span because JSON Lines has no
//! resource envelope to hang a resource-level attribute on.
//!
//! ## Scope
//!
//! No OTLP/HTTP transport, no batching, no retry, no parent-child spans, no
//! cardinality policy — ADR-0018 §6 places all of those downstream. Redaction
//! is likewise not wired *here*: `otlp::redaction::redact_and_limit_attributes`
//! consumes the `Vec<(String, String)>` on [`ExportedSpan`], so integrating M6.5
//! is a policy call, not a renderer change. That call already exists — it is
//! [`super::gates::run_service_pipeline`], which acts on the spans returned by
//! [`export_spans`] before rendering. [`export`] alone still renders unredacted,
//! because it is the renderer, not the policy.

use std::collections::HashMap;

use uuid::Uuid;

use super::correlation::{ChronosEvent, CorrelationStore, EventField};
use super::{OtlpInvocationId, RecordedInvocation, SpanId, TraceId};
use crate::trace::TraceEvent;

/// Clock domain disclosed on every emitted span. Same literal as
/// `chronos-services::session_export`, so a consumer that already knows how
/// to interpret the bundle export needs no second rule.
pub const TIMESTAMP_DOMAIN: &str = "monotonic_ns_from_session_start";

/// Trace id emitted for invocations with no inbound W3C context (ADR-0018
/// §2.5). All-zero is the W3C "no trace" value; `chronos_invocation_id` is
/// still emitted so the span stays correlatable back to chronos.
const ZERO_TRACE_ID_HEX: &str = "00000000000000000000000000000000";

/// Span-name prefix. Keeps chronos spans identifiable in a collector that
/// also receives spans from other producers.
const SPAN_NAME_PREFIX: &str = "chronos.event.";

/// Attribute keys the exporter itself contributes. They are not event fields,
/// so they never count towards [`ExportResult::truncated_fields`] (which
/// reports field truncation only).
const ATTR_PROBE: &str = "probe";
const ATTR_TIMESTAMP_DOMAIN: &str = "chronos.timestamp_domain";
const ATTR_TIMESTAMP_MONOTONIC_NS: &str = "chronos.timestamp_monotonic_ns";

/// Caller-driven opt-in: which events may leave chronos.
///
/// Exact match on the probe name — no prefix, glob or category matching. A
/// caller that wants `function_entry` gets exactly `function_entry`; the
/// alternative (matching a payload subject) would make the allow-list grow
/// with the program under observation, which is the opposite of an opt-in
/// gate.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OptInFilter {
    allowed: Vec<String>,
}

impl OptInFilter {
    /// Allow exactly these probe names.
    pub fn new(allowed: Vec<String>) -> Self {
        Self { allowed }
    }

    /// Allow nothing. The default state of a fresh filter, which is why
    /// `Default` is derived from the same empty vector (ADR-0018 §3.3).
    pub fn empty() -> Self {
        Self {
            allowed: Vec::new(),
        }
    }

    /// `true` iff `name` is on the allow-list.
    pub fn allows(&self, name: &str) -> bool {
        self.allowed.iter().any(|allowed| allowed == name)
    }
}

/// Declared caps for one export call (ADR-0018 §2.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExportLimits {
    /// Hard cap per `RecordedInvocation` in this export call.
    pub max_events_per_invocation: usize,
    /// Global cap per export call.
    pub max_events_total: usize,
    /// Per-string-field truncation cap, in bytes of rendered value.
    pub max_field_chars: usize,
}

impl Default for ExportLimits {
    fn default() -> Self {
        Self {
            max_events_per_invocation: 100,
            max_events_total: 1000,
            max_field_chars: 1024,
        }
    }
}

/// One exported span, before JSON rendering.
///
/// Kept as a distinct step from rendering so a downstream policy (M6.5
/// redaction) can inspect or rewrite attributes without re-deriving the span
/// identity — `trace_id`/`span_id`/`name` are already decided here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedSpan {
    /// W3C trace id, 32 lowercase hex chars, no dashes. All-zero for
    /// internal-only invocations.
    pub trace_id_hex: String,
    /// Span id derived from the correlation stream index.
    pub span_id_hex: String,
    /// `chronos.event.{probe}`.
    pub name: String,
    /// Always `0`; see the module docs on the temporal domain.
    pub start_time_unix_nano: u64,
    /// Chronos correlation id in hyphenated UUID form.
    pub chronos_invocation_id: String,
    /// OTel common-attribute encoding; every value is a string.
    pub attributes: Vec<(String, String)>,
}

/// Outcome of one export call: the JSON Lines payload plus one counter per
/// skip reason (ADR-0018 §3.4 — no silent defaults, so every drop is
/// attributed to a named counter).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportResult {
    /// One JSON object per emitted span, in emission order.
    pub lines: Vec<String>,
    /// Spans emitted.
    pub emitted: usize,
    /// Events dropped because the filter did not allow their probe.
    pub skipped_probe_filter: usize,
    /// Events dropped by the per-invocation cap.
    pub skipped_limit_per_invocation: usize,
    /// Events dropped by the global cap.
    pub skipped_limit_total: usize,
    /// Event fields shortened by the field cap.
    pub truncated_fields: usize,
}

/// Probe name for a real `TraceEvent`.
///
/// Measured, not assumed: `TraceEvent` has no probe field, so the only stable
/// event-level identity it carries is `EventType`, whose `Display` is the
/// canonical snake_case spelling that `EventType::from_snake_case` parses
/// back. That round-trip is what makes the name usable as an opt-in token —
/// a caller can copy the string out of a capture and get the same event back.
///
/// The name-bearing `EventData` variants (`Function { name }`,
/// `Syscall { name }`, `Signal { signal_name }`, `Custom { name }`, the
/// language-frame variants, `EbpfUprobeHit { symbol_name }`) name the
/// *subject* of the event, not the trigger that fired. Folding them in would
/// make the opt-in list grow with every function in the program, and would
/// let a payload value decide which spans the caller opted into. Subject
/// detail belongs in attributes, which the exporter already carries.
pub fn probe_name(event: &TraceEvent) -> String {
    event.event_type.to_string()
}

/// Export `events` that are bound in `store` and allowed by `filter`.
///
/// Limit priority, checked in this order (ADR-0018 §2.2): filter, then the
/// per-invocation cap, then the global cap, then field truncation. The order
/// is observable through the counters: an event that would trip two caps is
/// attributed to the first one checked.
///
/// `events` must be the slice the store was built from. A binding whose
/// index is past the end of `events` is neither emitted nor counted —
/// `ExportResult` has no counter for a store/slice mismatch, so this is
/// documented as a caller precondition rather than papered over with a
/// counter that would silently absorb a real inconsistency.
pub fn export(
    store: &CorrelationStore,
    events: &[ChronosEvent],
    filter: &OptInFilter,
    limits: &ExportLimits,
) -> ExportResult {
    let (spans, mut result) = export_spans(store, events, filter, limits);
    for span in spans {
        result.lines.push(render_json_line(&span));
    }
    result
}

/// Select and build the spans an export call would emit, **without**
/// rendering them, together with the counters the selection produced.
///
/// This is the seam [`export`] is built from, exposed for the one caller
/// that must act on a span before it becomes text: M6.5 redaction
/// (`otlp::redaction`) rewrites attribute values, so it has to run
/// between span construction and rendering. Everything else should keep
/// calling [`export`] — the rendered payload is the contract, and
/// `ExportResult::lines` stays empty in the returned counters so a caller
/// that renders later cannot mistake this for a finished export.
pub fn export_spans(
    store: &CorrelationStore,
    events: &[ChronosEvent],
    filter: &OptInFilter,
    limits: &ExportLimits,
) -> (Vec<ExportedSpan>, ExportResult) {
    let mut counters = ExportResult::default();
    let spans = collect_spans(store, events, filter, limits, &mut counters);
    (spans, counters)
}

/// Render the spans as a JSON Lines payload: one object per line, trailing
/// newline. An empty export renders as an empty string, not a lone newline,
/// so "nothing was exported" does not look like "one blank record".
pub fn to_jsonl(result: &ExportResult) -> String {
    let mut out = String::new();
    for line in &result.lines {
        out.push_str(line);
        out.push('\n');
    }
    out
}

/// Render one span as a single-line JSON object with the field order fixed by
/// ADR-0018 §2.3. `parent_span_id` is deliberately absent: ADR-0018 §6 defers
/// parent-child spans, and emitting the key as `null` would teach a collector
/// to expect it.
pub fn render_json_line(span: &ExportedSpan) -> String {
    let attributes = span
        .attributes
        .iter()
        .map(|(key, value)| {
            format!(
                r#"{{"key":{},"value":{}}}"#,
                json_string(key),
                json_string(value)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"{{"trace_id":{},"span_id":{},"name":{},"start_time_unix_nano":{},"chronos_invocation_id":{},"attributes":[{}]}}"#,
        json_string(&span.trace_id_hex),
        json_string(&span.span_id_hex),
        json_string(&span.name),
        span.start_time_unix_nano,
        json_string(&span.chronos_invocation_id),
        attributes,
    )
}

/// Apply the filter and the limits, filling `result` counters as it goes.
fn collect_spans(
    store: &CorrelationStore,
    events: &[ChronosEvent],
    filter: &OptInFilter,
    limits: &ExportLimits,
    result: &mut ExportResult,
) -> Vec<ExportedSpan> {
    let mut spans = Vec::new();
    for indices in group_by_invocation(store) {
        let mut per_invocation_emitted = 0usize;
        for event_idx in indices {
            let Some(event) = events.get(event_idx as usize) else {
                continue;
            };
            if !filter.allows(&event.probe) {
                result.skipped_probe_filter += 1;
                continue;
            }
            if per_invocation_emitted >= limits.max_events_per_invocation {
                result.skipped_limit_per_invocation += 1;
                continue;
            }
            if result.emitted >= limits.max_events_total {
                result.skipped_limit_total += 1;
                continue;
            }
            let Some(recorded) = store.lookup(event_idx) else {
                continue;
            };
            spans.push(render_span(recorded, event, event_idx, limits, result));
            result.emitted += 1;
            per_invocation_emitted += 1;
        }
    }
    spans
}

/// Event indices grouped per invocation, ascending within each group, and the
/// groups themselves ordered by their lowest `event_idx`.
///
/// ADR-0018 §3.1 asks for deterministic output and proposes a `BTreeMap` keyed
/// by invocation id to get it. The goal is sound; that key cannot reach it.
/// `OtlpInvocationId` is UUID **v4 random** by design (`otlp/mod.rs`), so two
/// stores built from the same stream order their groups differently — the map
/// would make the order stable within one store and still vary between runs,
/// which is the weaker property the ADR does not ask for.
///
/// So the ordering key is the group's lowest `event_idx`, which comes from the
/// stream rather than from a random identifier. Each `event_idx` binds to
/// exactly one invocation, so those minima are distinct and the sort is a
/// total order: same stream in, byte-identical lines out.
///
/// `CorrelationStore::bindings` is a `HashMap`, so binding order is sorted
/// first — a group must be ascending for its first element to be its minimum.
fn group_by_invocation(store: &CorrelationStore) -> Vec<Vec<u64>> {
    let mut bindings: Vec<(u64, &RecordedInvocation)> = store.bindings_iter().collect();
    bindings.sort_by_key(|(event_idx, _)| *event_idx);
    let mut by_invocation: HashMap<Uuid, Vec<u64>> = HashMap::new();
    for (event_idx, recorded) in bindings {
        by_invocation
            .entry(recorded.invocation_id.as_uuid())
            .or_default()
            .push(event_idx);
    }
    let mut groups: Vec<Vec<u64>> = by_invocation.into_values().collect();
    // No group is empty (each was created by an insertion) and every first
    // element is the group minimum, so this key is total.
    groups.sort_by_key(|group| group[0]);
    groups
}

/// Render one bound, allowed event into its span form.
fn render_span(
    recorded: &RecordedInvocation,
    event: &ChronosEvent,
    event_idx: u64,
    limits: &ExportLimits,
    result: &mut ExportResult,
) -> ExportedSpan {
    let trace_id_hex = match &recorded.external {
        Some(external) => trace_id_to_hex(external.trace_id()),
        None => ZERO_TRACE_ID_HEX.to_string(),
    };

    let span_id = derive_span_id(&trace_id_hex, &recorded.invocation_id, event_idx);

    let mut attributes: Vec<(String, String)> = Vec::with_capacity(event.fields.len() + 3);
    for (key, field) in &event.fields {
        let rendered = field_to_string(field);
        let (value, truncated) = truncate_field(&rendered, limits.max_field_chars);
        if truncated {
            result.truncated_fields += 1;
        }
        attributes.push((key.clone(), value));
    }
    attributes.push((ATTR_PROBE.to_string(), event.probe.clone()));
    // Disclose the clock domain on every span. See the module docs: the
    // canonical unix field is 0 because chronos does not observe the wall
    // clock during capture, and the monotonic value must not be lost.
    attributes.push((
        ATTR_TIMESTAMP_DOMAIN.to_string(),
        TIMESTAMP_DOMAIN.to_string(),
    ));
    attributes.push((
        ATTR_TIMESTAMP_MONOTONIC_NS.to_string(),
        event.ts_micros.saturating_mul(1000).to_string(),
    ));

    ExportedSpan {
        trace_id_hex,
        span_id_hex: hex_lower(span_id.bytes()),
        name: format!("{SPAN_NAME_PREFIX}{}", event.probe),
        start_time_unix_nano: 0,
        chronos_invocation_id: recorded.invocation_id.to_string(),
        attributes,
    }
}

/// FNV-1a/64, carried forward from one chunk of bytes to the next.
fn fnv1a64(seed: u64, bytes: &[u8]) -> u64 {
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = seed;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Derive a span id that is unique across the services sharing a trace.
///
/// ADR-0018 §2.3 derived it from `event_idx` alone, for reproducibility
/// across re-runs of the same stream. That reason does not survive two
/// services in one trace: `run_service_pipeline` builds a fresh
/// `CorrelationStore` per call, so `event_idx` restarts at 0 in every
/// service, and two services that share a traceparent both emitted
/// `span_id 0000000000000000` for their first event. Same OTel identity
/// for two different spans — a collector absorbs the second as a
/// duplicate and one span of the trace is lost.
///
/// Reproducibility and cross-service uniqueness cannot both come from a
/// counter, so the trade is recorded rather than implied: uniqueness
/// won, and re-exporting an event under a different trace id or
/// invocation now yields a different span id. Re-exporting the *same*
/// event under the same trace and invocation still reproduces it, which
/// is the reproducibility the ADR was protecting.
///
/// FNV-1a/64 rather than a cryptographic hash: this is trace identity,
/// not a trust boundary, and `chronos-domain` carries no hash dependency
/// today. A 64-bit space turns the certain collision of a shared counter
/// into a birthday-probability one, which is the whole point.
fn derive_span_id(trace_id_hex: &str, invocation_id: &OtlpInvocationId, event_idx: u64) -> SpanId {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    let hash = fnv1a64(OFFSET_BASIS, trace_id_hex.as_bytes());
    let hash = fnv1a64(hash, invocation_id.as_uuid().as_bytes());
    let hash = fnv1a64(hash, &event_idx.to_be_bytes());
    SpanId::from_bytes(hash.to_be_bytes())
}

/// Attribute values are strings on the wire even when the chronos-side field
/// is typed (ADR-0018 §2.3): the type stays available chronos-side, the
/// transport carries text.
fn field_to_string(field: &EventField) -> String {
    match field {
        EventField::Str(value) => value.clone(),
        EventField::Int(value) => value.to_string(),
        EventField::Bool(value) => value.to_string(),
        EventField::DomainRef(name) => format!("ref:{name}"),
    }
}

/// Cap a rendered field value at `max_field_chars` bytes.
///
/// Two things the spike's version got wrong, both load-bearing here: it cut
/// the string at a byte index, which panics when that index lands inside a
/// multi-byte character, and it appended the disclosure suffix on top of a
/// payload already sized to the cap, so the emitted value could exceed the
/// cap it claims to enforce. Here the suffix is counted first, and the cut is
/// walked back to a character boundary.
///
/// When the cap is smaller than the suffix, the suffix is emitted alone: a
/// value that says it was truncated is worth more than a value silently
/// shorter than the cap with no marker. The over-cap is then disclosed rather
/// than hidden.
fn truncate_field(rendered: &str, max_field_chars: usize) -> (String, bool) {
    if rendered.len() <= max_field_chars {
        return (rendered.to_string(), false);
    }
    let suffix = format!(
        "\u{2026}(truncated, {len}\u{2192}{max_field_chars})",
        len = rendered.len()
    );
    let budget = max_field_chars.saturating_sub(suffix.len());
    let mut cut = budget.min(rendered.len());
    while cut > 0 && !rendered.is_char_boundary(cut) {
        cut -= 1;
    }
    let mut out = String::with_capacity(cut + suffix.len());
    out.push_str(&rendered[..cut]);
    out.push_str(&suffix);
    (out, true)
}

fn trace_id_to_hex(trace_id: &TraceId) -> String {
    hex_lower(trace_id.bytes())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn json_string(value: &str) -> String {
    format!("\"{}\"", json_escape(value))
}

/// Escape a string for a JSON string literal (ADR-0018 §2.4): the seven
/// short escapes plus `\uXXXX` for anything else below `0x20`. Without the
/// control-char arm a raw newline in an event field would split one record
/// into two and corrupt the whole payload.
fn json_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                use std::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}
