//! `record_gap` must fail closed at the write boundary when the gap overlaps
//! evidence the session already holds.
//!
//! # Criterio de aceptación (escrito ANTES de implementar)
//!
//! **Condición que debe cumplir un `record_gap` válido:**
//!
//! > Tras `record_gap(s, [a..=b])` con `Ok`, la lista de entradas de `s` sigue
//! > siendo una partición contigua y sin repeticiones de su espacio de seqs: la
//! > entrada recién añadida ocupa `[a..=b]` y ninguna entrada previa —ni `Record`
//! > ni `Gap`— intersecta ese rango.
//!
//! Esa condición no es una preferencia de estilo: es exactamente el invariante
//! que el validador de reapertura exige. `build_replay_plan` recorre las
//! entradas del segmento en orden y exige que cada una empiece donde terminó la
//! anterior (`replay.rs`: `if entry_start != cursor { PayloadRangeMismatch }`).
//! Una entrada que revisitaba un rango ya ocupado rompe el encadenado, el
//! segmento se vuelve ilegible y **el log deja de poder reabrirse**.
//!
//! **La observación que falla hoy:** `record_gap` sólo valida el hueco *anterior*
//! al gap (`gap.first_missing > *allocator` ⇒ error) y empuja
//! `RecordEntry::Gap` sin comprobar nada contra lo ya escrito. Con 10 registros
//! `0..=9`, `record_gap(Gap 3..=5)` devuelve `Ok`, y el rango `3..=5` queda
//! declarado perdido mientras los registros 3, 4 y 5 siguen presentes. La
//! evidencia es contradictoria y contradictoria *de forma irrecuperable*: el
//! `flush` la persiste y la reabertura la rechaza.
//!
//! **Por qué "por debajo del allocator" no es una categoría legítima.** El
//! encargo distingue el gap retroactivo legítimo del gap solapado corrupto
//! como "empieza antes del allocator pero termina antes del último registro" vs
//! "contiene seqs ya escritos". Los dos tests de control de este fichero
//! (`control_*`) demuestran que esa distinción no se sostiene: en una sesión
//! cuyas entradas teselan el espacio de seqs —el estado que mantiene *todo* camino
//! de producción, porque `append` toma el siguiente seq y el overflow reserva el
//! suyo con `allocate_seq_for_gap`— **todo seq por debajo del allocator ya
//! está ocupado**. No existe un hueco real debajo. Un gap "por debajo" sólo
//! puede pisar un registro o pisar otro gap, y ambos casos producen un segmento
//! irreabrible. Por eso el guard pregunta por la intersección contra las
//! entradas reales, y no por la posición respecto al allocator.
//!
//! **La distinción operativa que sí funciona:**
//!
//! | caso | mínimo | veredicto |
//! |---|---|---|
//! | legítimamente adyacente | registros `0..=9`, `allocate_seq_for_gap`→`10`, gap `10..=10` | aceptado: no pisa nada |
//! | solapado (corrupto) | registros `0..=9`, gap `3..=5` | rechazado: pisa 3, 4 y 5 |
//! | gap sobre gap (corrupto) | registros `0..=9`, gap `10..=12`, luego gap `11..=11` | rechazado: pisa el gap previo |
//!
//! # Lo que este fichero fija
//!
//! 1. El solape se rechaza **antes** de escribirse (ni entrada en memoria, ni
//!    entrada en el buffer del segmento, ni fichero en disco).
//! 2. Tras el rechazo el log **sigue siendo reabrible** y conserva su
//!    evidencia. Este segundo punto es el que importa: un guard que rechaza y
//!    deja el log poisoned no ha arreglado nada.
//! 3. La forma del overflow de producción (gap en el allocator) sigue
//!    aceptada — el guard no puede cerrar el camino bueno.
//! 4. La severidad queda documentada de forma permanente: el segmento que la
//!    escritura actual produciría es rejected por `build_replay_plan`.

use std::num::NonZeroUsize;
use std::path::PathBuf;

use chronos_log::{
    build_replay_plan, write_segment, EventSeq, ExecutionKind, ExecutionLogBackend,
    ExecutionPayload, ExecutionRecord, Gap, GapReason, InMemoryExecutionLog, LogError,
    NewExecutionRecord, SegmentEntry, SegmentedConfig, SegmentedExecutionLog, SessionId,
};

fn tmpdir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "gap-overlap-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn rec(session: &SessionId, seq: u64) -> NewExecutionRecord {
    NewExecutionRecord {
        session_id: session.clone(),
        kind: ExecutionKind::Raw,
        monotonic_ns: seq,
        payload: ExecutionPayload::new(vec![seq as u8], "ev"),
        invocation_id: None,
        parent_invocation_id: None,
        symbol_id: None,
        captured_at_unix_ns: None,
    }
}

