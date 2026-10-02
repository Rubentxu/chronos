//! `ExecutionSummary::total_function_calls` is the whole-trace call census.
//!
//! `execution_summary` truncates `top_functions` to the 20 hottest functions
//! before returning. That makes the list a *subtotal*: summing it understates
//! the trace on every session that called more than 20 distinct functions.
//! `total_function_calls` exists so that a consumer computing a share has a
//! denominator that is actually a census.
//!
//! The fixture is deliberately lopsided -- one hot function plus a long tail
//! of cold ones -- because that is the shape that makes the two numbers differ
//! the most and therefore the shape that catches a regression.

use chronos_domain::MonotonicNs;
use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
use chronos_query::QueryEngine;

/// Engine top-N cutoff, duplicated here on purpose: if the engine ever
/// changes it, this test must fail loudly rather than silently stop
/// discriminating.
const ENGINE_TOP_N: usize = 20;

const HOT_CALLS: u64 = 100;
const TAIL_FUNCTIONS: u64 = 99;
const TAIL_CALLS_EACH: u64 = 1;

fn entry_event(event_id: u64, function: &str) -> TraceEvent {
    TraceEvent::new(
        event_id,
        MonotonicNs::from(1000 + event_id * 10),
        1,
        EventType::FunctionEntry,
        SourceLocation::new("bench.rs", 1, function, 0x4000 + event_id * 8),
        EventData::Function {
            name: function.to_string(),
            signature: None,
            symbol_id: None,
            invocation_id: None,
            parent_invocation_id: None,
        },
    )
}

/// One hot function with 100 calls, plus 99 cold functions with 1 call each:
/// 100 distinct functions, 199 function calls in the trace.
fn lopsided_trace() -> Vec<TraceEvent> {
    let mut events = Vec::new();
    let mut event_id = 1u64;
    for _ in 0..HOT_CALLS {
        events.push(entry_event(event_id, "hot"));
        event_id += 1;
    }
    for tail in 0..TAIL_FUNCTIONS {
        events.push(entry_event(event_id, &format!("cold_{tail:02}")));
        event_id += 1;
    }
    events
}

fn expected_trace_total() -> u64 {
    HOT_CALLS + TAIL_FUNCTIONS * TAIL_CALLS_EACH
}

#[test]
fn total_function_calls_is_the_whole_trace_not_the_top_20_subtotal() {
    let engine = QueryEngine::new(lopsided_trace());
    let summary = engine.execution_summary("s1");

    let trace_total = expected_trace_total();
    assert_eq!(trace_total, 199, "fixture sanity: 100 hot + 99 x 1 cold");

    // The census covers every call in the trace, including the tail the
    // ranking cutoff drops.
    assert_eq!(
        summary.total_function_calls, trace_total,
        "total_function_calls must be the census of the whole trace, not the \
         sum over the truncated top_functions list"
    );

    // The ranking is still truncated, and its subtotal is strictly smaller.
    // This is the exact confusion the new field removes.
    let top_subtotal: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();
    assert_eq!(
        summary.top_functions.len(),
        ENGINE_TOP_N,
        "the engine is expected to truncate top_functions at 20; if this \
         changed, this test no longer discriminates a subtotal from a census"
    );
    assert_eq!(
        top_subtotal,
        HOT_CALLS + (ENGINE_TOP_N as u64 - 1) * TAIL_CALLS_EACH
    );
    assert_eq!(top_subtotal, 119);
    assert!(
        top_subtotal < summary.total_function_calls,
        "the top-20 subtotal ({}) must stay below the census ({}): that gap is \
         exactly what a share computed over the subtotal gets wrong",
        top_subtotal,
        summary.total_function_calls
    );
}

#[test]
fn total_function_calls_covers_a_fully_uniform_tail() {
    // 40 distinct functions x 2 calls each: the cutoff drops half of them,
    // so the subtotal and the census differ by exactly the dropped half.
    const FUNCTIONS: u64 = 40;
    const CALLS_EACH: u64 = 2;

    let mut events = Vec::new();
    let mut event_id = 1u64;
    for f in 0..FUNCTIONS {
        for _ in 0..CALLS_EACH {
            events.push(entry_event(event_id, &format!("fn_{f:02}")));
            event_id += 1;
        }
    }
    let engine = QueryEngine::new(events);
    let summary = engine.execution_summary("s1");

    assert_eq!(summary.top_functions.len(), ENGINE_TOP_N);
    assert_eq!(summary.total_function_calls, FUNCTIONS * CALLS_EACH);
    assert_eq!(summary.total_function_calls, 80);

    // Uniformity makes this a clean invariant rather than an accident of the
    // fixture: the census is always the subtotal plus the dropped tail, and
    // here the tail is exactly 20 functions x 2 calls.
    let top_subtotal: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();
    assert_eq!(top_subtotal, 40);
    assert_eq!(
        summary.total_function_calls - top_subtotal,
        (FUNCTIONS - ENGINE_TOP_N as u64) * CALLS_EACH,
        "census minus subtotal must be precisely the calls of the dropped tail"
    );
}

#[test]
fn total_function_calls_equals_top_subtotal_at_or_below_the_cutoff() {
    // At 20 or fewer distinct functions the two numbers coincide, which is
    // why so much existing coverage never noticed the difference.
    const FUNCTIONS: u64 = 20;
    const CALLS_EACH: u64 = 3;

    let mut events = Vec::new();
    let mut event_id = 1u64;
    for f in 0..FUNCTIONS {
        for _ in 0..CALLS_EACH {
            events.push(entry_event(event_id, &format!("fn_{f:02}")));
            event_id += 1;
        }
    }
    let engine = QueryEngine::new(events);
    let summary = engine.execution_summary("s1");

    assert_eq!(summary.top_functions.len(), FUNCTIONS as usize);
    let top_subtotal: u64 = summary.top_functions.iter().map(|f| f.call_count).sum();
    assert_eq!(summary.total_function_calls, 60);
    assert_eq!(
        summary.total_function_calls, top_subtotal,
        "below the cutoff the census and the subtotal must agree, so consumers \
         of the old value see no change on small sessions"
    );
}

#[test]
fn total_function_calls_is_zero_without_function_entry_events() {
    let events = vec![TraceEvent::new(
        1,
        MonotonicNs::from(10),
        1,
        EventType::ThreadCreate,
        SourceLocation::from_address(0x1000),
        EventData::Empty,
    )];
    let engine = QueryEngine::new(events);
    let summary = engine.execution_summary("s1");

    assert_eq!(summary.total_function_calls, 0);
    assert!(summary.top_functions.is_empty());
}
