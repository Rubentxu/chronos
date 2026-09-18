# Debt ledger — rec-c3-3-1-identity-storage-seam

This ledger captures the hexagonal/identity/storage debt items relevant
to C3.3.1 close. Severity is "low" across the board (no production
incidents; all are structural refactor tasks); priority is set per the
operator's planning on what blocks the next cycle.

## CLOSED in this cycle

### C31-DEBT-01 — ExecutionLogProvider real (CLOSED)

- Status: CLOSED
- Closed by: commits `3bd71bdf` (port defined), `6bc5d412` (2 adapters), `794732fa` (productive consumer).
- Definition of done (operator-approved): the application layer (`chronos_services::session_log::SessionExecutionLog`) holds an `Arc<dyn ExecutionLogProvider>` field; the trait is object-safe; the segmented and in-memory adapters implement it; `record_gap` is on the port, not on maintenance; `chronos_domain::ports::execution_log` is the single declaration point.
- Evidence:
  - `crates/chronos-domain/src/ports/execution_log.rs` defines the trait and `ExecutionLogPage`/`ExecutionLogError`/`ExecutionLogKind`.
  - `crates/chronos-log/src/segmented.rs::SegmentedExecutionLogProvider` implements the trait.
  - `crates/chronos-log/src/memory.rs::InMemoryExecutionLogProvider` implements the trait.
  - `crates/chronos-services/src/session_log.rs::SessionExecutionLog.log: Arc<dyn ExecutionLogProvider>` (commit 794732fa).
  - 378/378 chronos-services tests pass; T4 smoke passes 16/16.
- Reopen criteria: only if a future change adds a *second* canonical-evidence operation to the wrapper that belongs on the port (would mean the port was incomplete at C3.3.1 close).

### C31-DEBT-02 — SessionId single owner (CLOSED)

- Status: CLOSED
- Closed by: commit `7a6062b0`.
- Definition of done: `chronos_domain::SessionId` is the single owner of the session identity concept. `chronos_log::SessionId` is a `pub use` re-export; no separate struct.
- Evidence:
  - `crates/chronos-log/src/lib.rs` re-exports `chronos_domain::SessionId`.
  - The `chronos_domain` test `IdentityMismatch` contract kills any future "helpful adapter" that fixes SessionId silently.
- Reopen criteria: only if a future change introduces a parallel session-id type in any other crate.

## OPEN at cycle close

### C31-DEBT-03 — full services → ports inversion not finished (OPEN)

- Status: OPEN
- Severity: low. Priority: P2.
- Owner: REC-C3.3.3.
- Why it is open: `chronos-services` still consumes concrete adapters through:
  - `legacy_segmented_backend_for_native_bridge` (probe lifecycle paths: `read_log_with_stats`, `attach_execution_log`).
  - `compaction_metrics()`, `maybe_compact()`, `compact_up_to()`, `retain_up_to()`, `flush()` on `SessionExecutionLog` (all maintenance, gated by `ProviderKind`).
  - Maintenance bridge exposes the concrete `SegmentedExecutionLog` to `chronos_native`.
- What closes it: when the canonical-evidence path uses ONLY `Arc<dyn ExecutionLogProvider>` for ALL evidence operations, AND a native-port-equivalent surface replaces `legacy_segmented_backend_for_native_bridge` (or the bridge is removed).
- Reopen criteria: any new canonical-evidence operation that bypasses the port, or any new consumer of concrete `SegmentedExecutionLog` outside `chronos_log`.

### C33-DEBT-RETENTION-01 — retention policy bypasses ExecutionLogProvider (OPEN)

- Status: OPEN (registered in this cycle).
- Severity: low. Priority: P2.
- Owner: REC-C3.3.2 / REC-C3.3.3.
- Description:
  - `retain_up_to` (and its sibling `compact_up_to`) lives on `SessionExecutionLog` as a maintenance capability, gated by `ProviderKind`. It is NOT on the `ExecutionLogProvider` port.
  - This is wrong because retention is a SEMANTIC operation, not mere storage maintenance: it changes what counts as evidence, what cursors are valid, what the meaning of `retained_from` is for downstream consumers. It should live on the port (or on a dedicated retention port), so consumers can ask the question portably and adapters must commit to the semantics, not just the bytes.
  - Until it moves to the port, the contract "what is retention?" is whatever the concrete `SegmentedExecutionLog` happens to do. That is a leaky abstraction.
