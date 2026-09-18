# Handoff — rec-c3-3-1-identity-storage-seam → next cycle

This handoff is the operator-curated record of what C3.3.1 did, what
it deliberately did NOT do, and what the next cycle (REC-C3.3.2) must
and must not touch. It exists to prevent the "just one more refactor"
trap.

## What C3.3.1 did

The seam. Specifically:

1. `chronos_domain::SessionId` is the single owner of session identity.
   `chronos_log::SessionId` is a `pub use` re-export. (C31-DEBT-02 closed.)
2. `EventSeq`, `ExecutionKind`, `ExecutionPayload`, `TripwireFiredEvidence`,
   `NewExecutionRecord`, `Gap`, `GapReason`, `TailState`, `SealedTail`,
   `ExecutionRecord` lifted to `chronos-domain`. (C31-DEBT-01 partial.)
3. `ExecutionLogProvider` is a real, object-safe, session-scoped port
   in `chronos_domain::ports::execution_log`. The trait declares
   `append`, `record_gap`, `read_from_seq`, `retained_from`, `tail_seq`,
   `tail_state`, `seal`, and `session_id` (the identity accessor).
4. Two adapters implement the port: `SegmentedExecutionLogProvider`
   (filesystem-backed) and `InMemoryExecutionLogProvider` (test/CI).
   The wrappers are explicit; no blanket impls.
5. `chronos_services::session_log::SessionExecutionLog.log` is now
   `Arc<dyn ExecutionLogProvider>` (FIELD INVERSION). All canonical-
   evidence operations go through the port.
6. Maintenance capabilities (`flush`, `compaction_metrics`,
   `maybe_compact`, `compact_up_to`, `retain_up_to`) stay on the
   wrapper, gated by `ProviderKind`. They are NOT on the port.
7. `legacy_segmented_backend_for_native_bridge` is the narrow
   concrete-backend escape hatch for `probe.rs` only. Ugly-named to
   discourage expansion.
8. `map_log_error` (legacy) was removed from `events_log_read.rs` and
   `projection.rs`. `map_execution_log_error` (canonical-evidence) is
   the only path.

## What C3.3.1 deliberately did NOT do

1. **Composition root inversion**. `chronos-mcp::server.rs` still
   constructs `NativeProbeBackend::new`, `BrowserAdapter::new`, etc.
   That is C3.3.2's job.
2. **ebpf / browser / webhook ports**. Only storage has the seam. The
   four-edge map from `rec-c3-3-services-inversion/exploration-report.md`
   is still mostly concrete. That is C3.3.3's job.
3. **Retention on the port**. `retain_up_to` is maintenance-gated,
   not port-level. Retention semantics are still whatever the concrete
   segmented log happens to do. That is C33-DEBT-RETENTION-01.
4. **Removing `legacy_segmented_backend_for_native_bridge`**. It is
   required for `probe.rs` to talk to `chronos_native`. That is
   C33-DEBT-NATIVE-LOG-BRIDGE-01.
5. **Closing HEX-002**. The seam exists; the inversion is incomplete.
   HEX-002 closes only after C3.3.3.
6. **Version bump / new tag**. No behavior-bumping API surface change
   for end users; the workspace version stays at `0.1.1`.

## DO NOT (ratchet; operator-curated)

These are forbidden for the next cycle and any future cycle unless a
new ADR explicitly revokes them:

```text
DO NOT:
- move record_gap back to maintenance
- reintroduce LogPage into services
- expose SegmentedExecutionLog for canonical evidence operations
- add flush/compaction to ExecutionLogProvider
- move composition root into chronos-services
```

Expanded explanation:

- **"move record_gap back to maintenance"** — `record_gap` is on the
  port, full stop. It is canonical-evidence. If a future cycle needs
  a "legacy gap" path for some reason, it must be a SEPARATE method
  (e.g. `record_gap_on_segmented`) explicitly named "legacy", NOT a
  reclassification of `record_gap`.
- **"reintroduce LogPage into services"** — `ExecutionLogPage` is the
  canonical-evidence page type, lives in `chronos-domain::ports::execution_log`.
  `LogPage` is the legacy segmented-internal page type, lives in
  `chronos-log`. Services consume ONLY `ExecutionLogPage`. Any
  reintroduction of `LogPage` outside `chronos-log` is a regression.
