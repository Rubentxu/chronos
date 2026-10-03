//! The client-visible contract of the D3 bucketing refusal.
//!
//! `chronos-services` proves the refusal exists and names the field
//! (`d3_a_bucketing_beyond_the_cap_is_refused_and_names_the_field`). That test
//! calls the service. This one calls the **tool**, over stdio, the way an agent
//! does — because the thing a client can act on is not the service's error
//! variant, it is the text that lands in the `isError` envelope, and nothing in
//! the services crate can prove that string survives the trip.
//!
//! ## Why this needs a wire test at all
//!
//! The gap this closes is the same shape as the one R3.1 found in the toolset:
//! a property that is true in the service and unproven at the boundary. The
//! mapping here has two independent hops —
//! `AggregateError::TooManyBuckets` → `ReadPathError::Aggregate(String)` → the
//! `isError` envelope the handler builds for any `Err` — and either hop could
//! lose the field name and leave the caller with an opaque failure it cannot
//! act on. A refusal that does not say what to change is a dead end, so the
//! text is the contract.
//!
//! ## The fixture is sized to cross the cap and nothing more
//!
//! `MAX_BUCKETS` is 1.000.000 and the cap is enforced against the **span**, not
//! the age of the host, so triggering it needs a span over a million
//! nanoseconds at a one-nanosecond bucket width. Twenty events one millisecond
//! apart span 19 ms: 19 buckets at the default width, 19.000.000 at one
//! nanosecond. That is over the cap by 19x and costs twenty records to set up.
//!
//! ## What is asserted
//!
//!   * The refusal is **actionable**: the text names `bucket_size_ns` and
//!     offers a width that would have worked. An opaque error would pass a
//!     "does it fail" test and still leave the caller stuck.
//!   * The **same log, same tool, same server** answers when the caller obeys
//!     the suggestion. Without this arm a cap set absurdly low would satisfy
//!     every assertion above it, which is the D2 lesson applied to buckets.
//!   * Nothing is truncated: the answering call reports every seeded event.
//!
//! STRUCTURE: one `#[tokio::test]`, one server, many assertions — for the
//! reason `execution_log_read_e2e` gives: a server's stdio is bound to the
//! runtime that booted it, and that runtime dies with the test.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use chronos_domain::trace::TraceEvent;
use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation};
use chronos_log::{
    ExecutionKind, ExecutionPayload, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog,
    SessionId,
};
use chronos_sandbox::client::tools::McpTestClient;
use serde_json::{json, Value};

const SESSION: &str = "bucket-cap-session";
/// 20 events, 1 ms apart: 19 ms of span, which is 19 buckets at the default
/// width and 19.000.000 at a one-nanosecond width.
const EVENTS: u64 = 20;
const EVENT_GAP_NS: u64 = 1_000_000;

fn temp_root() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "chronos-bucket-cap-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("create temp root");
    p
}

fn trace_event(event_id: u64) -> TraceEvent {
    TraceEvent {
        event_id,
        timestamp_ns: MonotonicNs::from(event_id * EVENT_GAP_NS),
        thread_id: 1,
        event_type: EventType::FunctionEntry,
        location: SourceLocation {
            function: Some("cap_work".to_string()),
            ..SourceLocation::default()
        },
        data: EventData::Function {
            name: "cap_work".to_string(),
            signature: None,
            symbol_id: None,
            invocation_id: None,
            parent_invocation_id: None,
        },
    }
}

fn seed(root: &Path) {
    let dir = root.join(SESSION);
    std::fs::create_dir_all(&dir).expect("create execution-log dir");
    let session_id = SessionId::new(SESSION);
    let log = SegmentedExecutionLog::open(session_id.clone(), SegmentedConfig::with_dir(&dir))
        .expect("open execution log for seeding");
    for i in 1..=EVENTS {
        log.append(NewExecutionRecord {
            session_id: session_id.clone(),
            kind: ExecutionKind::Raw,
            monotonic_ns: i * EVENT_GAP_NS,
            payload: ExecutionPayload::new(
                serde_json::to_vec(&trace_event(i)).expect("encode trace event"),
                "trace_event",
            ),
            ..Default::default()
        })
        .expect("append event");
    }
    log.flush().expect("flush seeded log");
}

/// The message an agent actually receives.
///
/// `isError: true` arrives as a transport `Err` whose message is the first
/// text block, which is how a client sees a refusal — `execution_log_read_e2e`
/// documents the same contract at the top of its file. So the refusal is
/// observed as an `Err`, and the assertion that matters is on its *text*.
///
/// Note what the reported span is: the distance at the moment the walk
/// crossed the cap, not the session's full extent. The walk stops at the first
/// event that does not fit, so a 19 ms session refuses at 2 ms with a suggested
/// width computed from what it had seen. That is why the suggestion can be
/// narrower than a width computed over the whole log would be, and it is
/// correct — it is the narrowest width that covers what the walk reached.
async fn summarize_refusal(client: &mut McpTestClient, bucket_size_ns: u64) -> String {
    match client
        .call_tool(
            "execution_log_read",
            json!({
                "session_id": SESSION,
                "mode": "summarize",
                "bucket_size_ns": bucket_size_ns,
            }),
        )
        .await
    {
        Ok(v) => panic!("a refusal must not arrive as a result, got {v}"),
        Err(e) => e.to_string(),
    }
}

