//! M7.2 lift: differential execution v2 — alignment by invocation/context.
//!
//! Lifted from `/home/rubentxu/m7-spikes/m7.2-invocation-alignment/` (464L).
//!
//! Answers the question: given two recorded sessions, which invocations
//! in A correspond to which invocations in B? Alignment is by **identity
//! key**, not by timestamp (which drifts across hosts):
//!
//! 1. **Primary key**: `chronos_invocation_id` (UUID v4 stable per
//!    `RecordedInvocation`).
//! 2. **Fallback key**: `trace_id` from the W3C `traceparent` (useful
//!    when one side lost the chronos envelope but kept the upstream
//!    trace context).
//!
//! Once aligned, M7.2 delegates to M7.1's `hash_invocation_canonical`
//! (in `super::equivalence`) to decide whether matched invocations carry
//! the same behaviour under a given `EquivalenceSpec`. M7.2 does NOT
//! re-implement hashing — it only organises invocations by identity.
//!
//! ## Duplication avoidance
//!
//! `EquivalenceSpec` and `hash_invocation_canonical` come from
//! `chronos_domain::otlp::equivalence` (M7.1 lift). `RecordedInvocation`
//! and `ChronosEvent` come from `chronos_domain::otlp::correlation`
//! (M6.3 lift) and `chronos_domain::otlp` (M6.1 lift).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use super::correlation::ChronosEvent;
use super::RecordedInvocation;

// Re-export so consumers can build a session + spec from one module path
// (`chronos_domain::otlp::alignment::*`).
pub use super::equivalence::{
    equivalence_spec_default, hash_invocation_canonical, EquivalenceSpec,
};

/// A `RecordedInvocation` paired with its events.
///
/// `RecordedInvocation` carries identity (`invocation_id`, optional
/// `external: Option<ExternalTraceContext>`) but not the events themselves
/// — those live in `CorrelationStore` indexed by `event_idx`. To align
/// two sessions we need events alongside identity so we can hash the
/// invocation's behaviour. This struct is the unit of alignment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationWithEvents<'a> {
    /// Reference to the recorded invocation (identity).
    pub invocation: &'a RecordedInvocation,
    /// Events belonging to this invocation.
    pub events: Vec<ChronosEvent>,
}

impl<'a> InvocationWithEvents<'a> {
    /// Build a new (invocation, events) pair.
    pub fn new(invocation: &'a RecordedInvocation, events: Vec<ChronosEvent>) -> Self {
        Self { invocation, events }
    }
}

/// A session is an ordered list of invocations. Recording order is the
/// order in `invocations`; alignment itself is order-**in**dependent
/// (we sort by key internally) so two sessions recorded in different
/// order align correctly.
#[derive(Debug, Clone)]
pub struct Session<'a> {
    /// Invocation/event pairs in recording order.
    pub invocations: Vec<InvocationWithEvents<'a>>,
}

impl<'a> Session<'a> {
    /// Build an empty session.
    pub fn new() -> Self {
        Self {
            invocations: Vec::new(),
        }
    }

    /// Append an invocation.
    pub fn push(&mut self, iwe: InvocationWithEvents<'a>) {
        self.invocations.push(iwe);
    }

    /// Number of invocations.
    pub fn len(&self) -> usize {
        self.invocations.len()
    }

    /// `true` iff the session has no invocations.
    pub fn is_empty(&self) -> bool {
        self.invocations.is_empty()
    }
}

impl<'a> Default for Session<'a> {
    fn default() -> Self {
        Self::new()
    }
}

/// The identity key used to align two invocations.
///
/// - `Invocation(uuid)` is the primary key (always present for any
///   `RecordedInvocation`).
/// - `Trace(Vec<u8>)` is the fallback when one side lacks an
///   invocation_id or when only the W3C trace context survived.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum AlignmentKey {
    /// Primary key: `OtlpInvocationId` UUID v4.
    Invocation(uuid::Uuid),
    /// Fallback key: W3C trace_id raw bytes.
    Trace(Vec<u8>),
}

