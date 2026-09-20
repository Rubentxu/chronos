//! Tripwire System — condition-based event notification.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::{EventData, EventType, TraceEvent};

// Tripwire firing semantics live entirely in this crate. Transport
// concerns (HTTP delivery, retries, timeouts) belong to driven
// adapters (e.g. `chronos-webhook`) and are reached via the
// `chronos_domain::ports::NotificationSink` port.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TripwireId(pub u64);
impl std::fmt::Display for TripwireId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tripwire-{}", self.0)
    }
}

/// REC-C2.1: a matched subscription, snapshotted for durable evidence.
///
/// Carries what is needed to persist a self-describing firing (identity of
/// the subscription that fired plus the condition and label in force at that
/// moment) without touching the legacy fired buffer.
#[derive(Debug, Clone, PartialEq)]
pub struct TripwireMatch {
    pub id: TripwireId,
    pub condition: TripwireCondition,
    pub label: Option<String>,
}

static NEXT_TRIPWIRE_ID: AtomicU64 = AtomicU64::new(1);
fn next_tripwire_id() -> TripwireId {
    TripwireId(NEXT_TRIPWIRE_ID.fetch_add(1, Ordering::Relaxed))
}

/// Reset the global tripwire ID counter.
///
/// **Test-only API.** Call this at the start of each test that asserts on
/// specific ID strings (e.g. `"tripwire-1"`). Without a reset, IDs accumulate
/// across the workspace test suite and assertions on fixed strings fail.
///
/// Compiled unconditionally so downstream test binaries can reach it even when
/// `chronos-domain` is compiled as a plain lib (not `--tests`).
pub fn reset_tripwire_ids_for_testing() {
    NEXT_TRIPWIRE_ID.store(1, Ordering::Relaxed);
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TripwireCondition {
    EventType(Vec<EventType>),
    FunctionName { pattern: String },
    ExceptionType { exc_type: String },
    MemoryAddress { start: u64, end: u64 },
    SyscallNumber { numbers: Vec<u64> },
    VariableName { name: String },
    Signal { numbers: Vec<i32> },
}

impl TripwireCondition {
    pub fn matches(&self, event: &TraceEvent) -> bool {
        match self {
            TripwireCondition::EventType(types) => types.contains(&event.event_type),
            TripwireCondition::FunctionName { pattern } => event
                .location
                .function
                .as_ref()
                .is_some_and(|n| glob_match(pattern, n)),
            TripwireCondition::ExceptionType { exc_type } => {
                matches!(&event.data, EventData::Exception { type_name, .. } if type_name.contains(exc_type))
            }
            TripwireCondition::MemoryAddress { start, end } => {
                event.location.address >= *start && event.location.address <= *end
            }
            TripwireCondition::SyscallNumber { numbers } => {
                matches!(&event.data, EventData::Syscall { number, .. } if numbers.contains(&{ *number }))
            }
            TripwireCondition::VariableName { name } => {
                matches!(&event.data, EventData::Variable(info) if info.name == *name)
            }
            TripwireCondition::Signal { numbers } => {
                matches!(&event.data, EventData::Signal { signal_number, .. } if numbers.contains(signal_number))
            }
        }
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let t: Vec<char> = text.chars().collect();
    glob_inner(&p, &t, 0, 0)
}

fn glob_inner(p: &[char], t: &[char], pi: usize, ti: usize) -> bool {
    if pi == p.len() {
        return ti == t.len();
    }
    if p[pi] == '*' {
        for skip in ti..=t.len() {
            if glob_inner(p, t, pi + 1, skip) {
                return true;
            }
        }
        return false;
    }
    if ti >= t.len() {
        return false;
    }
    if p[pi] == '?' || p[pi] == t[ti] {
        return glob_inner(p, t, pi + 1, ti + 1);
    }
    false
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Tripwire {
    pub id: TripwireId,
    pub condition: TripwireCondition,
    pub label: Option<String>,
    pub fire_count: u64,
}

impl Tripwire {
    /// Construct a tripwire with the global-counter id. Prefer
    /// [`Tripwire::with_id`](Self::with_id) when the caller already has an
    /// id allocated by a [`TripwireManager`] — that path is collision-free
    /// even when tests race the global counter.
    pub fn new(condition: TripwireCondition) -> Self {
        Self {
            id: next_tripwire_id(),
            condition,
            label: None,
            fire_count: 0,
        }
    }

    /// Construct a tripwire with a caller-provided id (typically allocated
    /// by [`TripwireManager::next_id`](crate::tripwire::TripwireManager)).
    pub fn with_id(id: TripwireId, condition: TripwireCondition) -> Self {
        Self {
            id,
            condition,
            label: None,
            fire_count: 0,
        }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn matches(&self, event: &TraceEvent) -> bool {
        self.condition.matches(event)
    }
}

/// Extract a function-name candidate from a [`SemanticEvent`](crate::SemanticEvent).
fn function_from_semantic(event: &crate::SemanticEvent) -> Option<String> {
    use crate::SemanticEventKind;
    match &event.kind {
        SemanticEventKind::FunctionCalled { function, .. }
        | SemanticEventKind::FunctionReturned { function, .. } => Some(function.clone()),
        // Syscalls (e.g., "open", "read") also expose a name; treat the name
        // as a function-name candidate so `TripwireCondition::FunctionName`
        // can match against syscall traffic.
        SemanticEventKind::Syscall { name, .. } => Some(name.clone()),
        _ => None,
    }
}

#[derive(Debug)]
pub struct TripwireManager {
    tripwires: std::sync::RwLock<Vec<Tripwire>>,
    /// Per-instance monotonic counter for tripwire IDs. Using a per-instance
    /// counter (rather than the process-global `NEXT_TRIPWIRE_ID`) avoids
    /// collisions when multiple test cases call `reset_tripwire_ids_for_testing()`
    /// concurrently and then call `register` — with a shared counter, two
    /// parallel registers could observe the same id, and `remove(id)` would
    /// then delete both tripwires, breaking `delete_one_of_two`-style tests.
    ///
    /// The first id allocated by any manager is `manager_id_seed + 1`, where
    /// `manager_id_seed` is fetched from the global atomic on `new()`. This
    /// keeps IDs globally unique while preventing within-manager collisions.
    next_id: std::sync::atomic::AtomicU64,
}

impl Default for TripwireManager {
    fn default() -> Self {
        Self::new()
    }
}

impl TripwireManager {
    pub fn new() -> Self {
        // Reserve the next id from the global counter as the *base* for this
        // manager's per-instance sequence. The per-instance counter then
        // increments monotonically, so consecutive `register` calls in this
        // manager are guaranteed to be unique within it — even when other
        // tests concurrently call `reset_tripwire_ids_for_testing()`.
        //
        // Cross-manager collisions (different managers observing the same
        // `TripwireId`) are harmless: each manager's `remove` is bounded to
        // its own `tripwires` Vec, so an id reused by another manager does
        // not affect a sibling manager's bookkeeping. The only invariant we
        // need is *intra-manager* uniqueness, which the per-instance counter
        // guarantees unconditionally.
        let base = NEXT_TRIPWIRE_ID.fetch_add(1, Ordering::Relaxed);
        Self {
            tripwires: std::sync::RwLock::new(Vec::new()),
            // First issued id is `base` (the id we just consumed). After
            // `reset_tripwire_ids_for_testing()`, `base` will be 1 — matching
            // the convention used by legacy tests that assert on
            // `"tripwire-1"`.
            next_id: std::sync::atomic::AtomicU64::new(base),
        }
    }

    fn next_id(&self) -> TripwireId {
        TripwireId(self.next_id.fetch_add(1, Ordering::Relaxed))
    }

    pub fn register(&self, condition: TripwireCondition) -> TripwireId {
        self.register_with_label(condition, None)
    }

    /// Register a tripwire with an optional label.
    ///
    /// The label is stored in the [`Tripwire`] so it appears in list/query results.
    pub fn register_with_label(
        &self,
        condition: TripwireCondition,
        label: Option<String>,
    ) -> TripwireId {
        let id = self.next_id();
        let mut tw = Tripwire::with_id(id, condition);
        tw.label = label;
        self.tripwires.write().unwrap().push(tw);
        id
    }

    pub fn remove(&self, id: TripwireId) -> bool {
        let mut tws = self.tripwires.write().unwrap();
        let before = tws.len();
        tws.retain(|tw| tw.id != id);
        tws.len() < before
    }

    pub fn list(&self) -> Vec<Tripwire> {
        self.tripwires.read().unwrap().clone()
    }

    /// REC-C2.1: the tripwires that match `event`, as a **pure** query (no
    /// buffer side effect).
    ///
    /// Derivation of durable `TripwireFired` evidence uses this rather than
    /// [`evaluate`](Self::evaluate), so that producing evidence does not
    /// simultaneously mutate the legacy in-memory fired buffer. Returns a
    /// snapshot of the subscription (id, condition, label) so the caller can
    /// persist a self-describing firing.
    pub fn matching(&self, event: &TraceEvent) -> Vec<TripwireMatch> {
        self.tripwires
            .read()
            .unwrap()
            .iter()
            .filter(|tw| tw.matches(event))
            .map(|tw| TripwireMatch {
                id: tw.id,
                condition: tw.condition.clone(),
                label: tw.label.clone(),
            })
            .collect()
    }

    /// REC-C2.1.6: the pure semantic matcher.
    ///
    /// Same matching rules as the retired `evaluate_semantic`, with **no**
    /// buffer side effect. `probe_drain` uses this to count live matches for
    /// its `tripwires_fired` signal; durable firing evidence is derived
    /// separately from the `ExecutionLog`.
    pub fn matching_semantic(&self, event: &crate::SemanticEvent) -> Vec<TripwireMatch> {
        self.tripwires
            .read()
            .unwrap()
            .iter()
            .filter_map(|tw| {
                if let TripwireCondition::FunctionName { pattern } = &tw.condition {
                    let candidate =
                        function_from_semantic(event).unwrap_or_else(|| event.description.clone());
                    if glob_match(pattern, &candidate) {
                        return Some(TripwireMatch {
                            id: tw.id,
                            condition: tw.condition.clone(),
                            label: tw.label.clone(),
                        });
                    }
                }
                None
            })
            .collect()
    }

    pub fn active_count(&self) -> usize {
        self.tripwires.read().unwrap().len()
    }
}

pub type TripwireManagerHandle = Arc<TripwireManager>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::MonotonicNs;

    /// CHAR-C2-04 (kept, re-expressed): `Tripwire.fire_count` is initialized to
    /// 0 and never mutated by any matching path. It is still present for
    /// compatibility but is not authoritative anywhere: the wire `fire_count`
    /// is derived from the ExecutionLog (REC-C2.1.5, FIND-C2.0-02).
    ///
    /// The old fired-buffer characterization (CHAR-C2-05) is gone with the
    /// buffer; its replacement lives where the semantics now do, in
    /// `chronos-services` FIRING-PAGE-2 (two consumers, one firing, no theft).
    #[test]
    fn char_c2_04_fire_count_is_always_zero() {
        let mgr = TripwireManager::new();
        mgr.register(TripwireCondition::Signal { numbers: vec![11] });

        for i in 0..3 {
            let matched = mgr.matching(&make_signal_event(i + 1, 11));
            assert_eq!(matched.len(), 1, "the tripwire matched");
        }

        let listed = mgr.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(
            listed[0].fire_count, 0,
            "char: the mutable counter is never incremented by any path"
        );
    }

    fn make_signal_event(id: u64, signal: i32) -> TraceEvent {
        TraceEvent::signal(id, MonotonicNs::from(id * 1000), 1, signal, "SIGTEST", 0)
    }

    #[test]
    fn test_glob_match() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("process_*", "process_payment"));
        assert!(!glob_match("process_*", "handle_request"));
    }

    #[test]
    fn test_condition_signal() {
        let cond = TripwireCondition::Signal { numbers: vec![11] };
        assert!(cond.matches(&make_signal_event(1, 11)));
        assert!(!cond.matches(&make_signal_event(2, 9)));
    }

    #[test]
    fn test_manager_register_and_match() {
        let mgr = TripwireManager::new();
        mgr.register(TripwireCondition::Signal { numbers: vec![11] });
        assert_eq!(mgr.matching(&make_signal_event(1, 11)).len(), 1);
        assert!(mgr.matching(&make_signal_event(2, 9)).is_empty());
    }

    #[test]
    fn test_matching_semantic_matches_description_fallback() {
        // Live probes emit wire-level SemanticEvents where the kind is
        // `Unresolved` and the function/syscall name lives in `description`.
        // matching_semantic must match against that fallback path so the live
        // `tripwires_fired` signal is accurate.
        let mgr = TripwireManager::new();
        let registered_id = mgr.register(TripwireCondition::FunctionName {
            pattern: "SyscallEnter".to_string(),
        });
        let event = crate::SemanticEvent {
            source_event_id: 42,
            timestamp_ns: 0,
            thread_id: 1,
            language: crate::Language::Unknown,
            kind: crate::SemanticEventKind::Unresolved,
            description: "SyscallEnter".to_string(),
        };
        let matched = mgr.matching_semantic(&event);
        assert_eq!(matched.len(), 1, "SyscallEnter tripwire must match");
        // Assert the registered tripwire fired, not that its id equals 1.
        // NEXT_TRIPWIRE_ID is a process-global atomic (tripwire.rs:24), so
        // id values depend on test scheduling — the only stable invariant is
        // "the tripwire we registered is the one that fired".
        assert_eq!(
            matched[0].id, registered_id,
            "the matched tripwire must be the one we just registered"
        );
    }

    #[test]
    fn test_matching_semantic_uses_function_field_when_present() {
        // When the SemanticEvent already has a function field (typed
        // FunctionCalled), the tripwire must match against it directly.
        let mgr = TripwireManager::new();
        mgr.register(TripwireCondition::FunctionName {
            pattern: "do_work".to_string(),
        });
        let event = crate::SemanticEvent {
            source_event_id: 1,
            timestamp_ns: 0,
            thread_id: 1,
            language: crate::Language::C,
            kind: crate::SemanticEventKind::FunctionCalled {
                function: "do_work".to_string(),
                module: None,
                arguments: vec![],
            },
            description: "ignored".to_string(),
        };
        let matched = mgr.matching_semantic(&event);
        assert_eq!(matched.len(), 1);
    }
}
