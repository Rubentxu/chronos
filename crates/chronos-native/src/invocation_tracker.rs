//! Per-thread invocation tracker for the M2 capture pipeline.
//!
//! When `PtraceConfig::track_function_frames` is `true`, the capture
//! pipeline consults `InvocationTracker::on_sigtrap` on every stop.
//! The tracker maintains a per-thread logical call stack and produces
//! `TraceEvent`s pre-populated with `InvocationId`, `parent_invocation_id`,
//! and `SymbolId`. When the probe detects process termination (SIGKILL
//! or exit without a paired FunctionExit), `flush_incomplete_on_exit`
//! emits one `EventType::InvocationIncomplete` per still-active
//! invocation in LIFO order.
//!
//! The legacy flat `FunctionEntryTracker` (no per-thread state, no ids)
//! remains in `capture_runner.rs` for the M0/M1 default. This module is
//! the M2 replacement.

use crate::symbol_resolver::SymbolResolver;
use chronos_domain::trace::{ThreadId, TimestampNs};
use chronos_domain::{EventData, EventType, InvocationId, Language, SourceLocation, TraceEvent};
use std::collections::HashMap;

/// One active invocation on the per-thread call stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveInvocation {
    pub invocation_id: InvocationId,
    pub parent_invocation_id: Option<InvocationId>,
    pub symbol_id: chronos_domain::SymbolId,
    pub entry_monotonic_ns: u64,
    pub entry_ip: u64,
    pub function_name: String,
}

/// Per-thread call-stack state for function-frame identity.
pub struct InvocationTracker {
    /// Address → `(SymbolId, function_name)`. Populated from the
    /// `SymbolResolver` once at tracker construction.
    symbols_by_address: HashMap<u64, (chronos_domain::SymbolId, String)>,
    /// Per-thread call stack. Most recent invocation at the end.
    per_thread_stack: HashMap<ThreadId, Vec<ActiveInvocation>>,
}

impl InvocationTracker {
    /// Construct a tracker from a `SymbolResolver`. Returns `None`
    /// when no symbols are resolvable (in which case the caller falls
    /// back to the legacy `FunctionEntryTracker`).
    pub fn new(resolver: &SymbolResolver) -> Option<Self> {
        let mut symbols_by_address = HashMap::new();
        for sym in resolver.symbols().values() {
            if sym.size > 0 {
                let sid = chronos_domain::SymbolId::new(&sym.name, None, Language::Unknown);
                symbols_by_address.insert(sym.address, (sid, sym.name.clone()));
            }
        }
        if symbols_by_address.is_empty() {
            return None;
        }
        Some(Self {
            symbols_by_address,
            per_thread_stack: HashMap::new(),
        })
    }

    /// Number of tracked addresses (test/debug accessor).
    pub fn tracked_addresses(&self) -> usize {
        self.symbols_by_address.len()
    }

    /// Resolve an IP to a known `(SymbolId, function_name)` pair, if any.
    pub fn lookup(&self, ip: u64) -> Option<&(chronos_domain::SymbolId, String)> {
        self.symbols_by_address.get(&ip)
    }

    /// Process a SIGTRAP stop at `ip` on thread `tid` at monotonic `mono_ns`.
    ///
    /// Each SIGTRAP at a known function entry address is treated as a
    /// new entry — we push a new `ActiveInvocation` and emit a
    /// `FunctionEntry`. Exit detection in this cycle is intentionally
    /// omitted (a fully correct exit detector needs DWARF unwinding
    /// which is M3+ work); the `flush_incomplete_on_exit` helper
    /// handles the "still active at probe-detach" case explicitly.
    ///
    /// Returns `Some(FunctionEntry)` when the IP matches a known
    /// symbol; `None` otherwise.
    pub fn on_sigtrap(&mut self, tid: ThreadId, ip: u64, mono_ns: u64) -> Option<TraceEvent> {
        let (symbol_id, name) = self.lookup(ip)?.clone();
        let stack = self.per_thread_stack.entry(tid).or_default();
        let parent = stack.last().map(|a| a.invocation_id);
        let invocation_id = InvocationId::now();
        stack.push(ActiveInvocation {
            invocation_id,
            parent_invocation_id: parent,
            symbol_id,
            entry_monotonic_ns: mono_ns,
            entry_ip: ip,
            function_name: name.clone(),
        });
        Some(TraceEvent {
            event_id: mono_ns,
            timestamp_ns: TimestampNs::from_ns(mono_ns),
            thread_id: tid,
            event_type: EventType::FunctionEntry,
            location: SourceLocation {
                function: Some(name.clone()),
                address: ip,
                ..Default::default()
            },
            data: EventData::Function {
                name,
                signature: None,
                symbol_id: Some(symbol_id),
                invocation_id: Some(invocation_id),
                parent_invocation_id: parent,
            },
        })
    }

    /// Flush every still-active invocation as
    /// `EventType::InvocationIncomplete`. Returns the events in LIFO
    /// order (deepest frame first).
    ///
    /// Called when the probe loop detects SIGKILL on the child or
    /// process exit without a paired FunctionExit for the active call.
    pub fn flush_incomplete_on_exit(&mut self) -> Vec<TraceEvent> {
        let mut out = Vec::new();
        // Iterate threads in deterministic order for test reproducibility.
        let tids: Vec<ThreadId> = self.per_thread_stack.keys().copied().collect();
        for tid in tids {
            if let Some(stack) = self.per_thread_stack.get_mut(&tid) {
                while let Some(active) = stack.pop() {
                    out.push(TraceEvent {
                        event_id: active.entry_monotonic_ns,
                        timestamp_ns: TimestampNs::from_ns(active.entry_monotonic_ns),
                        thread_id: tid,
                        event_type: EventType::InvocationIncomplete,
                        location: SourceLocation {
                            function: Some(active.function_name.clone()),
                            address: active.entry_ip,
                            ..Default::default()
                        },
                        data: EventData::Function {
                            name: active.function_name,
                            signature: None,
                            symbol_id: Some(active.symbol_id),
                            invocation_id: Some(active.invocation_id),
                            parent_invocation_id: active.parent_invocation_id,
                        },
                    });
                }
            }
        }
        out
    }
}