impl std::fmt::Display for AlignmentKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlignmentKey::Invocation(u) => write!(f, "inv:{}", u.hyphenated()),
            AlignmentKey::Trace(bytes) => {
                let mut s = String::with_capacity(2 + bytes.len() * 2);
                s.push_str("tr:");
                for b in bytes {
                    s.push_str(&format!("{:02x}", b));
                }
                write!(f, "{}", s)
            }
        }
    }
}

/// Per-invocation alignment verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvocationStatus {
    /// Same key in A and B; same invocation-level hash (per spec).
    Matched,
    /// Same key in A and B; different invocation-level hash (per spec).
    Mismatched,
    /// Key only in A.
    OnlyInA,
    /// Key only in B.
    OnlyInB,
}

/// A single aligned (or unaligned) invocation pair.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvocationAlignment {
    /// Identity key used to align.
    pub key: AlignmentKey,
    /// Verdict.
    pub status: InvocationStatus,
    /// `hash_invocation_canonical(events_a, spec)`. `None` if `OnlyInB`.
    pub hash_a: Option<u64>,
    /// `hash_invocation_canonical(events_b, spec)`. `None` if `OnlyInA`.
    pub hash_b: Option<u64>,
    /// `events_b.len() - events_a.len()`. `None` for `OnlyIn*`.
    pub delta_event_count: Option<i64>,
}

/// Which identity key was used to describe each side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlignmentKeyKind {
    /// Both sides aligned by `chronos_invocation_id` only.
    Invocation,
    /// Both sides aligned by `trace_id` only (no usable invocation_id on either side).
    Trace,
    /// Mixed: at least one side fell back to trace_id for some/all matches.
    Mixed,
}

impl std::fmt::Display for AlignmentKeyKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlignmentKeyKind::Invocation => write!(f, "invocation"),
            AlignmentKeyKind::Trace => write!(f, "trace"),
            AlignmentKeyKind::Mixed => write!(f, "mixed"),
        }
    }
}

/// Aggregated result of aligning two sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlignmentReport {
    /// Invocations matched by key and hash.
    pub matched: Vec<InvocationAlignment>,
    /// Invocations matched by key but differing in hash.
    pub mismatched: Vec<InvocationAlignment>,
    /// Keys present only in A.
    pub only_in_a: Vec<InvocationAlignment>,
    /// Keys present only in B.
    pub only_in_b: Vec<InvocationAlignment>,
    /// Key-kind used by A.
    pub alignment_key_a: AlignmentKeyKind,
    /// Key-kind used by B.
    pub alignment_key_b: AlignmentKeyKind,
}

impl AlignmentReport {
    /// Number of matched invocations.
    pub fn matched_count(&self) -> usize {
        self.matched.len()
    }
    /// Number of mismatched invocations.
    pub fn mismatched_count(&self) -> usize {
        self.mismatched.len()
    }
    /// Number of invocations only in A.
    pub fn only_in_a_count(&self) -> usize {
        self.only_in_a.len()
    }
    /// Number of invocations only in B.
    pub fn only_in_b_count(&self) -> usize {
        self.only_in_b.len()
    }

    /// Total invocations accounted for across all categories.
    pub fn total_pairs(&self) -> usize {
        self.matched_count()
            + self.mismatched_count()
            + self.only_in_a_count()
            + self.only_in_b_count()
    }

    /// `true` iff every invocation matches and there are no only-in-* extras.
    pub fn is_clean_equivalent(&self) -> bool {
        self.mismatched.is_empty()
            && self.only_in_a.is_empty()
            && self.only_in_b.is_empty()
            && !self.matched.is_empty()
    }

    /// `true` iff at least one Mismatched or only-in-* entry exists.
    pub fn has_drift(&self) -> bool {
        !self.mismatched.is_empty() || !self.only_in_a.is_empty() || !self.only_in_b.is_empty()
    }

    /// Single-line summary, useful for log output.
    pub fn summary_line(&self) -> String {
        format!(
            "alignment: matched={} mismatched={} only_in_a={} only_in_b={} key_kind={}",
            self.matched_count(),
            self.mismatched_count(),
            self.only_in_a_count(),
            self.only_in_b_count(),
            self.alignment_key_a,
        )
    }
}

