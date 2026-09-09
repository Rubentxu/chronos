//! Tripwire System — condition-based event notification.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use thiserror::Error;

use crate::{EventData, EventType, TraceEvent};

// Webhook delivery module
#[cfg(feature = "webhook")]
pub mod webhook;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TripwireId(pub u64);

impl std::fmt::Display for TripwireId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tripwire-{}", self.0)
    }
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

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
pub struct TripwireFired {
    pub tripwire_id: TripwireId,
    pub condition_description: String,
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Tripwire {
    pub id: TripwireId,
    pub condition: TripwireCondition,
    pub label: Option<String>,
    pub fire_count: u64,
}

impl Tripwire {
    pub fn new(condition: TripwireCondition) -> Self {
        Self {
            id: next_tripwire_id(),
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

    pub fn fire(&self, event: &TraceEvent) -> TripwireFired {
        TripwireFired {
            tripwire_id: self.id,
            condition_description: format!("{:?}", self.condition),
            event_id: event.event_id,
            timestamp_ns: event.timestamp_ns,
            thread_id: event.thread_id,
        }
    }

    /// Fire this tripwire against a semantic event. The tripwire id,
    /// condition description, timestamp, and thread_id are populated;
    /// `event_id` is `0` because [`SemanticEvent`](crate::SemanticEvent)
    /// does not carry the source event id.
    pub fn fire_semantic(&self, event: &crate::SemanticEvent) -> TripwireFired {
        TripwireFired {
            tripwire_id: self.id,
            condition_description: format!("{:?}", self.condition),
            event_id: 0,
            timestamp_ns: event.timestamp_ns,
            thread_id: event.thread_id,
        }
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

/// A tripwire subscription with optional webhook callback.
///
/// Represents a registered tripwire with its delivery configuration.
#[derive(Debug, Clone)]
pub struct TripwireSubscription {
    /// Unique identifier for this tripwire.
    pub id: TripwireId,
    /// The condition that triggers this tripwire.
    pub condition: TripwireCondition,
    /// Optional label for human readability.
    pub label: Option<String>,
    /// Optional callback URL for webhook delivery.
    /// If `None`, the tripwire operates in polling mode.
    pub callback_url: Option<url::Url>,
    /// When this subscription was created.
    pub created_at: std::time::SystemTime,
    /// Number of times this tripwire has fired.
    pub fire_count: u64,
}

impl TripwireSubscription {
    /// Create a new subscription.
    pub fn new(condition: TripwireCondition, callback_url: Option<url::Url>) -> Self {
        Self {
            id: next_tripwire_id(),
            condition,
            label: None,
            callback_url,
            created_at: std::time::SystemTime::now(),
            fire_count: 0,
        }
    }

    /// Create a new subscription with a label.
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Check if this subscription has a webhook callback.
    pub fn has_webhook(&self) -> bool {
        self.callback_url.is_some()
    }

    /// Get the callback URL if set.
    pub fn callback_url(&self) -> Option<&url::Url> {
        self.callback_url.as_ref()
    }

    /// Check if a trace event matches this subscription's condition.
    pub fn matches(&self, event: &TraceEvent) -> bool {
        self.condition.matches(event)
    }

    /// Create a `TripwireFired` event for this subscription.
    pub fn fire(&self, event: &TraceEvent) -> TripwireFired {
        TripwireFired {
            tripwire_id: self.id,
            condition_description: format!("{:?}", self.condition),
            event_id: event.event_id,
            timestamp_ns: event.timestamp_ns,
            thread_id: event.thread_id,
        }
    }
}

/// Errors that can occur during tripwire operations.
#[derive(Debug, Error)]
pub enum TripwireError {
    #[error("Invalid callback URL: must use https://")]
    InvalidCallbackUrl,

    #[error("Port numbers not allowed in callback URL")]
    UrlPortNotAllowed,

    #[error("Webhook delivery failed after 3 attempts")]
    DeliveryFailed,

    #[error("URL parse error: {0}")]
    UrlParseError(String),
}

/// Validate a callback URL for webhook delivery.
///
/// Returns `Ok(Url)` if valid, `Err(TripwireError)` otherwise.
///
/// Validation rules:
/// - Must use HTTPS scheme
/// - Must not have a port number
pub fn validate_callback_url(url_str: &str) -> Result<url::Url, TripwireError> {
    let parsed =
        url::Url::parse(url_str).map_err(|e| TripwireError::UrlParseError(e.to_string()))?;

    if parsed.scheme() != "https" {
        return Err(TripwireError::InvalidCallbackUrl);
    }

    if parsed.port().is_some() {
        return Err(TripwireError::UrlPortNotAllowed);
    }

    Ok(parsed)
}

#[derive(Debug, Default)]
pub struct TripwireManager {
    tripwires: std::sync::RwLock<Vec<Tripwire>>,
    fired_buffer: std::sync::RwLock<Vec<TripwireFired>>,
}

impl TripwireManager {
    pub fn new() -> Self {
        Self::default()
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
        let mut tw = Tripwire::new(condition);
        tw.label = label;
        let id = tw.id;
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

    pub fn evaluate(&self, event: &TraceEvent) -> Vec<TripwireFired> {
        let tws = self.tripwires.read().unwrap();
        let fired: Vec<_> = tws
            .iter()
            .filter(|tw| tw.matches(event))
            .map(|tw| tw.fire(event))
            .collect();
        drop(tws);
        self.record_fired(&fired);
        fired
    }

    /// Evaluate all tripwires against a semantic event.
    ///
    /// This is a subset of [`evaluate`](Self::evaluate): only conditions
    /// that can be matched purely from a [`SemanticEvent`](crate::SemanticEvent)
    /// (currently `FunctionName`, via the `description` and the function
    /// field of [`SemanticEventKind`](crate::SemanticEventKind)) are honoured.
    /// Other conditions silently do not match — semantic events do not
    /// carry the address/type fields those need.
    pub fn evaluate_semantic(&self, event: &crate::SemanticEvent) -> Vec<TripwireFired> {
        let tws = self.tripwires.read().unwrap();
        let fired: Vec<_> = tws
            .iter()
            .filter_map(|tw| {
                if let TripwireCondition::FunctionName { pattern } = &tw.condition {
                    // Try the canonical function-name field first; fall back
                    // to the description so wire-level events (where kind
                    // is `Unresolved` and the function name is encoded in
                    // the description, e.g. "SyscallEnter"/"FunctionCalled")
                    // still match.
                    let candidate =
                        function_from_semantic(event).unwrap_or_else(|| event.description.clone());
                    if glob_match(pattern, &candidate) {
                        return Some(tw.fire_semantic(event));
                    }
                }
                None
            })
            .collect();
        drop(tws);
        self.record_fired(&fired);
        fired
    }

    fn record_fired(&self, fired: &[TripwireFired]) {
        if fired.is_empty() {
            return;
        }
        let mut buf = self.fired_buffer.write().unwrap();
        let new_count = fired.len();
        let buf_len = buf.len();
        if buf_len + new_count > 1000 {
            let drain_count = buf_len + new_count - 1000;
            buf.drain(..drain_count);
        }
        buf.extend(fired.iter().cloned());
    }

    pub fn drain_fired(&self) -> Vec<TripwireFired> {
        std::mem::take(&mut *self.fired_buffer.write().unwrap())
    }

    pub fn active_count(&self) -> usize {
        self.tripwires.read().unwrap().len()
    }
}

pub type TripwireManagerHandle = Arc<TripwireManager>;

#[cfg(test)]
mod tests {
    use super::*;

    fn make_signal_event(id: u64, signal: i32) -> TraceEvent {
        TraceEvent::signal(id, id * 1000, 1, signal, "SIGTEST", 0)
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
    fn test_manager_register_and_fire() {
        let mgr = TripwireManager::new();
        mgr.register(TripwireCondition::Signal { numbers: vec![11] });
        assert_eq!(mgr.evaluate(&make_signal_event(1, 11)).len(), 1);
        assert!(mgr.evaluate(&make_signal_event(2, 9)).is_empty());
    }

    #[test]
    fn test_manager_drain_fired() {
        let mgr = TripwireManager::new();
        mgr.register(TripwireCondition::Signal { numbers: vec![11] });
        mgr.evaluate(&make_signal_event(1, 11));
        mgr.evaluate(&make_signal_event(2, 11));
        assert_eq!(mgr.drain_fired().len(), 2);
        assert!(mgr.drain_fired().is_empty());
    }

    #[test]
    fn test_evaluate_semantic_matches_description_fallback() {
        // Live probes emit wire-level SemanticEvents where the kind is
        // `Unresolved` and the function/syscall name lives in `description`.
        // evaluate_semantic must match against that fallback path so live
        // evidence flows into the tripwire subsystem.
        let mgr = TripwireManager::new();
        mgr.register(TripwireCondition::FunctionName {
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
        let fired = mgr.evaluate_semantic(&event);
        assert_eq!(fired.len(), 1, "SyscallEnter tripwire must fire");
        assert_eq!(fired[0].tripwire_id.0, 1);
    }

    #[test]
    fn test_evaluate_semantic_uses_function_field_when_present() {
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
        let fired = mgr.evaluate_semantic(&event);
        assert_eq!(fired.len(), 1);
    }
}
