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
use chronos_domain::trace::{MonotonicNs, ThreadId, TimestampNs};
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
    /// Next `event_id` to hand out, and the reason this field exists.
    ///
    /// `TraceEvent::event_id` is documented as the "monotonically increasing
    /// event identifier within a session". These events used to take their id
    /// from the clock instead of a counter, which cannot satisfy that: a single
    /// `on_sigtrap` receives one `mono_ns` and emits an exit for every frame it
    /// pops *and* an entry, so every one of those events carried the same id;
    /// `pop_all_as_exit` closes every open frame with one shared timestamp for
    /// the same reason; and `flush_incomplete_on_exit` reused the frame's
    /// `entry_monotonic_ns`, which is by construction the id its own
    /// `FunctionEntry` already took. Duplicate ids are not cosmetic — the query
    /// engine's `merge` drops the later event, so a frame and its own closure
    /// could be silently discarded from the session.
    next_event_id: u64,
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
            next_event_id: 0,
        })
    }

    /// Mint the next `event_id` from the session counter.
    ///
    /// Callers that are already holding a borrow of `self.per_thread_stack`
    /// cannot call this — hence the pattern of taking the counter into a local,
    /// minting from it, and writing it back before returning.
    fn alloc_event_id(next: &mut u64) -> u64 {
        let id = *next;
        *next = next.wrapping_add(1);
        id
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

        // The loops below already hold a mutable borrow of
        // `self.per_thread_stack`, so the session counter is taken into a local
        // and written back on every exit path.
        let mut next_event_id = self.next_event_id;

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

            if !is_recursive_reentry {
                if return_in_caller_range {
                    break; // normal execution inside the frame; caller is still active
                }
                // The return address is not inside this frame. When it is known,
                // keep unwinding only while it still belongs to a frame that is
                // on the stack. If it belongs to none, the frame below is a
                // *caller that is still running*, not a frame that has exited, and
                // unwinding it would emit a premature FunctionExit.
                //
                // The two checks above only ever looked at the top frame, which
                // assumed unwinding is consecutive. Recursion breaks that: on a
                // recursive re-entry the return address lies inside the recursive
                // function itself, so after popping it the caller below does not
                // contain the return address either — and used to be popped as
                // well, leaving the new invocation parentless.
                //
                // This only applies when a return address was actually observed.
                // Without one, `ra` is just `ip`, and there is nothing to check it
                // against: that is the `None` case in which unwinding proceeds.
                if return_addr.is_some()
                    && !stack
                        .iter()
                        .any(|f| ra >= f.entry_ip && ra < f.entry_ip + f.size)
                {
                    break;
                }
            }

            // Case (a) unwinding past a frame, or case (b) closing the previous
            // activation of a recursively re-entered function.
            let active = stack.pop().unwrap();
            events.push(make_function_exit(
                &active,
                tid,
                Self::alloc_event_id(&mut next_event_id),
                mono_ns,
            ));
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
                Self::alloc_event_id(&mut next_event_id),
                mono_ns,
                name,
                symbol_id,
                invocation_id,
                parent,
            ));
        }

        self.next_event_id = next_event_id;
        events
    }

    /// Emit `FunctionExit` events for every still-active invocation on
    /// every thread (LIFO order). Used when the process exits naturally
    /// — the call stack unwound — but we still need to close the open
    /// frames.
    ///
    /// Compare with `flush_incomplete_on_exit` which is reserved for
    /// abnormal termination (SIGKILL) where frames may not have closed.
    ///
    /// `mono_ns` is the moment the frames are being closed, and it is used for
    /// every emitted exit. It used to be `active.entry_monotonic_ns`, which
    /// stamped each exit with the moment its frame *opened*: a `FunctionExit`
    /// that carries its own entry timestamp, so every invocation closed here
    /// reported a duration of zero, and a "did the capture advance in time"
    /// check on the event stream failed. The defect stayed hidden while the
    /// unwind loop also closed these frames during traps — those did carry a
    /// real timestamp — and surfaced as soon as that loop stopped over-
    /// unwinding callers that were still running.
    pub fn pop_all_as_exit(&mut self, mono_ns: u64) -> Vec<TraceEvent> {
        let mut out = Vec::new();
        let mut next_event_id = self.next_event_id;
        let tids: Vec<ThreadId> = self.per_thread_stack.keys().copied().collect();
        for tid in tids {
            if let Some(stack) = self.per_thread_stack.get_mut(&tid) {
                while let Some(active) = stack.pop() {
                    out.push(make_function_exit(
                        &active,
                        tid,
                        Self::alloc_event_id(&mut next_event_id),
                        mono_ns,
                    ));
                }
            }
        }
        self.next_event_id = next_event_id;
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
        let mut next_event_id = self.next_event_id;
        // Iterate threads in deterministic order for test reproducibility.
        let tids: Vec<ThreadId> = self.per_thread_stack.keys().copied().collect();
        for tid in tids {
            if let Some(stack) = self.per_thread_stack.get_mut(&tid) {
                while let Some(active) = stack.pop() {
                    out.push(TraceEvent {
                        event_id: Self::alloc_event_id(&mut next_event_id),
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
        self.next_event_id = next_event_id;
        out
    }
}

// `TimestampNs` is now the typed `MonotonicNs` newtype; keep the local
// helper so callsites stay explicit about the clock domain.
trait FromNs {
    fn from_ns(ns: u64) -> Self;
}
impl FromNs for TimestampNs {
    fn from_ns(ns: u64) -> Self {
        MonotonicNs::from(ns)
    }
}

/// Build a FunctionEntry TraceEvent from scratch.
fn make_function_entry(
    tid: ThreadId,
    ip: u64,
    event_id: u64,
    mono_ns: u64,
    name: String,
    symbol_id: chronos_domain::SymbolId,
    invocation_id: InvocationId,
    parent: Option<InvocationId>,
) -> TraceEvent {
    TraceEvent {
        event_id,
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
fn make_function_exit(
    active: &ActiveInvocation,
    tid: ThreadId,
    event_id: u64,
    mono_ns: u64,
) -> TraceEvent {
    TraceEvent {
        event_id,
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
            next_event_id: 0,
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
        let exits = t.pop_all_as_exit(99);
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

        let exits = t.pop_all_as_exit(99);
        assert_eq!(exits.len(), 3, "must emit exit for each active frame");
        // Every exit carries the moment the frames were closed, not the moment
        // each one opened. Stamping them with their own entry timestamp made
        // every invocation report a duration of exactly zero.
        for e in &exits {
            assert_eq!(
                e.timestamp_ns,
                TimestampNs::from_ns(99),
                "exit of {:?} must be stamped when the frame closed, not when it opened",
                e.location.function
            );
        }

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

    /// A recursive call must not unwind its caller.
    ///
    /// The unwind loop decided whether a frame was still active by asking only
    /// whether the return address fell inside the topmost frame. On a recursive
    /// re-entry the top frame is the recursive function itself, and the return
    /// address points inside *it*, not inside the caller below. So the caller
    /// was asked next, did not contain the return address, and got popped
    /// anyway: `main` emitted its exit while it was still running, and the next
    /// `fact` frame entered with no parent at all.
    ///
    /// The existing `recursive_distinct_invocation_ids` test cannot catch this
    /// because it recurses with a single function: there is no caller frame
    /// below to be wrongly unwound.
    #[test]
    fn recursive_reentry_keeps_the_caller_frame_open() {
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sym("main", 0x100));
        symbols.insert(0x1100, sym("fact", 0x100));
        let mut t = InvocationTracker::from_symbols(symbols);

        // main enters.
        let r1 = t.on_sigtrap(1, 0x1000, None, 10);
        assert_eq!(r1.len(), 1, "main entry");
        let main_id = entry_id(&r1[0]);

        // main calls fact; the return address is the instruction after the
        // call, which lives inside main.
        let r2 = t.on_sigtrap(1, 0x1100, Some(0x1010), 20);
        assert_eq!(r2.len(), 1, "fact entry only, no unwind expected");
        let fact1_id = entry_id(&r2[0]);
        assert_eq!(
            parent_id(&r2[0]),
            Some(main_id),
            "fact(1) must be a child of main"
        );

        // fact calls itself; the return address now lives inside fact, not
        // inside main.
        let r3 = t.on_sigtrap(1, 0x1100, Some(0x1110), 30);
        assert_eq!(
            r3.len(),
            2,
            "exactly one unwind (fact(1)) plus one entry (fact(2)); got {:?}",
            r3.iter().map(|e| e.event_type).collect::<Vec<_>>()
        );
        assert_eq!(r3[0].event_type, EventType::FunctionExit);
        assert_eq!(
            exit_id(&r3[0]),
            fact1_id,
            "the frame that unwinds is fact(1), not main"
        );
        assert_eq!(
            r3[1].event_type,
            EventType::FunctionEntry,
            "the second event must be the new fact frame"
        );

        // The regression itself: fact(2) is still a call made by main.
        assert_eq!(
            parent_id(&r3[1]),
            Some(main_id),
            "a recursive call is still a call from the caller that made it"
        );

        // And main is still on the stack, still running.
        assert_eq!(
            t.active_invocations(),
            2,
            "main and the new fact must both still be active"
        );

        // One more level, to be sure it holds as the recursion deepens.
        let r4 = t.on_sigtrap(1, 0x1100, Some(0x1110), 40);
        assert_eq!(r4.len(), 2);
        assert_eq!(exit_id(&r4[0]), entry_id(&r3[1]));
        assert_eq!(parent_id(&r4[1]), Some(main_id));
        assert_eq!(t.active_invocations(), 2);
    }

    fn entry_id(e: &TraceEvent) -> InvocationId {
        match &e.data {
            EventData::Function {
                invocation_id: Some(id),
                ..
            } => *id,
            other => panic!("not a function event: {other:?}"),
        }
    }

    fn parent_id(e: &TraceEvent) -> Option<InvocationId> {
        match &e.data {
            EventData::Function {
                parent_invocation_id,
                ..
            } => *parent_invocation_id,
            other => panic!("not a function event: {other:?}"),
        }
    }

    fn exit_id(e: &TraceEvent) -> InvocationId {
        entry_id(e)
    }

    /// `TraceEvent::event_id` must be unique and increasing within the
    /// session — that is what the field is documented to be, and what the
    /// query engine's `merge` relies on when it drops an id it has already
    /// seen.
    ///
    /// The cases below do not depend on clock resolution at all: the clock is
    /// passed in explicitly, so feeding the same `mono_ns` twice is a legal
    /// input, and one `on_sigtrap` legitimately emits several events at once.
    /// Every collision here was guaranteed by construction before the fix:
    ///
    /// - a recursive re-entry emits an exit and an entry from one `mono_ns`;
    /// - `pop_all_as_exit` closes every open frame with one shared timestamp;
    /// - `flush_incomplete_on_exit` reused the frame's `entry_monotonic_ns`,
    ///   which is the very id its `FunctionEntry` already took.
    #[test]
    fn event_ids_are_unique_and_increasing_across_every_emitting_path() {
        fn ids(events: &[TraceEvent]) -> Vec<u64> {
            events.iter().map(|e| e.event_id).collect()
        }

        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sym("factorial", 0x100));
        symbols.insert(0x2000, sym("helper", 0x100));
        let mut t = InvocationTracker::from_symbols(symbols);

        // Two stops sharing one clock reading. A single `on_sigtrap` can emit
        // an exit and an entry for the same `mono_ns`, so using the same value
        // for both calls maximises the collisions without depending on how
        // many events each call happens to produce.
        let r1 = t.on_sigtrap(1, 0x1000, None, 500);
        let r2 = t.on_sigtrap(1, 0x2000, None, 500);
        // The still-open frames closed after an abnormal termination. This one
        // used to reuse `entry_monotonic_ns` — the very id the matching
        // `FunctionEntry` had already taken.
        let flushed = t.flush_incomplete_on_exit();

        let all: Vec<u64> = ids(&r1)
            .into_iter()
            .chain(ids(&r2))
            .chain(ids(&flushed))
            .collect();
        assert!(
            all.len() >= 4,
            "expected at least four events across the three calls, got {all:?}"
        );

        let mut sorted = all.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            all, sorted,
            "event ids must be strictly increasing within a session; got {all:?}"
        );

        // Control on the multi-frame close: two open frames, one timestamp.
        // `helper` sits inside `factorial`'s range, so the second stop nests
        // instead of unwinding the first frame.
        let mut symbols = HashMap::new();
        symbols.insert(0x1000, sym("factorial", 0x2000));
        symbols.insert(0x2000, sym("helper", 0x100));
        let mut t = InvocationTracker::from_symbols(symbols);
        t.on_sigtrap(1, 0x1000, None, 10);
        t.on_sigtrap(1, 0x2000, None, 20);
        assert_eq!(
            t.active_invocations(),
            2,
            "both frames must still be open, or this control proves nothing"
        );
        let closed = ids(&t.pop_all_as_exit(900));
        assert_eq!(closed.len(), 2);
        assert!(
            closed[0] < closed[1],
            "two frames closed at the same instant must still get distinct ids; got {closed:?}"
        );
    }
}
