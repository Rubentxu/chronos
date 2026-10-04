// Sonda de una sola ejecucion para DEBT-SCALE-MEM-01. NO es un test del repo:
// es un binario temporal que se borra tras la medicion. Vive aqui solo porque el
// harness de lanes necesita `mod common`, y `common` es codigo de test.
//
// La pregunta que responde: los 949 MB de RSS del servidor son memoria RETENIDA
// por el camino de lectura, o el pico de una operacion ya terminada que el
// allocator no ha devuelto al SO? Son cosas distintas y tienen arreglos
// distintos — la primera se arregla, la segunda se documenta.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use chronos_log::{
    ExecutionKind, ExecutionPayload, NewExecutionRecord, SegmentedConfig, SegmentedExecutionLog,
    SessionId,
};
use chronos_sandbox::client::tools::McpTestClient;
use serde_json::json;

const SESSION: &str = "rss-probe";
const EVENTS: u64 = 1_000_000;
static SEEDED: AtomicU64 = AtomicU64::new(0);

fn rss_kb(pid: u32) -> u64 {
    let raw = std::fs::read_to_string(format!("/proc/{pid}/statm")).expect("statm");
    let mut f = raw.split_whitespace();
    f.next().expect("size");
    f.next().expect("resident").parse::<u64>().expect("num") * 4
}

/// Pull the tool's own JSON payload out of a `tools/call` envelope.
///
/// A success carries one content block and it is the JSON. A structured failure
/// carries two — prose first, JSON second — so this takes the first block that
/// PARSES as JSON rather than blindly taking the first, which is the same trap
/// R2.9 found in the shipped helper. Copied here rather than imported because
/// `tests/common` is not reachable from `examples/`.
fn unwrap_payload(raw: &serde_json::Value) -> serde_json::Value {
    let blocks = raw["result"]["content"].as_array().unwrap_or_else(|| {
        panic!("tools/call must answer with a content array, got {raw}");
    });
    for b in blocks {
        if let Some(text) = b["text"].as_str() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(text) {
                return v;
            }
        }
    }
    panic!("no content block carried the tool's JSON payload, got {raw}");
}

/// The real `TraceEvent`, copied field-for-field from the shipped fixture in
/// `tests/common/mod.rs`.
///
/// An earlier version of this probe hand-rolled a plausible-looking JSON object
/// and the read path refused the log closed with `payload tag "trace_event"
/// could not be decoded`. The reader decodes into the actual domain type and
/// fails closed on anything else, so a fixture that is *nearly* right is worse
/// than none — the run reads as a product failure. Guessing the struct's
/// fields cost two more runs; this is transcribed, not invented.
fn trace_event(event_id: u64) -> chronos_domain::trace::TraceEvent {
    use chronos_domain::{EventData, EventType, MonotonicNs, SourceLocation};
    chronos_domain::trace::TraceEvent {
        event_id,
        timestamp_ns: MonotonicNs::from(event_id * 1_000),
        thread_id: 1,
        event_type: EventType::FunctionEntry,
        location: SourceLocation {
            function: Some("scale_work".to_string()),
            ..SourceLocation::default()
        },
        data: EventData::Function {
            name: "scale_work".to_string(),
            signature: None,
            symbol_id: None,
            invocation_id: None,
            parent_invocation_id: None,
        },
    }
}