- Why this is a debt, not a feature: C3.3.1 deliberately stopped at field inversion. Construction inversion + retention semantics are the next cycle's job.
- Closes when: `ExecutionLogProvider::retain_up_to` (or a dedicated `RetentionPolicy` port) exists; the wrapper no longer special-cases `ProviderKind::Segmented` for retention; the in-memory adapter must answer retention honestly (and likely has to grow a retention policy of its own to comply).
- Reopen criteria: any new operation that conceptually decides "what counts as evidence" that lives on the wrapper rather than the port.

### C33-DEBT-NATIVE-LOG-BRIDGE-01 — native probe integration requires concrete segmented backend (OPEN)

- Status: OPEN (registered in this cycle).
- Severity: low. Priority: P2.
- Owner: REC-C3.3.2 / REC-C3.3.3.
- Description:
  - `chronos_native::read_log_with_stats` and the native attach path take `&SegmentedExecutionLog` directly. The only call site in `chronos_services::probe` reaches the concrete backend via `SessionExecutionLog::legacy_segmented_backend_for_native_bridge()`.
  - This is the canonical example of "leaky abstraction": the native probe backend assumes a filesystem-backed segmented log, so `chronos_services::probe` must reach past the port to hand it one.
  - The bridge is intentionally ugly-named to discourage expansion. It is allowed in `session_log.rs` and `probe.rs` ONLY. See `ratchets.md`.
- Why this is a debt, not a feature: the hexagonal rule is "services depend on ports; ports do not know about concrete adapters". The native bridge breaks that rule in one specific place. C3.3.2 / C3.3.3 must either (a) introduce a native-port-equivalent surface that the native backend implements, or (b) restructure so the native backend does not need the concrete segmented log.
- Closes when: `legacy_segmented_backend_for_native_bridge` returns the concrete `SegmentedExecutionLog` nowhere in production code; either an equivalent port exists or the call sites no longer need it.
- Reopen criteria: any new call site of `legacy_segmented_backend_for_native_bridge` outside `session_log.rs` and `probe.rs` (enforced mechanically by a grep gate in `ratchets.md`).

## GAP

### HEX-002 — hexagonal services → ports inversion not verified (GAP)

- Status: GAP (not promoted to partial/verified by enthusiasm).
- Severity: structural. Priority: P1 (blocks REC-C3 close).
- Description:
  - HEX-002 tracks the full inversion: `chronos-services → chronos-domain::ports` for ALL of storage, native, ebpf, browser, and webhook. Today only storage has the seam. Native is partially bridged. ebpf, browser, and webhook still have concrete dependencies.
  - C3.3.1 created the storage seam correctly. C3.3.1 did NOT finish the inversion. Both are simultaneously true.
- Why GAP, not PARTIAL: a partial/verified promotion would imply "in progress, on track, no action needed". The actual state is "seam exists; bridge still leaky; native / ebpf / browser / webhook edges untouched". GAP is the honest classification.
- Closes when: every edge in the four-edge map from `rec-c3-3-services-inversion/exploration-report.md` is satisfied through a port, AND `legacy_segmented_backend_for_native_bridge` is removed, AND the composition root is in `chronos-mcp::composition`.
- Reopen criteria: any attempt to call this done before all of the above.

## Mechanical checks

The ratchet for `legacy_segmented_backend_for_native_bridge` is enforced by grep, documented in `ratchets.md`:

```bash
# Acceptable occurrences: session_log.rs (definition + test) and probe.rs (call sites).
# Any occurrence outside these two files is a debt escalation.
grep -rn "legacy_segmented_backend_for_native_bridge" crates/ \
  --include="*.rs" \
  | grep -v "session_log.rs" \
  | grep -v "probe.rs"
# Expected output: empty.
```

This check is part of T0 from C3.3.2 onwards.
