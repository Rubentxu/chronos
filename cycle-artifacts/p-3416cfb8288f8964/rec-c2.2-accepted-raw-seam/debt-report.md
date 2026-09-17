# Debt Report: rec-c2.2-accepted-raw-seam

- **Report ID:** `debt-rec-c2.2-accepted-raw-seam-r0`
- **Contract:** `debt-gate/v1`
- **Generated:** 2026-09-17T19:57:32Z
- **Cycle:** `rec-c2.2-accepted-raw-seam` (path A-lite, remediation round 0)
- **Subject SHA:** `1619fb250d94ebcad1730942bb8a309c0fe9049f`
- **Base SHA:** `e4fd938c528a42c85228e6a156bafe1f557045a7`
- **Diff digest:** `a182963b1086eba8860f23f7b96069352150c134bedcedc7e99176448a88263f`
- **Verify report evidence:** `cycle-artifacts/p-3416cfb8288f8964/rec-c2.2-accepted-raw-seam/verify-report.md` (sha256 `69f4567cf6d21b45a2029de4e003f6a7f5d4e03e8205886cb77a3a1bbe623317`), verdict PASS, subject rebound from `585dfc45` to `1619fb25`; this gate independently audited the `585dfc45..1619fb25` delta at head `1619fb25`.
- **Clusters required/completed:** coupling, duplication, overeng, smells — all completed, none failed.

## Verdict: PASS_WITH_WARNINGS

Zero introduced HIGH/CRITICAL findings. Seven unsuppressed introduced LOW findings
carry no blocker; the gate stays in the warning band so the duplication and
dead-trait-method follow-ups remain attached to the cycle record instead of
aging silently. The single MEDIUM finding is `attribution: pre_existing`,
visible and owned by tasks.md `FIND-C2.2-04`; per policy it is not emitted as
an INC because its durable record already exists as a named, owned task
finding.

## Summary

| Dimension | Value |
|---|---|
| Total findings | 8 |
| By severity | 0 critical / 0 high / 1 medium / 7 low |
| By confidence | 7 high / 1 medium |
| By attribution | 7 introduced / 1 pre-existing |
| By cluster | 2 coupling / 2 duplication / 2 overeng / 2 smells |
| INCs emitted | none (no suppression, no incident-grade introduced debt) |

## Findings

### FIND-DEBT-000001 — coupling (LOW, P1, introduced) — fingerprint `4c1e0942…f95a17`

**ProbeBackend::read_since has no production caller after the switchover; its contract varies by backend.**
After this cycle, the canonical drain path never calls the trait method. The
only non-test caller is `NativeProbeBackend::read_since`
(`crates/chronos-native/src/probe_backend.rs:1153`, itself test-only), while
`EbpfAdapter::read_since` now refuses unconditionally
(`crates/chronos-ebpf/src/lib.rs:218-246`) and `BrowserAdapter::read_since`
still serves a bounded-buffer snapshot. An interface production no longer
exercises, whose implementors must keep diverging contract behavior.

- **Impact:** comprehension cost plus a latent mis-wiring risk for the next implementor; not a current defect.
- **Remediation (backlog):** in a follow-up cycle, remove `read_since` from `ProbeBackend` (folding the live-transport read into the EventBus API) or split the trait so non-peekable backends do not implement a refusal.
- **Evidence:** `grep -rn '\.read_since('` returns only test call sites plus the two impls above; sed of `adapter.rs:31-40` shows the trait signature with no production dispatch.

### FIND-DEBT-000002 — coupling (LOW, P1, introduced) — fingerprint `9d3b7a15…e5246`

**ProbeService::drain derives the scan budget from offset+limit, coupling wire pagination to evidence examination.**
`crates/chronos-services/src/probe.rs:590-601` passes
`input.offset.saturating_add(input.limit).max(1)` as `max_raw_events`; the
cursor advances past every examined record while only the offset/limit window
is returned (`crates/chronos-mcp/src/server.rs:5198-5206`). An offset-paging
client re-examines and re-projects the prefix on every page, while cursor
continuation without offset stays O(window).

- **Impact:** wasted CPU proportional to window growth and a hidden dependency between wire pagination and scan depth. Latent: no CI test exercises long offset-paged clients.
- **Remediation (backlog):** serve offset from the already-examined page (server-side slice of the same scan) or deprecate offset windows in favor of cursor continuation; document `max_raw_events` semantics.
- **Evidence:** sed of `probe.rs:584-611`; sandbox suites `probe_drain_canonical` (4/4) and `rec_c2_2_uat_c2` (3/3) pass and cover cursor continuation only.

### FIND-DEBT-000003 — duplication (LOW, P2, introduced) — fingerprint `2f8a4c6e…83942`

