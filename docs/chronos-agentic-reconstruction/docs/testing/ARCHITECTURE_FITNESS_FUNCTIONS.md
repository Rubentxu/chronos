# Architecture fitness functions

These checks turn architecture decisions into executable constraints. They are deliberately small and conservative: a gate should detect drift reliably rather than pretend to measure architecture with a single score.

## AF-01 — Dependency direction

Machine source: `reconstruction-contracts.toml` + `scripts/check_architecture_contracts.py`.

Forbidden target-state edges include:

- `chronos-domain -> reqwest|tokio|tracing` infrastructure;
- `chronos-services -> chronos-native|chronos-ebpf|chronos-browser|chronos-store` concrete adapters;
- `chronos-store -> chronos-native`.

Existing violations are explicit temporary waivers. A new violation fails CI. Removing a violation without removing its waiver also fails CI, so the debt baseline can only shrink intentionally.

## AF-02 — No new legacy evidence primitives

Added production Rust lines are rejected when they introduce selected legacy primitives:

- `EventBus`;
- `snapshot_raw(`;
- `drain_fired(`;
- `TraceAdapter`;
- `UnsupportedOperation(`.

This is a ratchet, not proof that existing use is correct. REC-C1..C4 remove the baseline uses; REC-C7 requires the legacy allowlist/debt to be empty.

## AF-03 — Verified means reproducible

A contract may use status `verified` only if it records all of:

- `evidence`: concrete implementation/test paths;
- `uat`: one or more behavioral acceptance identifiers;
- `verify`: a reproducible command.

A document, DTO, tool registration or close report is not evidence by itself.

## AF-04 — No Silent Lies behavioral gates

Every agent-visible read path MUST have UAT covering:

- unknown/invalid input -> explicit validation error;
- unsupported capability -> explicit unsupported state;
- evidence loss -> explicit `Gap`/incomplete state;
- empty legitimate result -> distinguishable from unavailable/unknown;
- heuristic result -> heuristic classification and provenance;
- cursor stale/invalid -> explicit error, never silent restart from zero.

## AF-05 — Independent-consumer evidence

For every streaming/paged evidence API:

1. create at least two consumers;
2. interleave reads;
3. allow producers to advance between reads;
4. prove consumer A does not advance or truncate B;
5. prove resume begins after the authoritative last `EventSeq`;
6. force retention loss and prove a Gap is returned.

## AF-06 — Replay equivalence

Projection state produced by:

```text
full log replay
```

must equal:

```text
checkpoint + delta replay
```

for invocation, property, call graph and future causality projections. Equality must be semantic and deterministic.

## AF-07 — Capability honesty

A backend advertises capabilities before an agent plans a query. Tests assert:

- advertised capability -> operation succeeds for a fixture;
- absent capability -> operation is not offered/selected;
- no backend relies on discovering normal unsupported behavior by invoking a wide optional method.

## AF-08 — Time/identity semantics

Tests distinguish:

- `EventSeq`: ordering only;
- monotonic timestamp: elapsed runtime time;
- wall-clock timestamp: cross-system correlation.

An ordinal index MUST NOT be serialized as nanoseconds. Cross-system OTLP export MUST NOT use session-relative monotonic time as Unix time.

## AF-09 — MCP thinness

New MCP wrappers may perform only:

- schema/DTO decoding;
- authentication/transport concerns if introduced;
- calling one application use-case/dispatcher;
- mapping service result to protocol result.

No query, diff, property, probe-selection or persistence algorithm may be added directly to the MCP adapter.

Review check: any wrapper with non-trivial loops, graph traversal, event filtering, persistence decisions or backend construction requires extraction before merge.

## AF-10 — Feature matrix

CI compiles `--workspace --all-targets --all-features`. Runtime/platform-specific UATs may be separate jobs, but feature code is not allowed to rot merely because the default feature set does not compile it.

## AF-11 — Contract-to-UAT traceability

Every mandatory roadmap item has a row in `reconstruction-contracts.toml`. When a requirement changes:

1. update ADR/spec if semantics change;
2. update ledger status/evidence;
3. update or add UAT;
4. update roadmap gate if ordering changes;
5. land all changes in the same cycle.

## AF-12 — Close gate

At REC-C7 run:

```bash
python3 scripts/check_architecture_contracts.py --strict-no-gaps
cargo check --workspace --all-targets --all-features
cargo test --workspace -- --test-threads=1
```

plus the real sandbox UAT bundle. `--strict-no-gaps` rejects `gap`, `partial` and `blocked` states owned by REC-C milestones.

## What these functions do not do

They do not replace code review. SOLID, connascence and smells still require design review. The point is to make the most important architectural invariants impossible to regress silently.
