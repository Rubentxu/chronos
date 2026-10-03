//! C2 of `docs/roadmap/SCALE_BUDGETS.md` §7.2, as a fast regression test.
//!
//! > **C2.** A N fijo, el coste de `poll` debe crecer con el límite `L`.
//! > La firma **defectuosa** es `cost(1) ≈ cost(100)`; la firma **correcta** es
//! > `cost(100)` claramente mayor que `cost(1)`.
//!
//! The measurement is **bytes allocated per call**, not wall clock. That is a
//! deliberate choice: a duration assertion in the hot gate is a flaky gate,
//! whereas the allocation profile is a property of the code, and it is the
//! exact metric `SCALE_BUDGETS.md` §3.1b already recorded for the defect (~422
//! MB transient per call, ≈ 1x the whole session, which is the arithmetic
//! signature of a full-Vec clone).
//!
//! It discriminates the defect in both of its layers:
//!
//!   * layer 1 (the clone) — any full-Vec clone makes every call allocate the
//!     whole session, so `cost(1)` scales with N;
//!   * layer 2 (the O(position) walk) — invisible to allocations, which is why
//!     the cost-is-flat-in-N assertion here is a necessary but not sufficient
//!     guard. The scan half is C1 proper, and C1 is measured, not asserted on a
//!     clock: see `chronos-sandbox/tests/scale_read_from_seq_c1.rs` (ignored,
//!     its own target) and the measured numbers in `SCALE_BUDGETS.md`.
//!
//! ONE `#[test]`, not several: the counting allocator is process-global, so two
//! tests measuring concurrently would read each other's bytes. Seeding once and
//! asserting three labelled properties in sequence keeps the comparison exact
//! — same session, same process, same allocator state.
//!
//!     cargo test -p chronos-log --test c1_c2_read_cost_contract -- --nocapture

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

use chronos_log::{
    ExecutionKind, ExecutionLogBackend, ExecutionPayload, InMemoryExecutionLog, NewExecutionRecord,
    SessionId,
};

/// Small session: big enough that the fixed costs of a call are not what is
/// being measured.
const N_SMALL: u64 = 20;
/// Large session: 10⁴ x `N_SMALL`.
const N_LARGE: u64 = 200_000;
const LIMIT_LOW: usize = 1;
const LIMIT_HIGH: usize = 100;
/// Medians over this many calls, as C1's own "forma de test" prescribes.
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
    let s = SessionId::new("c2-cost");
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

/// Median bytes allocated by one `read_from_seq` call, over `REPS` calls at
/// the tail of the log (a deep position, where the O(position) walk is at its
/// worst). The first call is a warm-up: the very first allocation in a process
/// also pays one-off allocator growth, and this contract is about the steady
/// state a poll loop runs in.
fn alloc_per_call(log: &InMemoryExecutionLog, s: &SessionId, n: u64, limit: usize) -> usize {
    let from = chronos_log::EventSeq::new(n.saturating_sub(limit as u64));
    let _ = log.read_from_seq(s, from, limit).expect("warm-up read");
    let mut samples = Vec::with_capacity(REPS);
    for _ in 0..REPS {
        let before = ALLOCATED_BYTES.load(Relaxed);
        let page = log.read_from_seq(s, from, limit).expect("read");
        samples.push(ALLOCATED_BYTES.load(Relaxed) - before);
        // Touch the result so the call cannot be optimised away, and assert
        // the page is real while we are here.
        assert_eq!(
            page.records.len(),
            limit.min(n as usize),
            "a page at the tail must deliver what was asked for"
        );
    }
    samples.sort_unstable();
    samples[samples.len() / 2]
}

#[test]
fn read_from_seq_costs_scale_with_the_limit_and_not_with_the_session() {
    let (big, big_s) = seed(N_LARGE);
    let (small, small_s) = seed(N_SMALL);

    let big_low = alloc_per_call(&big, &big_s, N_LARGE, LIMIT_LOW);
    let big_high = alloc_per_call(&big, &big_s, N_LARGE, LIMIT_HIGH);
    let small_low = alloc_per_call(&small, &small_s, N_SMALL, LIMIT_LOW);
    eprintln!(
        "alloc/call  N={N_LARGE} limit={LIMIT_LOW}: {big_low} B\n\
         alloc/call  N={N_LARGE} limit={LIMIT_HIGH}: {big_high} B\n\
         alloc/call  N={N_SMALL} limit={LIMIT_LOW}: {small_low} B"
    );

    // ---- C2, stated literally: cost grows with L at a fixed N. ------------
    // Declared constant, not fitted to the measurement: the defect's signature
    // is cost(1) == cost(100) (ratio 1.0) because the clone precedes the limit.
    // A 4x demand sits far from 1.0 and far below the ~100x a genuinely
    // limit-proportional cost shows.
    const MIN_LIMIT_RATIO: usize = 4;
    assert!(
        big_high >= big_low * MIN_LIMIT_RATIO,
        "C2 FAILS: a page of {LIMIT_HIGH} records must cost clearly more than a page of \
         {LIMIT_LOW} (demand >= {MIN_LIMIT_RATIO}x). Got {big_low} B vs {big_high} B, \
         ratio {:.2}x — that is the full-Vec clone signature, where `limit` is \
         irrelevant because the clone happens before the limit is applied.",
        big_high as f64 / big_low as f64
    );

    // ---- The other half of C2: the cost must not be the session. -----------
    // `limit` is fixed at 1 here and N grows by 10^4, so a full-Vec clone grows
    // by 10^4 and a windowed read does not grow at all. The bound is a
    // declared constant, two orders of magnitude below the N ratio.
    const MAX_SESSION_RATIO: usize = 8;
    assert!(
        big_low <= small_low * MAX_SESSION_RATIO,
        "C2 FAILS: one record's page must not cost the session. N grew {MAX_SESSION_RATIO}x \
         less than allowed ({N_SMALL} -> {N_LARGE} is 10000x) and the per-call allocation grew \
         {:.1}x, from {small_low} B to {big_low} B.",
        big_low as f64 / small_low as f64
    );
}
