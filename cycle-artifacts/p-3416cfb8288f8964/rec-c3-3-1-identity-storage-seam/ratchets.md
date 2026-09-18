# Ratchets — rec-c3-3-1-identity-storage-seam

Mechanical and contractual ratchets that pin the seams C3.3.1 created.
Violating a ratchet is a debt-escalation event, not a style preference.

## R1 — `legacy_segmented_backend_for_native_bridge` placement

The function `SessionExecutionLog::legacy_segmented_backend_for_native_bridge`
is the only narrow concrete-backend escape hatch. It exists to keep
`chronos_native::read_log_with_stats` and the native attach path fed
while the hexagonal inversion is still incomplete.

### Allowed

- `crates/chronos-services/src/session_log.rs` — the definition.
- `crates/chronos-services/src/probe.rs` — the call sites (the only
  consumer: native probe lifecycle).

### Forbidden

Everywhere else, including but not limited to:

- `crates/chronos-services/src/canonical_drain.rs`
- `crates/chronos-services/src/events_log_read.rs`
- `crates/chronos-services/src/events_read.rs`
- `crates/chronos-services/src/observe.rs`
- `crates/chronos-services/src/projection.rs`
- `crates/chronos-services/src/tripwire_evidence.rs`
- `crates/chronos-services/src/error.rs`
- `crates/chronos-services/src/lib.rs`
- `crates/chronos-mcp/**`
- `crates/chronos-domain/**`
- `crates/chronos-log/**`
- any future crate

### Mechanical check

```bash
# Run from repo root.
grep -rn "legacy_segmented_backend_for_native_bridge" crates/ \
  --include="*.rs" \
  | grep -v "session_log.rs" \
  | grep -v "probe.rs"
```

Expected output: empty. Any line is a debt escalation. This check is
part of T0 from C3.3.2 onwards.

### Forbidden on this bridge (it is concrete-adapter only)

The bridge is for reaching the concrete `SegmentedExecutionLog` for
maintenance-adjacent native code. It is NOT a path to do canonical
evidence work.

- Forbidden: `append`, `record_gap`, `read_from_seq`, `retained_from`,
  `tail_seq`, `tail_state`, `seal` on the bridge result.
- Allowed on the bridge result (maintenance-adjacent only):
  `flush`, `compaction_metrics`, `maybe_compact`, `compact_up_to`,
  `retain_up_to`, `read_log_with_stats`, `attach_execution_log`.

The canonical-evidence path goes through `Arc<dyn ExecutionLogProvider>`
via `SessionExecutionLog.log` (or its `handle()` alias, which is a
misnomer kept temporarily for migration ease).

### Closes when

`legacy_segmented_backend_for_native_bridge` returns the concrete
`SegmentedExecutionLog` nowhere in production code, OR the function
itself is deleted because an equivalent native-port surface exists.

## R2 — `record_gap` is canonical-evidence (NOT maintenance)

`ExecutionLogProvider::record_gap` is the port-level write for declaring
gaps in canonical evidence. The wrapper exposes the same method on
`SessionExecutionLog` (delegating to the port).

### Forbidden

- A second `record_gap` method on `SessionExecutionLog` that goes to
  the concrete segmented adapter directly (`record_gap_on_segmented`
  is the LEGACY escape hatch and is allowed ONLY for callers that
  must hand a `chronos_log::Gap` to legacy code, e.g. existing tests
  that constructed gaps before the port existed; new code MUST use
  `wrapper.record_gap(...)`).
- `record_gap_on_segmented` outside `chronos-services/src/`.
- Calling `record_gap` on a maintenance bridge result.

### Allowed

- `SessionExecutionLog::record_gap(gap)` (delegates to port).
- `Arc<dyn ExecutionLogProvider>::record_gap(gap)`.
- `record_gap_on_segmented` in `session_log.rs` (legacy escape hatch)
  and existing test callers (until they migrate).

### Mechanical check

```bash
# record_gap_on_segmented is the legacy ratchet; callers must decrease.
grep -rn "record_gap_on_segmented" crates/ --include="*.rs" \
  | grep -v "session_log.rs"
```

Expected output: empty by end of C3.3.2 (or at most: tests with a
documented `// TODO: migrate to wrapper.record_gap` comment).

## R3 — Port stays minimal

`ExecutionLogProvider` does NOT have:

- `flush()` — maintenance, gated by `ProviderKind` on the wrapper.
- `compaction_metrics()` — maintenance, gated by `ProviderKind` on the wrapper.
- `maybe_compact()` — maintenance, gated by `ProviderKind` on the wrapper.
- `compact_up_to()` — maintenance, gated by `ProviderKind` on the wrapper.
- `retain_up_to()` — semantic, but currently maintenance-gated; future port surface (C33-DEBT-RETENTION-01).
- `dir()`, `path()`, `kind()` — would leak filesystem into the domain contract.
- Any helper that requires `Self: Sized` (would break object-safety).

### Mechanical check

```bash
# The port must remain object-safe; any `Self` in non-`where` clauses
# or any `Sized` bound is a debt escalation.
grep -n "Self: Sized\|where Self: Sized\|fn.*self.*-> Self" \
  crates/chronos-domain/src/ports/execution_log.rs
# Expected output: empty.

# Maintenance operations must NOT appear on the port.
grep -n "fn flush\|fn compaction_metrics\|fn maybe_compact\|fn compact_up_to\|fn retain_up_to" \
  crates/chronos-domain/src/ports/execution_log.rs
# Expected output: empty.
```

