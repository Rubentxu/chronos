//! REC-C2.1 — tripwire firing as durable ExecutionLog evidence.
//!
//! Contract under test:
//!
//! ```text
//! ExecutionRecord.seq   = identity of the firing as a durable fact
//! evidence.source_seq   = authoritative identity of the cause
//! tripwire_id           = snapshot of which subscription fired (not an identity)
//! ```
//!
//! A `TripwireFired` record must survive a segment round-trip with **both**
//! identities intact, and the historical `Raw` / `GapMarker` discriminants
//! must not move.

use std::path::PathBuf;

use chronos_domain::{TripwireCondition, TripwireId};
use chronos_log::segment::{read_segment, write_segment, SegmentEntry};
use chronos_log::{
    tripwire_evidence_codec as codec, EventSeq, ExecutionKind, ExecutionPayload, ExecutionRecord,
    SessionId, TripwireFiredEvidence, TRIPWIRE_FIRED_EVIDENCE_TAG,
};

fn tempdir(tag: &str) -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "chronos-c21-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).expect("create tempdir");
    p
}

fn evidence(source_seq: u64) -> TripwireFiredEvidence {
    TripwireFiredEvidence {
        tripwire_id: TripwireId(7),
        source_seq: EventSeq::new(source_seq),
        source_event_id: Some(83),
        condition: TripwireCondition::FunctionName {
            pattern: "main*".to_string(),
        },
        label: Some("entry-watch".to_string()),
        source_timestamp_ns: 700,
        source_thread_id: 1,
    }
}

fn raw_record(session: &SessionId, seq: u64) -> ExecutionRecord {
    ExecutionRecord {
        session_id: session.clone(),
        seq: EventSeq::new(seq),
        monotonic_ns: seq * 1000,
        kind: chronos_log::ExecutionKind::Raw,
        payload: ExecutionPayload::new(b"source-evidence".to_vec(), "trace_event"),
        invocation_id: None,
        parent_invocation_id: None,
        symbol_id: None,
        captured_at_unix_ns: None,
    }
}

fn fired_record(session: &SessionId, seq: u64, source_seq: u64) -> ExecutionRecord {
    ExecutionRecord {
        session_id: session.clone(),
        seq: EventSeq::new(seq),
        monotonic_ns: seq * 1000,
        kind: ExecutionKind::TripwireFired,
        payload: codec::encode(&evidence(source_seq)).expect("encode evidence"),
        invocation_id: None,
        parent_invocation_id: None,
        symbol_id: None,
        captured_at_unix_ns: None,
    }
}

/// The evidence type round-trips through `ExecutionPayload` and is
/// self-describing: `from_payload` on a non-firing payload returns `None`.
#[test]
fn evidence_round_trips_through_payload() {
    let ev = evidence(419);
    let payload = codec::encode(&ev).expect("encode");
    assert_eq!(payload.tag, TRIPWIRE_FIRED_EVIDENCE_TAG);
    let back = codec::decode(&payload)
        .expect("decode")
        .expect("some");
    assert_eq!(back, ev);

    // A source event payload is not a firing payload.
    let other = ExecutionPayload::new(b"{}".to_vec(), "trace_event");
    assert!(codec::decode(&other)
        .expect("decode")
        .is_none());
}

/// Fired evidence survives a real segment write/reopen with **both**
/// identities intact: the firing's own seq and the cause's source_seq.
#[test]
fn fired_evidence_survives_a_segment_round_trip_with_both_identities() {
    let dir = tempdir("roundtrip");
    let session = SessionId::new("rec-c2-1-evidence");

    // seq 419 = the accepted source; seq 421 = the derived firing.
    let entries = vec![
        SegmentEntry::Record(raw_record(&session, 419)),
        SegmentEntry::Record(fired_record(&session, 421, 419)),
    ];
    let path = write_segment(
        &dir,
        &session,
        EventSeq::new(419),
        EventSeq::new(421),
        2,
        &entries,
    )
    .expect("write segment");

    let decoded = read_segment(&path).expect("read segment");
    assert_eq!(decoded.entries.len(), 2);

    let SegmentEntry::Record(fired) = &decoded.entries[1] else {
        panic!("second entry must be the firing record");
    };
    assert_eq!(fired.kind, ExecutionKind::TripwireFired);
    assert_eq!(fired.seq, EventSeq::new(421), "firing identity");
    let ev = codec::decode(&fired.payload)
        .expect("decode")
        .expect("firing payload");
    assert_eq!(
        ev.source_seq,
        EventSeq::new(419),
        "cause identity survives the round trip"
    );
    assert_eq!(ev.tripwire_id, TripwireId(7));
    assert_eq!(ev.label.as_deref(), Some("entry-watch"));

    let _ = std::fs::remove_dir_all(&dir);
}
