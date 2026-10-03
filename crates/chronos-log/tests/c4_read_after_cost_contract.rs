//! C4 of `docs/roadmap/SCALE_BUDGETS.md` §7, as a fast regression test.
//!
//! > **C4.** The cost of `read_after` must be proportional to what is
//! > RETURNED, not to the size of the session. A consumer that is caught up
//! > returns nothing and must pay nothing.
//!
//! This is the third arm of the same defect family. R2.2 fixed
//! `read_from_seq`, which cloned the whole session `Vec` before applying
//! `limit`; `read_after` kept that clone, and its own comment said so:
//! "this arm keeps its own full-clone cost, which is a separate open item".
//! It has production callers — `chronos_log::analytics`, `call_graph`, the
//! segmented backend that `chronos_capture::session_feed` and the MCP server
//! read through — so it is not a test-only cost.
//!
//! The steady state that matters is the POLL LOOP, not the first read: a
//! consumer that has caught up asks for "what is new", gets nothing, and
//! should allocate nothing. Today it allocates the whole session every time,
//! so a feed that idles on a large session pays O(N) per tick to learn that
//! nothing happened.
//!
//! The metric is **bytes allocated per call**, not wall clock, for the reason
//! C1/C2 gave: a duration assertion in the hot gate is a flaky gate, whereas
//! the allocation profile is a property of the code. The defect's arithmetic
//! signature is exactly a full-`Vec` clone, so it is what an allocator counter
//! sees.
//!
//! ONE `#[test]`, not several: the counting allocator is process-global, so two
//! tests measuring concurrently would read each other's bytes.
//!
//!     cargo test -p chronos-log --test c4_read_after_cost_contract -- --nocapture

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use chronos_log::{
    ConsumerCursor, ExecutionKind, ExecutionLogBackend, ExecutionPayload, InMemoryExecutionLog,
    LogConsumerId, NewExecutionRecord, SessionId,
};

const N_SMALL: u64 = 20;
const N_LARGE: u64 = 200_000;
const REPS: usize = 7;

struct Counting;

static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        ALLOCATED_BYTES.fetch_add(layout.size(), Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        ALLOCATED_BYTES.fetch_add(new_size, Relaxed);
        unsafe { System.realloc(ptr, layout, new_size) }
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn seed(n: u64) -> (InMemoryExecutionLog, SessionId) {
    let log = InMemoryExecutionLog::new();
    let s = SessionId::new("c4-cost");
    for i in 0..n {
        log.append(NewExecutionRecord {
            session_id: s.clone(),
            kind: ExecutionKind::Raw,
            monotonic_ns: i,
            payload: ExecutionPayload::new(vec![7u8; 64], "scale"),
            ..Default::default()
        })
        .expect("append");
    }
    (log, s)
}

/// A cursor parked at the tail: the last seq present in the session. Passing it
/// back is what a caught-up consumer does on every poll, and what it must be
/// able to do without the cost scaling with `n`.
fn tail_cursor(n: u64) -> ConsumerCursor {
    ConsumerCursor::at(
        LogConsumerId::new("poll"),
        chronos_log::EventSeq::new(n.saturating_sub(1)),
    )
}

/// Median bytes allocated by one caught-up `read_after` call. The first call is
/// a warm-up: the very first allocations in a process also pay one-off
/// allocator growth, and this contract is about the steady state a poll loop
/// runs in.
fn alloc_per_caught_up_call(log: &InMemoryExecutionLog, s: &SessionId, n: u64) -> (usize, usize) {
    let consumer = LogConsumerId::new("poll");
    let cursor = tail_cursor(n);
    let _ = log
        .read_after(s.clone(), consumer.clone(), Some(cursor.clone()))
        .expect("warm-up read");

    let mut samples = Vec::with_capacity(REPS);
    let mut returned = 0usize;
    for _ in 0..REPS {
        let before = ALLOCATED_BYTES.load(Relaxed);
        let result = log
            .read_after(s.clone(), consumer.clone(), Some(cursor.clone()))
            .expect("read");
        samples.push(ALLOCATED_BYTES.load(Relaxed) - before);
        match result {
            chronos_log::ReadResult::Ok { records, .. } => returned = records.len(),
            other => panic!("a caught-up consumer must get an Ok result, got {other:?}"),
        }
    }
    samples.sort_unstable();
    (samples[samples.len() / 2], returned)
}

#[test]
fn a_caught_up_read_after_costs_nothing_that_scales_with_the_session() {
    let (big, big_s) = seed(N_LARGE);
    let (small, small_s) = seed(N_SMALL);

    let (big_cost, big_returned) = alloc_per_caught_up_call(&big, &big_s, N_LARGE);
    let (small_cost, small_returned) = alloc_per_caught_up_call(&small, &small_s, N_SMALL);

    eprintln!(
        "caught-up alloc/call  N={N_LARGE}: {big_cost} B (returned {big_returned})\n\
         caught-up alloc/call  N={N_SMALL}: {small_cost} B (returned {small_returned})"
    );

    // The precondition, stated so the measurement cannot be read backwards: if
    // the cursor is not actually caught up, the "returns nothing" property
    // below would be passing for the wrong reason.
    assert_eq!(
        big_returned, 0,
        "the cursor parked at the tail must return no records, otherwise this \
         test is not measuring the caught-up steady state it describes"
    );

    // ---- C4, stated literally. --------------------------------------------
    // A 10,000x session must not cost 10,000x. The demand is 8x: a genuine
    // full-clone regression scales with N (≈10,000x), while the fixed costs a
    // caught-up call really has — building the cursor, the map lookup, the
    // result enum — are the same order on both sizes. 8x sits far enough below
    // 10,000x to only pass on a cost that stopped following the session.
    const MAX_N_RATIO: usize = 8;
    assert!(
        big_cost <= small_cost * MAX_N_RATIO,
        "C4 FAILS: a caught-up read_after over a {N_LARGE}-event session allocated \
         {big_cost} B against {small_cost} B over {N_SMALL} — a ratio of {:.1}x for a \
         {:.0}x larger session. That is the full-Vec clone signature: the cost of \
         learning that NOTHING is new is proportional to the size of the log it \
         learned it from.",
        big_cost as f64 / small_cost.max(1) as f64,
        N_LARGE as f64 / N_SMALL as f64
    );
}
