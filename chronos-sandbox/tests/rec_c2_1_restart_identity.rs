//! REC-C2.1.7 — restart/replay identity for tripwire firing evidence.
//!
//! The requirement this proves:
//!
//! > A firing survives because it is **evidence**, not because a queue or a
//! > `Tripwire` object survived.
//!
//! Two fully separate MCP processes over the same durable root:
//!
//! ```text
//! PROCESS A   fresh manager, register nothing
//!             observe(list, scope=session{S})
//!               -> firing_seq F1, source_seq S1, tripwire_id T
//!             observe(query, scope=session{S})
//!               -> subscriptions [] (the manager is empty)
//!             capture cursor_after_F
//!             exit
//!
//! PROCESS B   same root, same session, fresh manager (knows nothing of T)
//!             observe(list, scope=session{S})
//!               -> the SAME F1 -> S1, tripwire_id T
//!             observe(query) -> subscriptions [] still
//!             observe(list, cursor=cursor_after_F) -> 0 firings
//!             producer appends Raw S2 + Firing F2
//!             observe(list, cursor=cursor_after_F) -> F2 only
//! ```
//!
//! The fixture writes the raw and firing records directly (the same bytes the
//! derivation produces) so the test exercises the durable read path across
//! processes without depending on a live capture.

use std::path::{Path, PathBuf};

use chronos_domain::tripwire::{TripwireCondition, TripwireId};
use chronos_domain::{EventData, EventType, SourceLocation, TraceEvent};
use chronos_log::{
    tripwire_evidence_codec as codec, ExecutionKind, ExecutionPayload, NewExecutionRecord,
    SegmentedConfig, SegmentedExecutionLog, SessionId, TripwireFiredEvidence,
};
use chronos_sandbox::client::tools::McpTestClient;

const SESSION: &str = "rec-c2-1-restart";

fn unique_root(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "chronos-c21-restart-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("create root");
    p
}

fn trace_event(event_id: u64, name: &str, ts: u64) -> TraceEvent {
    TraceEvent {
        event_id,
        timestamp_ns: chronos_domain::MonotonicNs::from(ts),
        thread_id: 1,
        event_type: EventType::FunctionEntry,
        location: SourceLocation {
            function: Some(name.to_string()),
            ..SourceLocation::default()
        },
        data: EventData::Function {
            name: name.to_string(),
            signature: None,
            symbol_id: None,
            invocation_id: None,
            parent_invocation_id: None,
        },
    }
}

fn open_log(root: &Path) -> SegmentedExecutionLog {
    let dir = root.join(SESSION);
    std::fs::create_dir_all(&dir).expect("create session dir");
    SegmentedExecutionLog::open(SessionId::new(SESSION), SegmentedConfig::with_dir(&dir))
        .expect("open log")
}

/// Append a raw source record and the firing it causes.
///
/// Returns `(source_seq, firing_seq)`.
fn append_source_and_firing(
    log: &SegmentedExecutionLog,
    session: &SessionId,
    tripwire_id: u64,
    event_id: u64,
    ts: u64,
) -> (u64, u64) {
    let event = trace_event(event_id, "main_work", ts);
    let source_seq = log
        .append(NewExecutionRecord {
            session_id: session.clone(),
            kind: ExecutionKind::Raw,
            monotonic_ns: ts,
            payload: ExecutionPayload::new(
                serde_json::to_vec(&event).expect("encode"),
                "trace_event",
            ),
            ..Default::default()
        })
        .expect("append raw");

    let evidence = TripwireFiredEvidence {
        tripwire_id: TripwireId(tripwire_id),
        source_seq,
        source_event_id: Some(event_id),
        condition: TripwireCondition::FunctionName {
            pattern: "main*".to_string(),
        },
        label: Some("entry-watch".to_string()),
        source_timestamp_ns: ts,
        source_thread_id: 1,
    };
    let firing_seq = log
        .append(NewExecutionRecord {
            session_id: session.clone(),
            kind: ExecutionKind::TripwireFired,
            monotonic_ns: ts,
            payload: codec::encode(&evidence).expect("encode evidence"),
            ..Default::default()
        })
        .expect("append firing");
    log.flush().ok();
    (source_seq.0, firing_seq.0)
}

/// Call `observe` and return the `list` result object.
async fn observe_list(client: &mut McpTestClient, cursor: Option<&str>) -> serde_json::Value {
    let mut params = serde_json::json!({
        "verb": "list",
        "scope": {"scope": "session", "session_id": SESSION},
    });
    if let Some(c) = cursor {
        params["cursor"] = serde_json::Value::String(c.to_string());
    }
    let response = client
        .call_tool("observe", params)
        .await
        .expect("observe list");
    assert_eq!(
        response.get("kind").and_then(|k| k.as_str()),
        Some("list"),
        "expected a list envelope, got {response}"
    );
    response
}

async fn observe_query(client: &mut McpTestClient) -> serde_json::Value {
    let response = client
        .call_tool(
            "observe",
            serde_json::json!({
                "verb": "query",
                "scope": {"scope": "session", "session_id": SESSION},
            }),
        )
        .await
        .expect("observe query");
    assert_eq!(
        response.get("kind").and_then(|k| k.as_str()),
        Some("query"),
        "expected a query envelope, got {response}"
    );
    response
}

