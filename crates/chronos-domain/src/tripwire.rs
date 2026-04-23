//! Tripwire System — condition-based event notification.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use crate::{EventData, EventType, TraceEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct TripwireId(pub u64);

impl std::fmt::Display for TripwireId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "tripwire-{}", self.0)
    }
}

static NEXT_TRIPWIRE_ID: AtomicU64 = AtomicU64::new(1);
fn next_tripwire_id() -> TripwireId { TripwireId(NEXT_TRIPWIRE_ID.fetch_add(1, Ordering::Relaxed)) }

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
            TripwireCondition::FunctionName { pattern } => {
                event.location.function.as_ref().map_or(false, |n| glob_match(pattern, n))
            }
            TripwireCondition::ExceptionType { exc_type } => {
                matches!(&event.data, EventData::Exception { type_name, .. } if type_name.contains(exc_type))
            }
            TripwireCondition::MemoryAddress { start, end } => event.location.address >= *start && event.location.address <= *end,
            TripwireCondition::SyscallNumber { numbers } => {
                matches!(&event.data, EventData::Syscall { number, .. } if numbers.contains(&(*number as u64)))
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
    if pi == p.len() { return ti == t.len(); }
    if p[pi] == '*' {
        for skip in ti..=t.len() {
            if glob_inner(p, t, pi + 1, skip) { return true; }
        }
        return false;
    }
    if ti >= t.len() { return false; }
    if p[pi] == '?' || p[pi] == t[ti] { return glob_inner(p, t, pi + 1, ti + 1); }
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
        Self { id: next_tripwire_id(), condition, label: None, fire_count: 0 }
    }

    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn matches(&self, event: &TraceEvent) -> bool { self.condition.matches(event) }

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

#[derive(Debug, Default)]
pub struct TripwireManager {
    tripwires: std::sync::RwLock<Vec<Tripwire>>,
    fired_buffer: std::sync::RwLock<Vec<TripwireFired>>,
}

impl TripwireManager {
    pub fn new() -> Self { Self::default() }

    pub fn register(&self, condition: TripwireCondition) -> TripwireId {
        let tw = Tripwire::new(condition);
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

    pub fn list(&self) -> Vec<Tripwire> { self.tripwires.read().unwrap().clone() }

    pub fn evaluate(&self, event: &TraceEvent) -> Vec<TripwireFired> {
        let tws = self.tripwires.read().unwrap();
        let fired: Vec<_> = tws.iter().filter(|tw| tw.matches(event)).map(|tw| tw.fire(event)).collect();
        drop(tws);
        if !fired.is_empty() {
            let mut buf = self.fired_buffer.write().unwrap();
            let new_count = fired.len();
            let buf_len = buf.len();
            if buf_len + new_count > 1000 {
                let drain_count = buf_len + new_count - 1000;
                buf.drain(..drain_count);
            }
            buf.extend(fired.clone());
        }
        fired
    }

    pub fn drain_fired(&self) -> Vec<TripwireFired> {
        std::mem::take(&mut *self.fired_buffer.write().unwrap())
    }

    pub fn active_count(&self) -> usize { self.tripwires.read().unwrap().len() }
}

pub type TripwireManagerHandle = Arc<TripwireManager>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EventData, SourceLocation};

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
}
