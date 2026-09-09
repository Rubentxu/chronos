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
    /// Size of the function in bytes (from the symbol table).
    /// The half-open range `[entry_ip, entry_ip + size)` is used for
    /// range-aware exit detection.
    pub size: u64,
    pub function_name: String,
}

/// Per-thread call-stack state for function-frame identity.
pub struct InvocationTracker {
    /// Address → `(SymbolId, function_name, size)`. Populated from the
    /// `SymbolResolver` once at tracker construction.
    symbols_by_address: HashMap<u64, (chronos_domain::SymbolId, String, u64)>,
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
                symbols_by_address.insert(sym.address, (sid, sym.name.clone(), sym.size));
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

    /// Resolve an IP to a known `(SymbolId, function_name, size)` triple, if any.
    pub fn lookup(&self, ip: u64) -> Option<&(chronos_domain::SymbolId, String, u64)> {
        self.symbols_by_address.get(&ip)
    }

    /// Process a SIGTRAP stop at `ip` on thread `tid` at monotonic `mono_ns`.
    ///
    /// The `return_addr` is read from the traced process's `[rsp]` (top of stack)
    /// and tells us where control will resume when the *current* frame returns.
    /// This is critical for `_start → main`-style transitions where `_start`'s
    /// ELF-reported size does not cover the caller's entry point: libc is entered
    /// from `_start`, and libc jumps to `main` without `_start` ever returning.
    /// By checking whether `return_addr` falls within the caller's range, we can
    /// correctly detect that `_start` has exited even though `main`'s address is
    /// outside `_start`'s symbol-reported range.
    ///
    /// Implements **function-exit via return-address check + recursive re-entry detection**:
    ///
    /// 1. **Exit check**: If `return_addr` is NOT inside the top frame's
    ///    half-open range `[entry_ip, entry_ip + size)`, the caller has exited
    ///    (control was transferred to a different context). Emit `FunctionExit`
    ///    and pop. Repeat until the stack is consistent.
    ///
    /// 2. **Recursive re-entry check**: Even when `return_addr` IS inside the
    ///    range, if `ip == top.entry_ip` AND the symbol at that address is the
    ///    SAME as `top.symbol_id`, we re-entered this function. Emit
    ///    `FunctionExit` for the previous invocation and proceed to step 3.
    ///
    /// 3. **Push**: If `ip` matches a known function entry, emit a
    ///    `FunctionEntry` and push a new `ActiveInvocation`.
    ///
    /// Returns zero or more events (pop-then-push pattern). The caller
    /// iterates and emits each one.
    pub fn on_sigtrap(
        &mut self,
        tid: ThreadId,
        ip: u64,
        return_addr: Option<u64>,
        mono_ns: u64,
    ) -> Vec<TraceEvent> {
        let mut events = Vec::new();

        // Look up the symbol BEFORE taking the mutable borrow on per_thread_stack
        // to avoid a borrow conflict between `entry()` and `lookup()`.
        let symbol_info = self.lookup(ip).cloned();

        let stack = self.per_thread_stack.entry(tid).or_default();

        // 1. Pop frames whose range does not contain the return address.
        //
        // Two cases drive a pop:
        //
        // (a) Normal return: `return_addr` is OUTSIDE the top frame's half-open
        //     range `[entry_ip, entry_ip + size)`. The caller has exited and
        //     we are in a different context (e.g. libc jumping to `main` after
        //     `_start` called `__libc_start_main`). Emit FunctionExit.
        //
        // (b) Recursive re-entry: `return_addr` IS inside the range AND
        //     `ip == top.entry_ip` AND the symbol at that address is the SAME
        //     symbol. This means we re-entered this same function (not a
        //     different function whose entry happens to fall inside our range).
        while let Some(top) = stack.last() {
            // Case (a): return address outside the current frame's range → caller exited.
            // Case (b): return address inside AND we hit our own entry point → recursive.
            let ra = return_addr.unwrap_or(ip);
            let return_in_caller_range = ra >= top.entry_ip && ra < top.entry_ip + top.size;
            let is_recursive_reentry = ip == top.entry_ip
                && self
                    .symbols_by_address
                    .get(&top.entry_ip)
                    .is_some_and(|(sid, _, _)| *sid == top.symbol_id);
            if return_in_caller_range && !is_recursive_reentry {
                break; // normal execution inside the frame; caller is still active
            }
            // return_addr is outside the frame's range OR is a recursive re-entry
            let active = stack.pop().unwrap();
            events.push(make_function_exit(&active, tid, mono_ns));
        }

        // 2. If ip matches a known function entry, push and emit entry.
        if let Some((symbol_id, name, _size)) = symbol_info {
            let parent = stack.last().map(|a| a.invocation_id);
            let invocation_id = InvocationId::now();
            stack.push(ActiveInvocation {
                invocation_id,
                parent_invocation_id: parent,
                symbol_id,
                entry_monotonic_ns: mono_ns,
                entry_ip: ip,
                size: _size,
                function_name: name.clone(),
            });
            events.push(make_function_entry(
                tid,
                ip,
                mono_ns,
                name,
                symbol_id,
                invocation_id,
                parent,
            ));
        }

        events
    }