fn stored_rec(session: &SessionId, seq: u64) -> ExecutionRecord {
    ExecutionRecord {
        session_id: session.clone(),
        seq: EventSeq::new(seq),
        monotonic_ns: seq,
        kind: ExecutionKind::Raw,
        payload: ExecutionPayload::new(vec![seq as u8], "ev"),
        invocation_id: None,
        parent_invocation_id: None,
        symbol_id: None,
        captured_at_unix_ns: None,
    }
}

fn gap(first: u64, last: u64, source: &str) -> Gap {
    Gap::new(
        EventSeq::new(first),
        EventSeq::new(last),
        GapReason::KernelRingOverflow,
        source,
    )
}

/// Assert the failure is the overlap refusal specifically, so a test cannot
/// pass because some *other* guard happened to fire.
fn assert_overlap_refused(result: Result<(), LogError>, what: &str) {
    match result {
        Ok(()) => panic!("{what}: record_gap accepted a gap that overlaps written evidence"),
        Err(LogError::InvalidGap { reason }) => assert!(
            reason.contains("overlap"),
            "{what}: expected an OVERLAP refusal, got an unrelated InvalidGap: {reason}"
        ),
        Err(other) => panic!("{what}: expected InvalidGap, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// 1. The boundary refuses, before anything is written.
// ---------------------------------------------------------------------------

#[test]
fn an_overlapping_gap_is_refused_before_it_is_written() {
    let log = InMemoryExecutionLog::new();
    let s = SessionId::new("overlap-inmemory");
    for i in 0..10 {
        log.append(rec(&s, i)).expect("append");
    }
    assert_eq!(
        log.entry_count(&s),
        10,
        "the session starts with 10 entries"
    );

    assert_overlap_refused(
        log.record_gap(s.clone(), gap(3, 5, "late")),
        "retroactive gap",
    );

    assert_eq!(
        log.entry_count(&s),
        10,
        "the refused gap must leave no entry behind: rejection happens at the boundary"
    );
    let page = log.read_from_seq(&s, EventSeq::ZERO, 100).expect("read");
    assert!(
        page.gaps.is_empty(),
        "no gap may be observable after a refusal: {:?}",
        page.gaps
    );
    assert_eq!(
        page.records.len(),
        10,
        "all ten records survive the refusal"
    );
}

/// A gap that starts *after* the last written seq and swallows it, the worst
/// shape: it both contradicts existing records and swallows seqs that were
/// never allocated.
#[test]
fn a_gap_that_straddles_the_head_of_the_record_run_is_refused() {
    let log = InMemoryExecutionLog::new();
    let s = SessionId::new("overlap-straddle");
    for i in 0..10 {
        log.append(rec(&s, i)).expect("append");
    }
    assert_overlap_refused(
        log.record_gap(s.clone(), gap(5, 12, "straddle")),
        "gap straddling the head",
    );
    assert_eq!(log.entry_count(&s), 10);
}

/// A gap over a *previous gap* is the same defect: it revisits an occupied
/// range, so the segment cannot tile.
#[test]
fn a_gap_overlapping_a_previous_gap_is_refused() {
    let log = InMemoryExecutionLog::new();
    let s = SessionId::new("overlap-gap-on-gap");
    for i in 0..10 {
        log.append(rec(&s, i)).expect("append");
    }
    log.record_gap(s.clone(), gap(10, 12, "first"))
        .expect("the adjacent gap is legitimate and must still be accepted");
    assert_overlap_refused(
        log.record_gap(s.clone(), gap(11, 11, "second")),
        "gap over a gap",
    );
    assert_eq!(log.entry_count(&s), 11, "only the legitimate gap is stored");
}

// ---------------------------------------------------------------------------
// 2. The property that matters most: the log is still reopenable.
// ---------------------------------------------------------------------------

#[test]
fn the_log_stays_reopenable_after_an_overlapping_gap_is_refused() {
    let dir = tmpdir("reopen");
    let session = SessionId::new("overlap-reopen");
    let mut cfg = SegmentedConfig::with_dir(&dir);
    cfg.flush_threshold = NonZeroUsize::new(4096).expect("non-zero");

    {
        let log = SegmentedExecutionLog::open(session.clone(), cfg.clone()).expect("open");
        for i in 0..10 {
            log.append(rec(&session, i)).expect("append");
        }
        assert_overlap_refused(
            log.record_gap(gap(3, 5, "late")),
            "segmented overlapping gap",
        );
        // A refusal must not have poisoned the segment buffer either, so the
        // flush has to succeed and contain exactly the ten records.
        log.flush().expect("flush after the refusal");
    }

    // The decisive assertion. Before the fix this is where the log died:
    // the segment the accepted gap produced is rejected by the strict
    // validator and `open` never returns a handle.
    let log = SegmentedExecutionLog::open(session.clone(), cfg).expect("reopen after the refusal");
    let page = log
        .read_from_seq(EventSeq::ZERO, 100)
        .expect("read after reopen");
    let seqs: Vec<u64> = page.records.iter().map(|r| r.seq.0).collect();
    assert_eq!(
        seqs,
        (0..10).collect::<Vec<_>>(),
        "every record survives the reopen"
    );
    assert!(
        page.gaps.is_empty(),
        "the refused gap must not be resurrected by the reopen: {:?}",
        page.gaps
    );
}

/// Same property through the overflow path's own configuration, so the guard is
/// pinned where the eBPF ring overflow actually lands.
#[test]
fn the_overflow_path_keeps_working_under_the_guard() {
    let log = InMemoryExecutionLog::new();
    let s = SessionId::new("overflow-shape");
    for i in 0..5 {
        log.append(rec(&s, i)).expect("append");
    }
    // Exactly `SegmentedExecutionLog::append_inner` case 5: reserve the seq,
    // then declare that single seq lost.
    let reserved = log.allocate_seq_for_gap(&s).expect("reserve");
    assert_eq!(reserved, EventSeq::new(5));
    log.record_gap(s.clone(), gap(5, 5, "overflow"))
        .expect("the production overflow shape must remain accepted");
    let next = log.append(rec(&s, 99)).expect("append after the gap");
    assert_eq!(
        next,
        EventSeq::new(6),
        "the allocator advanced past the gap"
    );
}

// ---------------------------------------------------------------------------
// 3. Severity, pinned permanently: the segment the old write produced is
//    unreopenable. These two controls are what refute the "retroactive but
//    non-overlapping gap is legitimate" reading of the brief.
// ---------------------------------------------------------------------------

#[test]
fn control_the_segment_an_accepted_record_overlap_would_have_produced_is_unreopenable() {
    let dir = tmpdir("control-record-overlap");
    let session = SessionId::new("control-record-overlap");
    // Records 0..=4, then a gap 1..=3 — exactly what today's `record_gap`
    // accepts and flushes.
    let entries = vec![
        SegmentEntry::Record(stored_rec(&session, 0)),
        SegmentEntry::Record(stored_rec(&session, 1)),
        SegmentEntry::Record(stored_rec(&session, 2)),
        SegmentEntry::Record(stored_rec(&session, 3)),
        SegmentEntry::Record(stored_rec(&session, 4)),
        SegmentEntry::Gap(gap(1, 3, "late")),
    ];
    write_segment(
        &dir,
        &session,
        EventSeq::ZERO,
        EventSeq::new(4),
        entries.len() as u64,
        &entries,
    )
    .expect("write the segment the old behaviour produced");

    let err = build_replay_plan(&dir, &session, EventSeq::ZERO)
        .expect_err("an overlapping gap makes the log unreopenable");
    assert!(
        matches!(
            err,
            chronos_log::ReplayIntegrityError::PayloadRangeMismatch { .. }
        ),
        "expected PayloadRangeMismatch, got {err:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn control_a_retroactive_gap_is_also_unreopenable_so_it_is_not_a_legitimate_category() {
    let dir = tmpdir("control-retroactive");
    let session = SessionId::new("control-retroactive");
    // The shape `memory::c1_c2_read_from_seq` uses: 10 records 0..=9, then a
    // "retroactive" gap 3..=5 that starts below the allocator and ends below
    // the last record. It is accepted today. It is ALSO unreopenable.
    let mut entries: Vec<SegmentEntry> = (0..10)
        .map(|s| SegmentEntry::Record(stored_rec(&session, s)))
        .collect();
    entries.push(SegmentEntry::Gap(gap(3, 5, "late")));
    write_segment(
        &dir,
        &session,
        EventSeq::ZERO,
        EventSeq::new(9),
        entries.len() as u64,
        &entries,
    )
    .expect("write the segment the old behaviour produced");

    let err = build_replay_plan(&dir, &session, EventSeq::ZERO)
        .expect_err("a retroactive gap is unreopenable too, so it is not legitimate");
    assert!(
        matches!(
            err,
            chronos_log::ReplayIntegrityError::PayloadRangeMismatch { .. }
        ),
        "expected PayloadRangeMismatch, got {err:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