/// Errors raised before alignment begins.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlignmentError {
    /// Two invocations in the same session share the same
    /// `chronos_invocation_id`. The alignment cannot pick a unique
    /// representative.
    AmbiguousInvocationId {
        /// The duplicated id.
        invocation_id: uuid::Uuid,
        /// Number of times it appeared.
        occurrences: usize,
    },
}

impl std::fmt::Display for AlignmentError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AlignmentError::AmbiguousInvocationId {
                invocation_id,
                occurrences,
            } => write!(
                f,
                "ambiguous invocation_id {} appears {} times in one session",
                invocation_id.hyphenated(),
                occurrences
            ),
        }
    }
}

impl std::error::Error for AlignmentError {}

/// Compute an [`AlignmentReport`] for two sessions under a spec.
///
/// Steps:
/// 1. Detect (and reject) duplicate invocation_ids inside A or B.
/// 2. Index A and B by `chronos_invocation_id`.
/// 3. For each remaining key in B that didn't match by invocation_id,
///    try `trace_id` fallback (only when the side has
///    `external: Some(...)`).
/// 4. Anything left is `only_in_*`.
/// 5. For matched pairs, hash both sides with
///    `hash_invocation_canonical` and decide `Matched` vs `Mismatched`.
/// 6. Compute `delta_event_count` for matched pairs.
pub fn align_sessions<'a>(
    a: Session<'a>,
    b: Session<'a>,
    spec: &EquivalenceSpec,
) -> Result<AlignmentReport, AlignmentError> {
    check_no_duplicate_invocation_ids(&a)?;
    check_no_duplicate_invocation_ids(&b)?;

    let a_by_inv = index_by_invocation(&a);
    let a_by_trace = index_by_trace(&a);
    let b_by_inv = index_by_invocation(&b);
    let b_by_trace = index_by_trace(&b);

    let mut matched: Vec<InvocationAlignment> = Vec::new();
    let mut mismatched: Vec<InvocationAlignment> = Vec::new();
    let mut only_in_a: Vec<InvocationAlignment> = Vec::new();
    let mut only_in_b: Vec<InvocationAlignment> = Vec::new();
    let mut b_consumed: BTreeSet<AlignmentKey> = BTreeSet::new();

    for (key, iwe) in a_by_inv.iter() {
        // Prefer match by invocation_id.
        if let Some(b_iwe) = b_by_inv.get(key) {
            let pair = compute_pair(AlignmentKey::Invocation(*key), Some(iwe), Some(b_iwe), spec);
            push_pair(pair, &mut matched, &mut mismatched);
            b_consumed.insert(AlignmentKey::Invocation(*key));
            continue;
        }
        // Fallback to trace_id, if A has one.
        if let Some(trace_bytes) = trace_id_bytes(iwe.invocation) {
            if let Some((b_key, b_iwe)) = b_by_trace.get_key_value(&trace_bytes) {
                if !b_consumed.contains(&AlignmentKey::Trace(b_key.clone())) {
                    let pair = compute_pair(
                        AlignmentKey::Trace(trace_bytes),
                        Some(iwe),
                        Some(b_iwe),
                        spec,
                    );
                    push_pair(pair, &mut matched, &mut mismatched);
                    b_consumed.insert(AlignmentKey::Trace(b_key.clone()));
                    continue;
                }
            }
        }
        // No match — only in A.
        let pair = compute_pair(AlignmentKey::Invocation(*key), Some(iwe), None, spec);
        only_in_a.push(pair);
    }

    // Anything in B not consumed is only_in_b.
    for (key, iwe) in b_by_inv.iter() {
        if b_consumed.contains(&AlignmentKey::Invocation(*key)) {
            continue;
        }
        // Try trace fallback.
        if let Some(trace_bytes) = trace_id_bytes(iwe.invocation) {
            if let Some((b_key, _b_iwe)) = b_by_trace.get_key_value(&trace_bytes) {
                if b_consumed.contains(&AlignmentKey::Trace(b_key.clone())) {
                    continue;
                }
            }
        }
        let pair = compute_pair(AlignmentKey::Invocation(*key), None, Some(iwe), spec);
        only_in_b.push(pair);
    }

    // Decide AlignmentKeyKind for each side.
    let a_has_trace = !a_by_trace.is_empty();
    let b_has_trace = !b_by_trace.is_empty();
    let a_has_inv = !a_by_inv.is_empty();
    let b_has_inv = !b_by_inv.is_empty();
    let alignment_key_a = classify_kind(a_has_trace, a_has_inv);
    let alignment_key_b = classify_kind(b_has_trace, b_has_inv);

    Ok(AlignmentReport {
        matched,
        mismatched,
        only_in_a,
        only_in_b,
        alignment_key_a,
        alignment_key_b,
    })
}