**Fail-closed decode + typed error idiom repeated at five call sites across three readers (two sites added by this cycle).**
`decode(record).ok_or_else(|| ServiceError::EvidenceDecodeFailed { .. })?`
appears at `events_log_read.rs:412`, `events_log_read.rs:602`, and
`projection.rs:139` (all pre-existing), plus the two new sites
`canonical_drain.rs:92` (`read_all_raw_events`) and `canonical_drain.rs:242`
(`read_canonical_drain_page`). The two new sites are verbatim copies of the
pre-existing idiom.

- **Impact:** a change to the decode-failure contract (e.g. a decode-metrics counter) must be edited in five places; a missed site silently diverges the fail-closed policy between readers.
- **Remediation (backlog):** extract `decode_raw_fail_closed(log, record) -> Result<TraceEvent, ServiceError>` next to `decode` in events_log_read.rs; adopt at all five sites. Sketch: takes the log handle for `session_id`, the raw record for `seq`/`payload_tag`, returns the decoded event; `?`-compatible with all five call sites.
- **Evidence:** `grep -rn 'decode(record).ok_or_else' crates/chronos-services/src/` matches exactly the five sites above; `git diff e4fd938c..1619fb25 --name-only` shows the cycle touched canonical_drain.rs but not events_log_read.rs/projection.rs, so only the two new instances are introduced.

### FIND-DEBT-000004 — overeng (LOW, P2, introduced) — fingerprint `6b2e8d1f…46c9d3`

**AUDIT VERDICT, NOT A DEFECT: ProbeService::drain dependency set judged proportionate (orchestrator packet Q2).**
Each dependency has one distinct, observable job: `SessionExecutionLog` is the
durable authority; `EventsCursorV1` decodes the session-bound resumable
position with a typed wrong-session error; `canonical_drain` supplies the scan
law (fail-closed decode, cluster-atomic cursor, completeness); the resolver
pipeline projects Raw into the producer's semantic view (proven bus-free by
`c2_2_projecting_semantics_reads_no_bus`); the `project` closure
(`probe.rs:584-588`) captures the cloned pipeline for the page callback and
adds nothing. Five small collaborators, one reason each; removing any would put
its job back inline. 69 lines total, no mock/strategy/factory layering, lock
taken and dropped (`probe.rs:568-582`) before per-event projection.

- **Impact:** none as implemented; recorded so future additions to `drain` are compared against this baseline.
- **Remediation:** none; re-judge only if a second projection path or a manager-style parameter appears.
- **Evidence:** sed `probe.rs:543-611`; `cargo test -p chronos-native --lib -- --test-threads=1` green (107 passed) at `1619fb25`.

### FIND-DEBT-000005 — duplication (LOW, P2, introduced) — fingerprint `a17d3f62…361a8`

**AUDIT VERDICT, NOT A DEFECT: sibling readers not extracted; extraction would be a shallow abstraction (orchestrator packet Q1).**
`read_all_raw_events` (`canonical_drain.rs:63-125`) and
`read_canonical_drain_page` (`canonical_drain.rs:163-308`) share only surface
shape. They disagree on every decision: continuation model (implicit
`retained_from..tail` vs cursor-resumable with budget stops at `:207/:227`),
termination (`:82-84` vs `:224-229`), cluster handling (`TripwireFired`
ignored at `:107` vs counted with per-source cap and typed refusal at
`:249-259`), boundary validation (absent vs head-read refusal at `:178-194`),
stall detection (absent vs `:273-278`), page size (1024 vs 512 at `:73/:212`),
and output type. A parameterized shared scan needs ≥4 behavioral flags — the
boolean-condensation shape Ousterhout warns about. The honestly-shared code is
the `CompletenessReport` construction (`:114-122` vs `:288-296`) and the
decode idiom (tracked as FIND-DEBT-000003).

- **Impact:** none today; revisit only if a third reader with the same shape appears.
- **Remediation:** no extraction of the scan loops. If a third sibling appears, extract only the completeness construction plus the decode idiom.
- **Evidence:** line-anchored comparison above; both readers carry independent law tests (DRAIN-2..7, STOP-1..4, transport-falsification), all green in `cargo test -p chronos-services --lib` (370 passed) at `1619fb25`.

### FIND-DEBT-000006 — smells (LOW, P3, introduced) — fingerprint `e84b1c95…0b4957`

**`legacy_cursor` wire field is written only as null and asserted only as none.**
The server always emits `Null` (`crates/chronos-mcp/src/server.rs:5210`), the
sandbox DTO types it `Option<serde_json::Value>`
(`chronos-sandbox/src/client/types.rs:257`), and the only consumers are two
negative assertions (`chronos-sandbox/tests/m0_acceptance.rs:128`,
`chronos-sandbox/tests/probe_drain_canonical.rs:68`). No old client is known
to read it; the field only documents that the old coordinate space is gone.