// `TimestampNs` is a u64 type alias; this is just for clarity in callsites.
trait FromNs {
    fn from_ns(ns: u64) -> Self;
}
impl FromNs for TimestampNs {
    fn from_ns(ns: u64) -> Self {
        ns
    }
}

/// Helper used by tests and integration code to build a tracker
/// against an in-memory symbol table without spinning up a real
/// `SymbolResolver`. Owners may want to mock the resolver; for now we
/// just expose the symbol map directly.
impl InvocationTracker {
    /// Construct a tracker from a precomputed address → (SymbolId, name)
    /// map. Used by tests and integration code.
    pub fn from_symbols(symbols: HashMap<u64, (chronos_domain::SymbolId, String)>) -> Self {
        Self {
            symbols_by_address: symbols,
            per_thread_stack: HashMap::new(),
        }
    }

    /// Total count of still-active invocations across all threads.
    pub fn active_invocations(&self) -> usize {
        self.per_thread_stack.values().map(|s| s.len()).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chronos_domain::Language;

    fn sid(name: &str) -> (chronos_domain::SymbolId, String) {
        (
            chronos_domain::SymbolId::new(name, None, Language::C),
            name.to_string(),
        )
    }

    #[test]
    fn recursive_distinct_invocation_ids() {
        // Addresses: factorial at 0x1000. Each SIGTRAP at 0x1000 is
        // a new recursive entry — the tracker pushes, never pops.
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sid("factorial"));
        let mut t = InvocationTracker::from_symbols(symbols);

        // Simulate three recursive entries.
        let e1 = t.on_sigtrap(1, 0x1000, 1).unwrap();
        let e2 = t.on_sigtrap(1, 0x1000, 2).unwrap();
        let e3 = t.on_sigtrap(1, 0x1000, 3).unwrap();

        let entry_ids: Vec<_> = [&e1, &e2, &e3]
            .iter()
            .map(|e| {
                if let EventData::Function {
                    invocation_id: Some(id),
                    ..
                } = &e.data
                {
                    *id
                } else {
                    panic!("expected Function with invocation_id")
                }
            })
            .collect();
        // All three are entries; each gets a distinct InvocationId.
        assert_ne!(entry_ids[0], entry_ids[1], "recursive call 1 distinct");
        assert_ne!(entry_ids[1], entry_ids[2], "recursive call 2 distinct");
        assert_ne!(entry_ids[0], entry_ids[2], "outer and inner distinct");

        // Three events, all FunctionEntry (no auto-exit in this cycle).
        assert_eq!(e1.event_type, EventType::FunctionEntry);
        assert_eq!(e2.event_type, EventType::FunctionEntry);
        assert_eq!(e3.event_type, EventType::FunctionEntry);

        // Stack holds three active invocations.
        assert_eq!(t.active_invocations(), 3);
    }

    #[test]
    fn parent_link_chains_correctly() {
        // Two distinct functions: a() calls b(). Stack: a→b.
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sid("a"));
        symbols.insert(0x2000, sid("b"));
        let mut t = InvocationTracker::from_symbols(symbols);

        let ea = t.on_sigtrap(1, 0x1000, 1).unwrap();
        let eb = t.on_sigtrap(1, 0x2000, 2).unwrap();

        let pa = match &ea.data {
            EventData::Function {
                invocation_id: Some(id),
                ..
            } => *id,
            _ => panic!("ea must have invocation_id"),
        };
        let (pb, parent_b) = match &eb.data {
            EventData::Function {
                invocation_id,
                parent_invocation_id,
                ..
            } => (
                invocation_id.expect("eb must have invocation_id"),
                *parent_invocation_id,
            ),
            _ => panic!(),
        };
        assert!(parent_b.is_some(), "b must have a parent");
        assert_eq!(parent_b.unwrap(), pa);
        assert_ne!(pa, pb, "pa and pb are distinct invocations");
    }

    #[test]
    fn kill_mid_stack_emits_one_incomplete_per_active() {
        // a→b still active when "kill" happens.
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sid("a"));
        symbols.insert(0x2000, sid("b"));
        let mut t = InvocationTracker::from_symbols(symbols);
        let _ = t.on_sigtrap(1, 0x1000, 1).unwrap();
        let _ = t.on_sigtrap(1, 0x2000, 2).unwrap();
        assert_eq!(t.active_invocations(), 2);

        let flushed = t.flush_incomplete_on_exit();
        assert_eq!(flushed.len(), 2);
        // LIFO order: b first, then a.
        assert_eq!(flushed[0].event_type, EventType::InvocationIncomplete);
        assert_eq!(flushed[1].event_type, EventType::InvocationIncomplete);

        let name_b = match &flushed[0].data {
            EventData::Function { name, .. } => name.clone(),
            _ => panic!(),
        };
        let name_a = match &flushed[1].data {
            EventData::Function { name, .. } => name.clone(),
            _ => panic!(),
        };
        assert_eq!(name_b, "b");
        assert_eq!(name_a, "a");

        assert_eq!(t.active_invocations(), 0);
    }

    #[test]
    fn unknown_address_emits_nothing() {
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sid("a"));
        let mut t = InvocationTracker::from_symbols(symbols);
        assert!(t.on_sigtrap(1, 0x9999, 1).is_none());
        assert_eq!(t.active_invocations(), 0);
    }
}