fn fired_events(response: &serde_json::Value) -> Vec<serde_json::Value> {
    response
        .get("fired_events")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// The firing survives a full runtime restart with a manager that has never
/// heard of the tripwire, and the EventSeq coordinate system survives with it.
#[tokio::test]
async fn firing_identity_and_cursor_survive_a_real_restart() {
    let root = unique_root("identity");
    let db = root.join("sessions.redb");

    // ---- Fixture: a source event and the firing it caused -------------
    let (s1, f1) = {
        let log = open_log(&root);
        let session = log.session_id().clone();
        append_source_and_firing(&log, &session, 7, 1001, 1_000)
    };
    assert!(f1 > s1, "the firing is appended after its source");

    // ---- PROCESS A ----------------------------------------------------
    let cursor_after_f = {
        let mut a = McpTestClient::start_with_db_and_exec_log_root(db.clone(), root.clone())
            .await
            .expect("start MCP A");

        let listed = observe_list(&mut a, None).await;
        let events = fired_events(&listed);
        assert_eq!(events.len(), 1, "A sees exactly one durable firing");
        assert_eq!(
            events[0].get("firing_seq").and_then(|v| v.as_u64()),
            Some(f1),
            "A: firing identity is its ExecutionRecord.seq"
        );
        assert_eq!(
            events[0].get("source_seq").and_then(|v| v.as_u64()),
            Some(s1),
            "A: cause identity is the accepted source seq"
        );
        assert_eq!(
            events[0].get("tripwire_id").and_then(|v| v.as_str()),
            Some("tripwire-7"),
            "A: the subscription snapshot survives"
        );

        // A's manager never registered anything either: query is empty.
        let queried = observe_query(&mut a).await;
        assert_eq!(
            queried
                .get("subscriptions")
                .and_then(|v| v.as_array())
                .map(|a| a.len()),
            Some(0),
            "A: no active subscriptions"
        );

        listed
            .get("next_cursor")
            .and_then(|v| v.as_str())
            .expect("A yields a checkpoint cursor")
            .to_string()
        // `a` (and the whole runtime) is dropped here.
    };

    // ---- PROCESS B: fresh server, fresh manager, same durable root ----
    let mut b = McpTestClient::start_with_db_and_exec_log_root(db, root.clone())
        .await
        .expect("start MCP B");

    // 1. The firing is still there, with both identities intact.
    let listed = observe_list(&mut b, None).await;
    let events = fired_events(&listed);
    assert_eq!(events.len(), 1, "B sees the durable firing");
    assert_eq!(
        events[0].get("firing_seq").and_then(|v| v.as_u64()),
        Some(f1),
        "B: the SAME firing seq as A"
    );
    assert_eq!(
        events[0].get("source_seq").and_then(|v| v.as_u64()),
        Some(s1),
        "B: the SAME source seq as A"
    );
    assert_eq!(
        events[0].get("tripwire_id").and_then(|v| v.as_str()),
        Some("tripwire-7"),
        "B: the subscription snapshot survived even though B never registered it"
    );
    assert_eq!(
        events[0]
            .get("condition_description")
            .and_then(|v| v.as_str()),
        Some("FunctionName { pattern: \"main*\" }"),
        "the firing is self-describing after replay"
    );

    // 2. Subscription lifecycle != firing evidence lifecycle.
    let queried = observe_query(&mut b).await;
    assert_eq!(
        queried
            .get("subscriptions")
            .and_then(|v| v.as_array())
            .map(|a| a.len()),
        Some(0),
        "B must not invent the subscription it never registered"
    );

    // 3. The cursor token from before the restart still works.
    let resumed = observe_list(&mut b, Some(&cursor_after_f)).await;
    assert!(
        fired_events(&resumed).is_empty(),
        "the pre-restart checkpoint already covered the firing"
    );

    // 4. Producer advances: a second source + firing.
    let (_s2, f2) = {
        let log = open_log(&root);
        let session = log.session_id().clone();
        append_source_and_firing(&log, &session, 7, 1002, 2_000)
    };
    assert_ne!(f2, f1);

    // A server that is already running holds the in-memory view it
    // bootstrapped at startup, so a new process is what observes new durable
    // records (the same property REC-C1.8 measured for late appends).
    drop(b);
    let mut c =
        McpTestClient::start_with_db_and_exec_log_root(root.join("sessions.redb"), root.clone())
            .await
            .expect("start MCP C");

    // The cursor minted by process A still means the same thing in C: it
    // yields ONLY the new firing, because the EventSeq coordinate system
    // survived the restarts along with the evidence.
    let advanced = observe_list(&mut c, Some(&cursor_after_f)).await;
    let advanced_events = fired_events(&advanced);
    assert_eq!(
        advanced_events.len(),
        1,
        "the surviving coordinate system yields exactly the new firing, not the old one"
    );
    assert_eq!(
        advanced_events[0]
            .get("firing_seq")
            .and_then(|v| v.as_u64()),
        Some(f2)
    );
    // And a cursor-less read still sees both firings, in order.
    let all = fired_events(&observe_list(&mut c, None).await);
    assert_eq!(all.len(), 2, "both firings are durable and replayable");
    assert_eq!(all[0].get("firing_seq").and_then(|v| v.as_u64()), Some(f1));
    assert_eq!(all[1].get("firing_seq").and_then(|v| v.as_u64()), Some(f2));
}