- **Impact:** one redundant wire field to carry or deliberately drop later; no behavior risk.
- **Remediation (backlog):** drop `legacy_cursor` at the next wire revision or state an expiry in its documentation; the null-only invariant is meanwhile pinned by test.
- **Evidence:** `grep -rn legacy_cursor` returns exactly the five sites above; no code path populates it.

### FIND-DEBT-000007 — smells (MEDIUM, P2, pre-existing, visible/owned) — fingerprint `c3a59d17…5e2f3`

**BrowserAdapter::take_semantic_events remains a destructive read, relocated off-trait and exempted by FIND-C2.2-04.**
`s.event_buffer.drain(..).collect()` persists at
`crates/chronos-browser/src/adapter.rs:371`, identical in
`e4fd938c:crates/chronos-browser/src/adapter.rs` — the destructiveness
pre-dates the cycle. The cycle relocated the method off `ProbeBackend`,
documented the eviction at the definition and at the
`browser_probe.rs:238-242` call site, and switched the stop path to the
non-destructive `raw_events()` (`browser_probe.rs:217`).

- **Impact:** browser-local drain still consumes a bounded buffer; evidence older than capacity is silently absent and no completeness model exists on this path until FIND-C2.2-04 lands.
- **Remediation:** owned by `tasks.md FIND-C2.2-04` (next cycle): wire browser capture through the accepted-Raw seam, then retire `take_semantic_events`.
- **Not emitted as INC:** the durable record already exists as a named, owned task finding with a recorded exemption justification.

### FIND-DEBT-000008 — overeng (LOW, P2, introduced, baseline updated) — fingerprint `b71f2d4a…b6f15`

**EbpfAdapter::read_since destructive-fallback removal is enforced by construction, not by a running test.**
Commit `1619fb25` replaced the destructive `drain_events()` fallback with a
typed refusal (`Err(TraceError::CursorStale { expected: 0, current: 0 })`,
`crates/chronos-ebpf/src/lib.rs:218-246`) and documents why no runtime test
pins it: constructing a real adapter needs CAP_BPF and kernel ≥5.8, and
feature-off construction returns `Err(EbpfError::Unavailable)` before the
refusal branch is reachable. The property is enforced by source construction;
`MockEbpfAdapter` covers the non-destructive read law runnably
(`ebpf_integration.rs:36-63`: second read sees the same events).

- **Impact:** the refusal property could theoretically regress on a future edit without a failing test; the source comment reduces detection to a review glance.
- **Remediation (backlog, optional):** a source-level ratchet asserting no `ProbeBackend` impl in chronos-ebpf calls `drain_events()`; or accept the by-construction guarantee as final.
- **Evidence:** `cargo test -p chronos-ebpf` green (30 lib + 3 integration passed, 1 ignored root-gated) at `1619fb25`; method body is a single `Err(..)`.

## Marker sweep

`grep -nE 'TODO|FIXME|ponytail:'` over the full cycle diff (`e4fd938c..1619fb25`)
returns no match. Zero marker-driven findings.

## Follow-up ledger

| Priority | Action | Finding |
|---|---|---|
| P1 | Decide `ProbeBackend::read_since` fate: remove from trait or split trait | FIND-DEBT-000001 |
| P1 | Serve offset from the examined page or deprecate offset windows; document `max_raw_events` | FIND-DEBT-000002 |
| P2 | Extract `decode_raw_fail_closed`; adopt at all five sites | FIND-DEBT-000003 |
| P2 | Own under FIND-C2.2-04: browser capture via accepted-Raw seam; retire `take_semantic_events` | FIND-DEBT-000007 |
| P3 | Drop `legacy_cursor` at next wire revision or document expiry | FIND-DEBT-000006 |
| P3 | Optional source-level ratchet for ebpf `read_since` refusal | FIND-DEBT-000008 |

## Gate evidence at subject 1619fb25

| Gate | Command | Result |
|---|---|---|
| fmt | `cargo fmt --all -- --check` | clean |
| services lib | `cargo test -p chronos-services --lib` | 370 passed, 0 failed |
| ebpf lib+integration | `cargo test -p chronos-ebpf` | 30 + 3 passed, 1 ignored (root-gated) |
| browser lib | `cargo test -p chronos-browser --lib` | 44 passed, 1 ignored |
| native lib (serial, per repo rule) | `cargo test -p chronos-native --lib -- --test-threads=1` | 107 passed, 0 failed |

`CHRONOS_MCP_PATH` resolved to a pre-built binary; no full sandbox run (T5) and
no chronos-e2e were executed, per gate scope.