fn classify_kind(has_trace: bool, has_inv: bool) -> AlignmentKeyKind {
    if has_trace && has_inv {
        AlignmentKeyKind::Mixed
    } else if has_trace {
        AlignmentKeyKind::Trace
    } else {
        AlignmentKeyKind::Invocation
    }
}

fn check_no_duplicate_invocation_ids(s: &Session<'_>) -> Result<(), AlignmentError> {
    let mut seen: HashMap<uuid::Uuid, usize> = HashMap::new();
    for iwe in &s.invocations {
        let id = iwe.invocation.invocation_id.as_uuid();
        *seen.entry(id).or_insert(0) += 1;
    }
    for (id, count) in seen {
        if count > 1 {
            return Err(AlignmentError::AmbiguousInvocationId {
                invocation_id: id,
                occurrences: count,
            });
        }
    }
    Ok(())
}

fn index_by_invocation<'a>(
    s: &'a Session<'a>,
) -> BTreeMap<uuid::Uuid, &'a InvocationWithEvents<'a>> {
    let mut map: BTreeMap<uuid::Uuid, &InvocationWithEvents<'a>> = BTreeMap::new();
    for iwe in &s.invocations {
        map.insert(iwe.invocation.invocation_id.as_uuid(), iwe);
    }
    map
}

fn index_by_trace<'a>(s: &'a Session<'a>) -> BTreeMap<Vec<u8>, &'a InvocationWithEvents<'a>> {
    let mut map: BTreeMap<Vec<u8>, &InvocationWithEvents<'a>> = BTreeMap::new();
    for iwe in &s.invocations {
        if let Some(bytes) = trace_id_bytes(iwe.invocation) {
            map.entry(bytes).or_insert(iwe);
        }
    }
    map
}

fn trace_id_bytes(rec: &RecordedInvocation) -> Option<Vec<u8>> {
    rec.external.as_ref().map(|e| e.trace_id().bytes().to_vec())
}

fn compute_pair<'a>(
    key: AlignmentKey,
    a: Option<&InvocationWithEvents<'a>>,
    b: Option<&InvocationWithEvents<'a>>,
    spec: &EquivalenceSpec,
) -> InvocationAlignment {
    let hash_a = a.map(|iwe| hash_invocation_canonical(&iwe.events, spec));
    let hash_b = b.map(|iwe| hash_invocation_canonical(&iwe.events, spec));

    let status = match (a, b) {
        (Some(_), Some(_)) => {
            if hash_a == hash_b {
                InvocationStatus::Matched
            } else {
                InvocationStatus::Mismatched
            }
        }
        (Some(_), None) => InvocationStatus::OnlyInA,
        (None, Some(_)) => InvocationStatus::OnlyInB,
        (None, None) => InvocationStatus::OnlyInA, // degenerate
    };

    let delta_event_count = match (a, b) {
        (Some(ia), Some(ib)) => Some(ib.events.len() as i64 - ia.events.len() as i64),
        _ => None,
    };

    InvocationAlignment {
        key,
        status,
        hash_a,
        hash_b,
        delta_event_count,
    }
}

fn push_pair(
    pair: InvocationAlignment,
    matched: &mut Vec<InvocationAlignment>,
    mismatched: &mut Vec<InvocationAlignment>,
) {
    match pair.status {
        InvocationStatus::Matched => matched.push(pair),
        InvocationStatus::Mismatched => mismatched.push(pair),
        // These two should never reach push_pair; surface as a hard push.
        InvocationStatus::OnlyInA => matched.push(pair),
        InvocationStatus::OnlyInB => matched.push(pair),
    }
}
