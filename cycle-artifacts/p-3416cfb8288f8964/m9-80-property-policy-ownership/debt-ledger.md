# Debt Ledger — m9-80-property-policy-ownership

Assessed 2026-09-29 during reconciliation of the abandoned PAUSED cycle.
Surface: cycle diff `82e219f..ccf8811` (18 files, +1000/-450).

| ID | Severity | Priority | Location | Observation |
|---|---|---|---|---|
| m980-D1 | medium | P3 | `crates/chronos-domain/src/property.rs:245-605` | `compare_ord` is a single 361-line function — 7x the 51-line runner-up. Six public domain fns delegate into it. |
| m980-D2 | low | P4 | `crates/chronos-domain/src/property.rs:432` | One production `unwrap()` in 779 lines (`observations.last().cloned().unwrap()`). The other 12 unwraps are confined to `cfg(test)`. |

## Ratchet

No regression attributable to this cycle. It **reduced**
`chronos-services/hypothesis_test.rs` by 239 lines and moved policy ownership
into the domain layer, which is the intended direction. Both findings are
pre-existing shape in code this cycle relocated, not debt it introduced.

## Rationale for not blocking

964 tests pass, fmt and clippy `-D warnings` are clean, and `compare_ord`'s
match arms are flat and total. m980-D1 is a maintainability cost, not a
correctness risk, so it is deferred to a follow-up extraction cycle rather than
holding a shipped, tagged release (`v0.7.82`) open.