## R4 — Adapter wrappers are explicit

Adapters are concrete types named `SegmentedExecutionLogProvider` and
`InMemoryExecutionLogProvider`, NOT blanket impls on the existing
`SegmentedExecutionLog` / `InMemoryExecutionLog` types. This means:

- A new test or feature cannot accidentally satisfy the port by
  deriving the right thing on a pre-existing type.
- The port surface and the maintenance surface are two separate types;
  the wrapper holds the port Arc<dyn>.

### Forbidden

- `impl ExecutionLogProvider for Arc<SegmentedExecutionLog>`.
- `impl ExecutionLogProvider for &SegmentedExecutionLog`.
- Any blanket impl that would let `SegmentedExecutionLog` itself
  satisfy the port without going through `SegmentedExecutionLogProvider`.

## R5 — `map_log_error` is the legacy ratchet

`map_log_error(e: chronos_log::LogError) -> ServiceError` translates
the legacy log error type into the service error type. `map_execution_log_error`
translates the port's `ExecutionLogError`.

### Allowed

- `map_execution_log_error` — the canonical-evidence path.
- `map_log_error` ONLY in `chronos_log` internals or in adapters
  that need to translate from `LogError` to `ExecutionLogError` at the
  port boundary (none today; future adapters may need one).

### Forbidden

- New callers of `map_log_error` in `chronos-services` or `chronos-mcp`.
- `map_log_error` outside `chronos_log`.

### Mechanical check

```bash
grep -rn "map_log_error" crates/ --include="*.rs" \
  | grep -v "chronos-log/src/" \
  | grep -v "session_log.rs"  # session_log.rs is allowed to host map_log_error
                              # only if it has callers; check emptiness separately
# Also: number of map_log_error callers must decrease over cycles.
```

At cycle close (C3.3.1): `map_log_error` was removed from
`events_log_read.rs` and `projection.rs`. There are zero remaining
non-`chronos-log` callers. Ratchet locked.

## R6 — Composition root placement

The composition root lives in `chronos-mcp::composition`, NOT in
`chronos_services::composition`.

### Forbidden

- A `composition` module in `chronos-services`.
- Any `pub fn build_*(...)` in `chronos-services` that constructs a
  concrete adapter (`SessionStore::try_open`, `NativeProbeBackend::new`,
  `BrowserAdapter::new`, `EbpfAdapter::new`) without going through the
  port.
- Adapter constructors imported in `chronos-services` outside test code.

### Allowed

- The composition root in `chronos-mcp::composition` (C3.3.2 work).
- Adapter constructors used in tests of `chronos-services` (test fixtures
  are allowed; production code is not).
- `chronos-log` and `chronos-domain` exposing ports and adapters; they
  do not own composition.

## R7 — domain stays clock-free and serde-free at runtime

- `chronos-domain` does not read `SystemTime::now()`.
- `chronos-domain` does not depend on `serde_json` at runtime.

### State at C3.3.1 close (OBSERVED)

R7 is **partially violated at C3.3.1 close** by drift preexistent to
this cycle:

- `crates/chronos-domain/src/trace/session.rs` reads `Instant::now()`
  and `SystemTime::now()` in 6 places (introduced in `d433f8f9` —
  `feat(domain): CaptureSession exposes both monotonic and wall-clock
  timestamps (m0-08)`).
- `crates/chronos-domain/Cargo.toml` lists `serde_json` as a runtime
  dependency (`2e2b7bda m9-93: add JsonSchema derive to TraceEvent +
  transitive deps`).

Neither was touched by this cycle. Neither is part of C3.3.1 scope
(governance-only commit 8 forbids functional changes). R7 is documented
here as a known historical debt that future cycles must address; the
honest classification is **OPEN with OBSERVED drift**, NOT locked.

### Mechanical check (forward-looking)

```bash
grep -rn "SystemTime::now\|Instant::now" crates/chronos-domain/src/ --include="*.rs"
# Expected output (eventually): empty.

grep -n "serde_json" crates/chronos-domain/Cargo.toml
# Expected output (eventually): empty for runtime dependency.
```

R7 is registered as a known debt; the mechanical check above should be
satisfied before the cycle that closes it can claim C3.3.1-style
"domain is a clean hexagon" status.

## How these ratchets relate to debt items

| Ratchet | Enforces | Closes when debt item closes |
|---|---|---|
| R1 | `legacy_segmented_backend_for_native_bridge` placement | C33-DEBT-NATIVE-LOG-BRIDGE-01 |
| R2 | `record_gap` canonical-evidence | C31-DEBT-01 (already closed; ratchet prevents regression) |
| R3 | Port minimalism | C31-DEBT-01 (already closed; ratchet prevents regression) |
| R4 | Adapter wrappers explicit | C31-DEBT-01 (already closed; ratchet prevents regression) |
| R5 | `map_log_error` legacy ratchet | prevents regression of the ratchet advance in C3.3.1 |
| R6 | Composition root placement | C31-DEBT-03 (owner: C3.3.3) |
| R7 | domain stays clock-free and serde-free | OPEN with OBSERVED drift; close before claiming full hexagonal cleanliness |

R1, R2, R5, R6 are active ratchets that future cycles must preserve.
R3, R4 are mostly historical (enforced since before C3.3.1) but
remain ratchets because the architecture depends on them.
R7 is documented as a historical debt; closing it is future work,
NOT part of C3.3.1 scope.