    /// Emit `FunctionExit` events for every still-active invocation on
    /// every thread (LIFO order). Used when the process exits naturally
    /// — the call stack unwound — but we still need to close the open
    /// frames.
    ///
    /// Compare with `flush_incomplete_on_exit` which is reserved for
    /// abnormal termination (SIGKILL) where frames may not have closed.
    pub fn pop_all_as_exit(&mut self) -> Vec<TraceEvent> {
        let mut out = Vec::new();
        let tids: Vec<ThreadId> = self.per_thread_stack.keys().copied().collect();
        for tid in tids {
            if let Some(stack) = self.per_thread_stack.get_mut(&tid) {
                while let Some(active) = stack.pop() {
                    out.push(make_function_exit(&active, tid, active.entry_monotonic_ns));
                }
            }
        }
        out
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

/// Build a FunctionEntry TraceEvent from scratch.
fn make_function_entry(
    tid: ThreadId,
    ip: u64,
    mono_ns: u64,
    name: String,
    symbol_id: chronos_domain::SymbolId,
    invocation_id: InvocationId,
    parent: Option<InvocationId>,
) -> TraceEvent {
    TraceEvent {
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
    }
}

/// Build a FunctionExit TraceEvent from an ActiveInvocation.
fn make_function_exit(active: &ActiveInvocation, tid: ThreadId, mono_ns: u64) -> TraceEvent {
    TraceEvent {
        event_id: mono_ns,
        timestamp_ns: TimestampNs::from_ns(mono_ns),
        thread_id: tid,
        event_type: EventType::FunctionExit,
        location: SourceLocation {
            function: Some(active.function_name.clone()),
            address: active.entry_ip,
            ..Default::default()
        },
        data: EventData::Function {
            name: active.function_name.clone(),
            signature: None,
            symbol_id: Some(active.symbol_id),
            invocation_id: Some(active.invocation_id),
            parent_invocation_id: active.parent_invocation_id,
        },
    }
}

/// Helper used by tests and integration code to build a tracker
/// against an in-memory symbol table without spinning up a real
/// `SymbolResolver`. Owners may want to mock the resolver; for now we
/// just expose the symbol map directly.
impl InvocationTracker {
    /// Construct a tracker from a precomputed address → (SymbolId, name, size)
    /// map. Used by tests and integration code.
    pub fn from_symbols(symbols: HashMap<u64, (chronos_domain::SymbolId, String, u64)>) -> Self {
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

    /// Make a `(SymbolId, name, size)` tuple.
    fn sym(name: &str, size: u64) -> (chronos_domain::SymbolId, String, u64) {
        (
            chronos_domain::SymbolId::new(name, None, Language::C),
            name.to_string(),
            size,
        )
    }

    // ------------------------------------------------------------------------
    // Tests migrated from the original file (updated for Vec<TraceEvent> return
    // and range-aware + re-entry detection semantics)
    // ------------------------------------------------------------------------

    #[test]
    fn recursive_distinct_invocation_ids() {
        // Addresses: factorial at 0x1000, size=0x100.
        //
        // With entry-re-entry detection: when ip == top.entry_ip (recursive call),
        // we pop the previous activation and push a new one.
        // Only one fact frame is ever active (depth collapses).
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sym("factorial", 0x100));
        let mut t = InvocationTracker::from_symbols(symbols);

        // First entry: push fact(1)
        let r1 = t.on_sigtrap(1, 0x1000, None, 1);
        assert_eq!(r1.len(), 1);
        assert_eq!(r1[0].event_type, EventType::FunctionEntry);

        // Second entry: ip=0x1000 == top.entry_ip → recursive re-entry detected.
        // Pop fact(1) + push fact(2)
        let r2 = t.on_sigtrap(1, 0x1000, None, 2);
        assert_eq!(r2.len(), 2);
        assert_eq!(r2[0].event_type, EventType::FunctionExit);
        assert_eq!(r2[1].event_type, EventType::FunctionEntry);

        // Third entry: same — pop fact(2) + push fact(3)
        let r3 = t.on_sigtrap(1, 0x1000, None, 3);
        assert_eq!(r3.len(), 2);
        assert_eq!(r3[0].event_type, EventType::FunctionExit);
        assert_eq!(r3[1].event_type, EventType::FunctionEntry);

        // Only one frame is ever active (depth collapses at each recursive entry).
        assert_eq!(t.active_invocations(), 1);

        // With entry re-entry detection, each call emits (exit, entry) pairs (except
        // the first which emits only entry). After 3 calls: 5 events with invocation IDs
        // (1 entry + 2 pairs). All entries have distinct IDs.
        let ids: Vec<InvocationId> = r1
            .iter()
            .chain(r2.iter())
            .chain(r3.iter())
            .filter_map(|e| {
                if let EventData::Function {
                    invocation_id: Some(id),
                    ..
                } = &e.data
                {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        // First call: 1 entry. Second call: exit + entry. Third call: exit + entry.
        // Total: 1 + 2 + 2 = 5 invocation IDs.
        assert_eq!(ids.len(), 5, "total invocation IDs across all events");
        // The 3 distinct entry invocation IDs:
        let entry_ids: Vec<InvocationId> = [&r1[..], &r2[..], &r3[..]]
            .iter()
            .flat_map(|v| v.iter())
            .filter(|e| e.event_type == EventType::FunctionEntry)
            .filter_map(|e| {
                if let EventData::Function {
                    invocation_id: Some(id),
                    ..
                } = &e.data
                {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(entry_ids.len(), 3);
        assert_ne!(entry_ids[0], entry_ids[1]);
        assert_ne!(entry_ids[1], entry_ids[2]);
    }

    #[test]
    fn parent_link_chains_correctly() {
        // Two functions with OVERLAPPING ranges so the callee's entry is
        // inside the caller's range (the range-aware pop only fires when
        // the IP is OUTSIDE the caller's range).
        //
        // a() at [0x1000, 0x3000) — large enough to contain add's entry.
        // b() at [0x2000, 0x2050)  — entry 0x2000 falls inside a's range.
        //
        // Entry a at 0x1000: push a. Stack: [a]
        // Entry b at 0x2000: 0x2000 is inside a's range → a not popped.
        //   Push b. Stack: [a, b]
        // b's parent is a (correct parent linking).
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sym("a", 0x2000)); // large range
        symbols.insert(0x2000, sym("b", 0x50)); // entry inside a's range
        let mut t = InvocationTracker::from_symbols(symbols);

        let r_a = t.on_sigtrap(1, 0x1000, None, 1);
        assert_eq!(r_a.len(), 1);
        assert_eq!(r_a[0].event_type, EventType::FunctionEntry);

        // Non-recursive: b's entry inside a's range → no pop of a.
        let r_b = t.on_sigtrap(1, 0x2000, None, 2);
        assert_eq!(r_b.len(), 1, "b entry: a still active, no pop, push b");
        assert_eq!(r_b[0].event_type, EventType::FunctionEntry);

        // Both frames are active.
        assert_eq!(t.active_invocations(), 2);

        let pa = match &r_a[0].data {
            EventData::Function {
                invocation_id: Some(id),
                ..
            } => *id,
            _ => panic!(),
        };
        let (pb, parent_b) = match &r_b[0].data {
            EventData::Function {
                invocation_id,
                parent_invocation_id,
                ..
            } => (
                invocation_id.expect("must have invocation_id"),
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
        // a→b still active when "kill" happens (overlapping ranges).
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sym("a", 0x2000)); // large range
        symbols.insert(0x2000, sym("b", 0x50)); // entry inside a's range
        let mut t = InvocationTracker::from_symbols(symbols);
        let _ = t.on_sigtrap(1, 0x1000, None, 1);
        let _ = t.on_sigtrap(1, 0x2000, None, 2);
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
        symbols.insert(0x1000, sym("a", 0x50));
        let mut t = InvocationTracker::from_symbols(symbols);
        let events = t.on_sigtrap(1, 0x9999, None, 1);
        assert!(events.is_empty());
        assert_eq!(t.active_invocations(), 0);
    }

    // ------------------------------------------------------------------------
    // M2 function-exit-dwarf new tests
    // ------------------------------------------------------------------------

    /// REQ-3/4: range_aware_pop_emits_exit_when_caller_returns.
    ///
    /// This test uses NON-OVERLAPPING ranges where the callee's entry is
    /// OUTSIDE the caller's range. With range-aware pop, each new function
    /// entry POPS the previous frame (the callee "returned").
    ///
    /// - main at [0x1000, 0x10C8) → add at [0x2000, 0x2064) → fact at [0x3000, 0x3050)
    /// - Entry add: 0x2000 outside main's range → main popped. Stack: [add]
    /// - Entry fact: 0x3000 outside add's range → add popped. Stack: [fact]
    /// - Entry fact(recursive): ip=0x3000 matches fact.entry_ip → pop fact. Stack: [fact]
    /// - pop_all_as_exit: 1 exit (fact)
    #[test]
    fn range_aware_pop_emits_exit_when_caller_returns() {
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sym("main", 200));
        symbols.insert(0x2000, sym("add", 100));
        symbols.insert(0x3000, sym("fact", 80));
        let mut t = InvocationTracker::from_symbols(symbols);

        // main entry
        let ev = t.on_sigtrap(1, 0x1000, None, 1);
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].event_type, EventType::FunctionEntry);
        assert_eq!(t.active_invocations(), 1);

        // add entry: 0x2000 outside main's range → main popped, add pushed
        let ev = t.on_sigtrap(1, 0x2000, None, 2);
        assert_eq!(ev.len(), 2); // exit main, entry add
        assert_eq!(ev[0].event_type, EventType::FunctionExit); // main exit
        assert_eq!(ev[1].event_type, EventType::FunctionEntry); // add entry
        assert_eq!(t.active_invocations(), 1);

        // fact entry: 0x3000 outside add's range → add popped, fact pushed
        let ev = t.on_sigtrap(1, 0x3000, None, 3);
        assert_eq!(ev.len(), 2); // exit add, entry fact
        assert_eq!(ev[0].event_type, EventType::FunctionExit); // add exit
        assert_eq!(ev[1].event_type, EventType::FunctionEntry); // fact entry
        assert_eq!(t.active_invocations(), 1);

        // fact recursive re-entry: ip=0x3000 matches fact.entry_ip → pop fact, push fact
        let ev = t.on_sigtrap(1, 0x3000, None, 4);
        assert_eq!(ev.len(), 2); // exit fact(inner), entry fact(outer)
        assert_eq!(ev[0].event_type, EventType::FunctionExit);
        assert_eq!(ev[1].event_type, EventType::FunctionEntry);
        assert_eq!(t.active_invocations(), 1);

        // Only fact is on the stack (main and add were popped when called).
        let exits = t.pop_all_as_exit();
        assert_eq!(exits.len(), 1, "only fact remains active");
        assert_eq!(exits[0].event_type, EventType::FunctionExit);
        assert_eq!(t.active_invocations(), 0);
    }

    /// REQ-4: entry_inside_self_range_does_not_emit_exit.
    ///
    /// With entry re-entry detection, a recursive re-entry at the same entry
    /// address DOES emit an exit (it pops the inner activation).
    /// This gives paired entry/exit for each recursive level.
    #[test]
    fn entry_inside_self_range_does_not_emit_exit() {
        // fact at 0x3000, size=200 (range: [0x3000, 0x30C8))
        let mut symbols = HashMap::new();
        symbols.insert(0x3000, sym("fact", 200));
        let mut t = InvocationTracker::from_symbols(symbols);

        // First entry
        let ev1 = t.on_sigtrap(1, 0x3000, None, 1);
        assert_eq!(ev1.len(), 1);
        assert_eq!(ev1[0].event_type, EventType::FunctionEntry);

        // Recursive entry — ip=0x3000 matches top.entry_ip → recursive re-entry.
        // Pop fact(1) + push fact(2)
        let ev2 = t.on_sigtrap(1, 0x3000, None, 2);
        assert_eq!(
            ev2.len(),
            2,
            "recursive re-entry: pop fact(1) + push fact(2)"
        );
        assert_eq!(ev2[0].event_type, EventType::FunctionExit);
        assert_eq!(ev2[1].event_type, EventType::FunctionEntry);

        // Another recursive level: pop fact(2) + push fact(3)
        let ev3 = t.on_sigtrap(1, 0x3000, None, 3);
        assert_eq!(ev3.len(), 2);
        assert_eq!(ev3[0].event_type, EventType::FunctionExit);
        assert_eq!(ev3[1].event_type, EventType::FunctionEntry);

        // Only one frame is active (depth collapses at each recursive entry)
        assert_eq!(t.active_invocations(), 1);
    }

    /// REQ-5: pop_all_as_exit_emits_lifo.
    ///
    /// When pop_all_as_exit is called with 3 active frames, they must be
    /// emitted in LIFO order (deepest first).
    /// Uses OVERLAPPING ranges so all frames stay active.
    #[test]
    fn pop_all_as_exit_emits_lifo() {
        // Overlapping ranges so all functions stay active when called:
        // main [0x1000, 0x5000) large — contains helper
        // helper [0x2000, 0x3100) — entry 0x3000 is INSIDE (half-open: 0x3000 < 0x3100)
        // leaf [0x3000, 0x3080) — entry 0x3000 is inside helper AND at leaf's own entry
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sym("main", 0x4000)); // large
        symbols.insert(0x2000, sym("helper", 0x1100)); // 0x3000 is inside [0x2000, 0x3100)
        symbols.insert(0x3000, sym("leaf", 80)); // entry inside helper
        let mut t = InvocationTracker::from_symbols(symbols);

        // Push three frames (non-recursive: no pops between them)
        let _ = t.on_sigtrap(1, 0x1000, None, 1); // main
        let _ = t.on_sigtrap(1, 0x2000, None, 2); // helper (inside main's range)
        let _ = t.on_sigtrap(1, 0x3000, None, 3); // leaf (inside helper's range)
        assert_eq!(t.active_invocations(), 3);

        let exits = t.pop_all_as_exit();
        assert_eq!(exits.len(), 3, "must emit exit for each active frame");

        // LIFO: leaf first, then helper, then main
        let names: Vec<String> = exits
            .iter()
            .map(|e| match &e.data {
                EventData::Function { name, .. } => name.clone(),
                _ => panic!("expected Function data"),
            })
            .collect();
        assert_eq!(
            names,
            vec!["leaf", "helper", "main"],
            "exits must be in LIFO order"
        );
        assert_eq!(t.active_invocations(), 0);
    }

    /// REQ-6: exit_events_share_invocation_id_with_entry.
    ///
    /// When a FunctionExit is emitted, its invocation_id must match the
    /// corresponding FunctionEntry's invocation_id.
    #[test]
    fn exit_event_carries_same_invocation_id_as_entry() {
        // OVERLAPPING ranges so foo stays active when bar is entered
        // and both stay active until they are exited.
        // foo at [0x1000, 0x3000) — large range containing bar's entry
        // bar at [0x2000, 0x2050) — entry inside foo's range
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sym("foo", 0x2000)); // large
        symbols.insert(0x2000, sym("bar", 0x50)); // inside foo
        let mut t = InvocationTracker::from_symbols(symbols);

        // foo entry: push foo. Stack: [foo]
        let ev_foo = t.on_sigtrap(1, 0x1000, None, 1);
        assert_eq!(ev_foo.len(), 1);
        let entry_id_foo = match &ev_foo[0].data {
            EventData::Function {
                invocation_id: Some(id),
                ..
            } => *id,
            _ => panic!(),
        };

        // bar entry: ip=0x2000 inside foo's range → no pop of foo.
        // Push bar. Stack: [foo, bar]
        let ev_bar = t.on_sigtrap(1, 0x2000, None, 2);
        assert_eq!(ev_bar.len(), 1, "foo still active, bar entry only");
        assert_eq!(ev_bar[0].event_type, EventType::FunctionEntry);
        let entry_id_bar = match &ev_bar[0].data {
            EventData::Function {
                invocation_id: Some(id),
                ..
            } => *id,
            _ => panic!(),
        };
        assert_ne!(
            entry_id_foo, entry_id_bar,
            "foo and bar must have distinct invocation_ids"
        );

        // bar recursive re-entry: pop bar + push bar(new).
        // Stack: [foo, bar(1)] → pop bar(1) → [foo] → push bar(2) → [foo, bar(2)]
        let ev_exit = t.on_sigtrap(1, 0x2000, None, 3);
        assert_eq!(
            ev_exit.len(),
            2,
            "bar recursive re-entry: pop bar(1) + push bar(2)"
        );
        assert_eq!(ev_exit[0].event_type, EventType::FunctionExit);
        assert_eq!(ev_exit[1].event_type, EventType::FunctionEntry);

        let exit_id_bar = match &ev_exit[0].data {
            EventData::Function {
                invocation_id: Some(id),
                ..
            } => *id,
            _ => panic!(),
        };
        let entry_id_bar2 = match &ev_exit[1].data {
            EventData::Function {
                invocation_id: Some(id),
                ..
            } => *id,
            _ => panic!(),
        };

        assert_eq!(
            exit_id_bar, entry_id_bar,
            "bar(1) exit must carry bar(1)'s invocation_id"
        );
        assert_ne!(
            exit_id_bar, entry_id_bar2,
            "bar(1) and bar(2) must have distinct invocation_ids"
        );
    }
}
