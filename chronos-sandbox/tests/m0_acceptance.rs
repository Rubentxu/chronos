//! M0 acceptance harness scaffold.
//!
//! Each `#[test]` is a stub for one M0 ticket. Implementation lands in
//! follow-on cycles (m0-01, m0-02, …, m0-10) per the reconstruction
//! backlog at `docs/chronos-agentic-reconstruction/docs/roadmap/IMPLEMENTATION_BACKLOG.md`.
//!
//! DoD for THIS file: it compiles cleanly as part of
//! `cargo test -p chronos-sandbox --test m0_acceptance --no-run`.
//!
//! Each stub is `#[ignore]` with `reason = "implemented in cycle m0-NN"`,
//! so the default sandbox test run remains green and only `--include-ignored`
//! reveals the unimplemented bodies.

#[test]
#[ignore = "implemented in cycle m0-01"]
fn m0_01_live_pagination_is_non_destructive() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-01.md");
}

#[test]
#[ignore = "implemented in cycle m0-02"]
fn m0_02_session_snapshot_is_cumulative() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-02.md");
}

#[test]
#[ignore = "implemented in cycle m0-03"]
fn m0_03_ebpf_probe_lifetime_is_session_owned() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-03.md");
}

#[test]
#[ignore = "implemented in cycle m0-04"]
fn m0_04_tripwires_evaluated_on_canonical_flow() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-04.md");
}

#[test]
#[ignore = "implemented in cycle m0-05"]
fn m0_05_typed_event_filters_reject_unknown() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-05.md");
}

#[test]
#[ignore = "implemented in cycle m0-06"]
fn m0_06_state_diff_preserves_register_evidence() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-06.md");
}

#[test]
#[ignore = "implemented in cycle m0-07"]
fn m0_07_query_returns_not_found_when_target_missing() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-07.md");
}

#[test]
#[ignore = "implemented in cycle m0-08"]
fn m0_08_timestamp_contract_is_separated() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-08.md");
}

#[test]
#[ignore = "implemented in cycle m0-09"]
fn m0_09_race_heuristic_is_renamed() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-09.md");
}

#[test]
#[ignore = "implemented in cycle m0-10"]
fn m0_10_sandbox_m0_acceptance_suite_passes() {
    unimplemented!("See vault/cycles/m0-truth-first-foundation/m0-10.md");
}