#[tokio::test]
async fn a_bucketing_over_the_cap_is_refused_with_a_field_the_caller_can_change() {
    let root = temp_root();
    seed(&root);

    let mcp_path = McpTestClient::resolve_mcp_path();
    let mut env = HashMap::new();
    env.insert(
        "CHRONOS_EXECUTION_LOG_DIR".to_string(),
        root.to_string_lossy().to_string(),
    );
    let mut client = McpTestClient::start_with_env(&mcp_path, &env)
        .await
        .expect("MCP server must start against the seeded log root");

    // ---------------------------------------------------- the refusal, on the wire
    let refused = summarize_refusal(&mut client, 1).await;
    assert!(
        refused.contains("bucket_size_ns"),
        "the refusal must name the field the caller controls, or the caller has nothing to \
         change: {refused}"
    );
    assert!(
        refused.contains("1000000"),
        "the refusal must report the cap it hit, so a caller can tell a policy limit from a \
         fault: {refused}"
    );
    assert!(
        refused.contains("Widen"),
        "the refusal must say what to do next: {refused}"
    );
    // The span in the message is what the walk reached, and the message has to
    // mark it as such. Without that, a caller sizes its retry from a prefix of
    // the session and is refused again — which is exactly what an earlier
    // version of this error invited, by naming a width computed from it.
    assert!(
        refused.contains("not the session's full extent"),
        "the reported span must be marked as a floor, not the requirement: {refused}"
    );

    // ---------------------------------- and a width the caller can size answers
    // The fixture's full span is 19 ms, so the rule the message states — width
    // above span/1e6 — puts any width at 1 ms comfortably inside the cap.
    let obeyed = client
        .call_tool(
            "execution_log_read",
            json!({
                "session_id": SESSION,
                "mode": "summarize",
                "bucket_size_ns": EVENT_GAP_NS,
            }),
        )
        .await
        .unwrap_or_else(|e| {
            panic!("a width sized from the session must answer, not be refused again: {e:?}")
        });
    assert_eq!(
        obeyed["total_events"].as_u64(),
        Some(EVENTS),
        "obeying the rule must cover the whole log, not a prefix: {obeyed}"
    );

    // ------------------------------------------- and the default width still answers
    let ok = client
        .call_tool(
            "execution_log_read",
            json!({ "session_id": SESSION, "mode": "summarize" }),
        )
        .await
        .unwrap_or_else(|e| {
            panic!("the default width must answer on the very same log and server: {e:?}")
        });
    let total = ok["total_events"]
        .as_u64()
        .unwrap_or_else(|| panic!("a complete summarize must report total_events, got {ok}"));
    assert_eq!(
        total, EVENTS,
        "the answering call must cover the whole log, never a prefix: {ok}"
    );
    // 19 ms of span at a 1 s width is a single bucket, which is the D3 shape:
    // the count follows the session, not the age of the host.
    assert_eq!(
        ok["bucket_count"].as_u64(),
        Some(1),
        "19 ms of session under a 1 s bucket is one bucket wherever the host's clock is: {ok}"
    );

    let _ = std::fs::remove_dir_all(&root);
}

/// A field that the service accepts must not be refused at the boundary.
///
/// The inverse arm, and the reason it is not folded into the test above: a
/// change that made the transport reject widths the service is happy with
/// would pass every "it refuses" assertion there is.
#[tokio::test]
async fn a_width_the_service_accepts_is_answered_over_the_wire() {
    let root = temp_root();
    seed(&root);

    let mcp_path = McpTestClient::resolve_mcp_path();
    let mut env = HashMap::new();
    env.insert(
        "CHRONOS_EXECUTION_LOG_DIR".to_string(),
        root.to_string_lossy().to_string(),
    );
    let mut client = McpTestClient::start_with_env(&mcp_path, &env)
        .await
        .expect("MCP server must start against the seeded log root");

    // One millisecond per bucket over a 19 ms span: 19 populated buckets, well
    // inside the cap, and a width a caller would plausibly ask for.
    let ok = client
        .call_tool(
            "execution_log_read",
            json!({
                "session_id": SESSION,
                "mode": "summarize",
                "bucket_size_ns": EVENT_GAP_NS,
            }),
        )
        .await
        .unwrap_or_else(|e| panic!("a width inside the cap must answer: {e:?}"))
        as Value;

    assert_eq!(
        ok["total_events"].as_u64(),
        Some(EVENTS),
        "must account for every seeded event: {ok}"
    );
    assert_eq!(
        ok["bucket_count"].as_u64(),
        Some(EVENTS),
        "20 events 1 ms apart under a 1 ms bucket is one bucket each: {ok}"
    );

    let _ = std::fs::remove_dir_all(&root);
}
