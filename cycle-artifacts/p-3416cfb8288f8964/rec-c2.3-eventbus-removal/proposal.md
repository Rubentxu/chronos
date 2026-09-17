# REC-C2.3 — EventBus removal

## Why now

REC-C2.0 inventoried 28 production uses of the live EventBus / legacy-queue
plumbing. REC-C2.1 (tripwire evidence) and REC-C2.2 (accepted-Raw seam) have
removed every **CANONICAL destructive read** of that plumbing — the canonical
flow now reads from `ExecutionLog`. What remains under
`scripts/check_legacy_evb.py` is 21 **COMPATIBILITY** uses:

- 20 type references: `EventBus`, `EventBusHandle`, `shared_bus`,
  `BusMetrics`, `CursorStatus`, `EventCursor`, `ReadResult`.
- 1 `read_since` call (the live mirror the browser/ebpf adapters still take).

None of those 21 sites change observable output any more: every production
flow that reaches them is now an authoritative reader of `ExecutionLog`.
The ratchet (`--strict`) passed with `CANONICAL=0` at the close of C2.2.

That is the invariant REC-C2 set out to establish, and it holds. What is
left is the **structural residue** — a second type that still pretends to
be the live source of truth while in fact being an opportunistic mirror
populated only after `accept_raw` persists the canonical evidence.

## Scope

`REC-C2.3` retires the `EventBus` module entirely, along with every caller
that still names the type. After this cycle, the term `EventBus` no longer
appears in any production source file under `crates/`. The
`scripts/check_legacy_evb.py` ratchet must reach `total tracked: 0` and
`scripts/check_legacy_evb.py --strict` must also pass.

## Out of scope

- `BrowserAdapter::take_semantic_events` is **still destructive** because the
  browser path has no `ExecutionLog` (FIND-C2.2-04, named). Closing that
  gap is `rec-c2.4` (browser persistence), not C2.3.
- `ProbeBackend::read_since` (the trait method that delegates to
  `EventBus::read_since`): the trait method **goes away** in this cycle
  because its only implementor was the live-mirror path. The durable
  reader lives at `chronos-services::canonical_drain::read_events_log_page`.
- CC#18 (browser destructive read) and CC#39 (browser has no execution
  log) — gov-cycle follow-ups, unchanged by this work.

## Plan

| Step | What | Why this commit |
|---|---|---|
| C2.3.0 | `ProbeService` (services) stops holding an `EventBusHandle` | services is the canonical authority and no longer needs the mirror |
| C2.3.1 | `PtraceProbeBackend` (native) stops taking an `EventBusHandle`; the `event_bus` field is gone | native's only job is `accept_raw`; the mirror is no longer a producer |
| C2.3.2 | `BrowserAdapter::take_semantic_events` becomes `raw_events` (already non-destructive); `EbpfAdapter::read_since` is removed entirely | browser needs the live view (still destructive) but no longer pretends to be canonical; ebpf path had no production caller |
| C2.3.3 | Delete `crates/chronos-domain/src/bus.rs` and the re-exports in `chronos-domain/src/lib.rs` | the module is now unused |
| C2.3.4 | Test cleanup: tests that used `EventBus::new_shared()` only for setup are rewritten to use the canonical seam | vacuous setup is no longer load-bearing |
| C2.3.5 | Regenerate `legacy-evb-inventory.json` to `tracked=0` and re-run the ratchet | honest final state, not a deletion in the source |

## Acceptance

- `cargo test --workspace --lib --tests --exclude chronos-sandbox --exclude chronos-e2e`
  exits 0; `chronos-native --lib --test-threads=1` exits 0.
- `python3 scripts/check_legacy_evb.py` reports `total tracked: 0`,
  `CANONICAL=0`.
- `python3 scripts/check_legacy_evb.py --strict` exits 0.
- `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
  clean.
- 3 sandbox suites covering `probe_drain`, `program_scenarios`, and
  `e2e_connectivity` pass against a freshly built `chronos-mcp`.
- `grep -RIn EventBus crates/` returns nothing.

## Risk

The 21 sites to remove are concentrated in three files; the cycle is
mechanical but touches test setup. The biggest risk is silent behavior
change in tests that previously relied on EventBus-only delivery to observe
"what the probe saw live". Mitigation: tests are rewritten to construct
canonical evidence through `accept_raw` (which is what the production path
now does), so a passing test means a property of the canonical flow, not
of the mirror.