#[tokio::main]
async fn main() {
    let dir = std::env::temp_dir().join("rss-probe-run");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let inner = dir.join(SESSION);
    std::fs::create_dir_all(&inner).unwrap();

    let session = SessionId::new(SESSION);
    let log = SegmentedExecutionLog::open(session.clone(), SegmentedConfig::with_dir(&inner))
        .expect("open");
    let t0 = Instant::now();
    for i in 1..=EVENTS {
        log.append(NewExecutionRecord {
            session_id: session.clone(),
            kind: ExecutionKind::Raw,
            monotonic_ns: i * 1_000,
            payload: ExecutionPayload::new(
                serde_json::to_vec(&trace_event(i)).expect("encode"),
                "trace_event",
            ),
            ..Default::default()
        })
        .expect("append");
    }
    log.flush().expect("flush");
    drop(log);
    SEEDED.store(EVENTS, Ordering::Relaxed);
    println!("seeded {EVENTS} in {:.1}s", t0.elapsed().as_secs_f64());

    let mut env = std::collections::HashMap::new();
    env.insert(
        "CHRONOS_EXECUTION_LOG_DIR".to_string(),
        dir.to_string_lossy().to_string(),
    );
    let mut client = McpTestClient::start_with_env(&McpTestClient::resolve_mcp_path(), &env)
        .await
        .expect("server");
    let pid = client.server_pid().expect("pid");

    let base = rss_kb(pid);
    println!("server RSS at boot:            {base:>9} KB");
    // The first run of this probe stopped one line below this one: the server
    // was already at ~600 MB having served ZERO aggregates. Whatever the read
    // path costs, it is not the whole of it — and a "1M aggregate needs 949 MB"
    // claim that never subtracts the boot cost is measuring two things at once.

    // `call_with_timeout` speaks raw JSON-RPC: the method is `tools/call` and
    // `params` IS the `{name, arguments}` envelope. Two wrong shapes were tried
    // before this one, and both looked like something else:
    //   * the tool NAME as the method -> -32601 on the name
    //   * `params` as the bare argument object -> accepted, and answered
    //     `total_events: 0` in 0,0 s, because the tool never ran; the zero came
    //     from reading the envelope instead of the payload
    // The working shape is the one the shipped lane uses, and the payload has
    // to come out of the envelope's first JSON content block.
    let summarize = || {
        json!({
            "name": "execution_log_read",
            "arguments": { "session_id": SESSION, "mode": "summarize", "bucket_size_ns": 1_000_000_000 },
        })
    };
    let t = Instant::now();
    let raw = client
        .call_with_timeout(
            "tools/call",
            summarize(),
            std::time::Duration::from_secs(600),
        )
        .await
        .unwrap_or_else(|e| panic!("summarize: {e}"));
    let secs = t.elapsed().as_secs_f64();
    let after1 = rss_kb(pid);
    let r = unwrap_payload(&raw);
    let total = r.get("total_events").and_then(|v| v.as_u64()).unwrap_or(0);
    // An aggregate that answered instantly over a million events did not run.
    assert_eq!(
        total, EVENTS,
        "the aggregate must have walked the log, got {raw}"
    );
    println!(
        "after aggregate #1 ({secs:.1}s, total {total}): {after1:>9} KB  (delta {:+})",
        after1 as i64 - base as i64
    );

    // The decisive measurement: do more aggregates. If RSS grows per call, the
    // path LEAKS and the cost scales with the number of requests, not with the
    // session. If it plateaus, the peak is one working set and the allocator
    // simply never gave it back.
    for i in 2..=4 {
        let t = Instant::now();
        let raw = client
            .call_with_timeout(
                "tools/call",
                summarize(),
                std::time::Duration::from_secs(600),
            )
            .await
            .unwrap_or_else(|e| panic!("summarize #{i}: {e}"));
        let p = unwrap_payload(&raw);
        let n = p.get("total_events").and_then(|v| v.as_u64()).unwrap_or(0);
        assert_eq!(n, EVENTS, "aggregate #{i} must walk the log, got {raw}");
        let now = rss_kb(pid);
        println!(
            "after aggregate #{i} ({:.1}s):            {now:>9} KB  (delta vs #1 {:+})",
            t.elapsed().as_secs_f64(),
            now as i64 - after1 as i64
        );
    }

    let final_rss = rss_kb(pid);
    println!("\nSUMMARY boot={base} peak={after1} final={final_rss} KB");
    println!(
        "retained after 4 aggregates: {} KB ({}% of peak)",
        final_rss as i64 - base as i64,
        if after1 > 0 {
            100.0 * (final_rss as f64 - base as f64) / after1 as f64
        } else {
            0.0
        } as i64
    );
    let _ = std::fs::remove_dir_all(&dir);
}