- **"expose SegmentedExecutionLog for canonical evidence operations"** —
  the ONLY way services reach storage for canonical evidence is
  `Arc<dyn ExecutionLogProvider>`. The concrete `SegmentedExecutionLog`
  may be reached ONLY via `legacy_segmented_backend_for_native_bridge`,
  and ONLY for maintenance-adjacent native code, and ONLY in
  `session_log.rs` and `probe.rs`. See `ratchets.md` R1.
- **"add flush/compaction to ExecutionLogProvider"** — the port stays
  minimal. Maintenance operations belong on the wrapper, gated by
  `ProviderKind`. Adding them to the port would leak filesystem into
  the domain contract. See `ratchets.md` R3.
- **"move composition root into chronos-services"** — composition
  belongs in `chronos-mcp::composition`, NOT in
  `chronos-services::composition`. `chronos-services` does not own
  adapter construction; it consumes ports. See `ratchets.md` R6.

## What the next cycle (REC-C3.3.2) should do

Per the operator's framing at C3.3.1 close:

```text
REC-C3.3.2 — composition/integration inversion
center: pull concrete adapter construction out of chronos-services
        reduce/eliminate legacy_segmented_backend_for_native_bridge
        define chronos-mcp::composition
```

Concretely:

1. Create `chronos-mcp::composition` module. Move every `NativeProbeBackend::new`,
   `BrowserAdapter::new`, `EbpfAdapter::new`, `SessionStore::try_open`,
   etc. construction from `chronos-mcp::server.rs` into the composition
   module.
2. Replace `Arc<NativeProbeBackend>` direct fields in
   `chronos_services::probe` with `Arc<dyn NativeProbe>` (a new port)
   OR with `Arc<dyn ExecutionLogProvider>` for the log surface.
3. Decide on the native integration's port surface. Either:
   - (a) introduce a `NativeProbe` port that takes a `&dyn ExecutionLogProvider`
     where the bridge is currently used; OR
   - (b) restructure the native probe to read through the segmented
     log via its port surface.
4. Run the ratchet grep for `legacy_segmented_backend_for_native_bridge`
   occurrences outside `session_log.rs` and `probe.rs`. Expected:
   empty.
5. Run the ratchet grep for `map_log_error` occurrences outside
   `chronos-log` and `session_log.rs`. Expected: empty (or only
   tests with documented migration TODOs).
6. Document the composition-root module in an ADR.

## What the cycle AFTER that (REC-C3.3.3) should do

Per the operator's framing:

```text
REC-C3.3.3 — full services → ports migration
center: ebpf / browser / webhook ports
        close C31-DEBT-03
        close HEX-002
```

Concretely:

1. Introduce ports for ebpf, browser, and webhook adapters (or fold
   them under existing ports where they fit).
2. Migrate services consumers to the new ports.
3. Add retention semantics to the `ExecutionLogProvider` port (or to
   a dedicated `RetentionPolicy` port). Close C33-DEBT-RETENTION-01.
4. Promote HEX-002 from GAP to CLOSED.

## Carry-forward debt (state at C3.3.1 close)

```text
C31-DEBT-01  CLOSED
C31-DEBT-02  CLOSED
C31-DEBT-03  OPEN     owner: REC-C3.3.3
C33-DEBT-RETENTION-01  OPEN     owner: REC-C3.3.2/REC-C3.3.3
C33-DEBT-NATIVE-LOG-BRIDGE-01  OPEN     owner: REC-C3.3.2/REC-C3.3.3
HEX-002       GAP      closes only after C3.3.3
```

## Commits since cycle base `147c1019`

```text
794732fa feat(c3.3.1): SessionExecutionLog consumes Arc<dyn ExecutionLogProvider>
5ca9aeb6 feat(c3.3.1): record_gap is canonical-evidence, port lifted
6bc5d412 feat(log): session-scoped ExecutionLogProvider adapters + literal LogPage mapping
3bd71bdf feat(domain): define ExecutionLogProvider port (object-safe, session-scoped)
4d165a93 refactor(domain,log): lift NewExecutionRecord/Gap/GapReason/TailState/SealedTail/ExecutionRecord to domain (single owner)
0e9aa473 refactor(domain,log,services): lift ExecutionKind/ExecutionPayload/TripwireFiredEvidence to domain (single owner)
9d751759 refactor(domain,log): lift EventSeq to domain (single owner)
7a6062b0 refactor(domain,log): single SessionId owner; domain owns identity
```

## STOP marker

This is the C3.3.1 close. No further functional work is authorized in
this cycle. Any subsequent code changes require a new cycle.
