use chronos_log::*;
use chronos_log::{
    ConsumerCursor, ExecutionKind, ExecutionLogBackend, ExecutionPayload, InMemoryExecutionLog,
    LogConsumerId, NewExecutionRecord, SessionId,
};
use std::time::Instant;

fn seed(n: u64) -> (InMemoryExecutionLog, SessionId) {
    let log = InMemoryExecutionLog::new();
    let s = SessionId::new("t");
    for i in 0..n {
        log.append(NewExecutionRecord {
            session_id: s.clone(),
            kind: ExecutionKind::Raw,
            monotonic_ns: i,
            payload: ExecutionPayload::new(vec![7u8; 64], "s"),
            ..Default::default()
        })
        .expect("append");
    }
    (log, s)
}

fn main() {
    for n in [20u64, 200_000, 1_000_000] {
        let (log, s) = seed(n);
        let c = LogConsumerId::new("poll");
        let cur = ConsumerCursor::at(c.clone(), EventSeq::new(n - 1));
        let _ = log
            .read_after(s.clone(), c.clone(), Some(cur.clone()))
            .expect("warm");
        let reps = 20;
        let t = Instant::now();
        for _ in 0..reps {
            let r = log
                .read_after(s.clone(), c.clone(), Some(cur.clone()))
                .expect("read");
            if let ReadResult::Ok { records, .. } = r {
                assert!(records.is_empty());
            }
        }
        let el = t.elapsed();
        println!(
            "N={:>9}  caught-up read_after: {:>8.3} ms/call",
            n,
            el.as_secs_f64() * 1000.0 / reps as f64
        );
    }
}
