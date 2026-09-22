# M7 (chapter) — Differential execution v2 close report

**Status:** Closed 2026-09-22
**Cycle:** M7 (Differential execution v2)
**Exit criterion:** *Criterion of semantic equivalence + hierarchical hashes, alignment by invocation/context (not timestamps), behaviour fingerprint for regression detection, and UAT scenarios with cost/memory baselines + collision hunt.*
**Verdict:** ✅ Satisfied (4/4 sub-cycles).

This report audits the M7 chapter at the close of sub-cycle M7.4, records the achieved state, and documents the deferred items per honest scope (ADR-0004 §2.2).

> **Naming clarification.** The cycle labels `m7-01..m7-07` used in commit footers and `docs/milestones/m7-*.md` refer to the **v2-spec surface reduction sub-cycle** (M6 continuation; see `docs/milestones/m7-close-report.md`). This is distinct from the **reconstruction-roadmap M7** (`docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` §M7 line 75) which targets differential execution v2 and is the chapter this close report covers. Sub-cycle labels `M7.1..M7.4` below refer to the **reconstruction-roadmap M7** sub-cycles defined in ROADMAP §M7 §77.

---

## 1. Exit criterion verdict

The ROADMAP §M7 §77 framed the M7 chapter around four concrete deliverables:

| Sub-cycle | ROADMAP scope | ADR | Status |
|---|---|---|---|
| M7.1 | semantic equivalence criterion + hierarchical hashes | ADR-0022 (376L) | verified |
| M7.2 | alignment by invocation/context, not only timestamps | ADR-0023 (289L) | verified |
| M7.3 | state/properties comparison + `BehaviourFingerprint` as measured spike | ADR-0024 (354L) | verified |
| M7.4 | UAT-M7-01/02 + cost/memory baselines + collision hunt | ADR-0025 (451L) | verified |

**Verdict:** satisfied. Four cycles (M7.1..M7.4) shipped every deliverable on the planned list. The M7 layer is end-to-end: `EquivalenceSpec { ignore_timestamps, ignore_field_order }` with FNV-1a 64-bit hierarchical hashes (event→invocation→session), `align_sessions` with primary UUID / fallback `trace_id` alignment + 4-way classification (`Matched`/`Mismatched`/`OnlyInA`/`OnlyInB`) + `AmbiguousInvocationId` rejection, `BehaviourFingerprint` with `aggregate_hash` + `fingerprint_hash` over sorted canonical byte streams, and UAT scenarios M7.4 (cost baseline 1.12M invocations/sec, memory 0 reallocations, collision hunt 0/10000 in 10K corpus with splitmix64 deterministic PRNG).

## 2. Achieved state

### 2.1 Wire equivalence (M7.1)

- `EquivalenceSpec { ignore_timestamps: bool, ignore_field_order: bool }` — default lenient (both true), strict (both false).
- `canonical_event_bytes(event, spec) -> Vec<u8>` length-prefixed + type-tagged (Str=0x01, Int=0x02, Bool=0x03, DomainRef=0x04).
- Three hierarchical hashes FNV-1a 64-bit (`FNV_OFFSET=0xcbf29ce484222325` + `FNV_PRIME=0x100000001b3`; **NO `DefaultHasher`** because of randomized seed per Rust version):
  - `hash_event_canonical` (per-event)
  - `hash_invocation_canonical` (sort+dedup event hashes)
  - `hash_session_canonical` (sort+dedup invocation hashes)
- `DiffVerdict::Equivalent | Differ` (binary, honest about what spec ignores).

### 2.2 Session alignment (M7.2)

- `InvocationWithEvents<'a>` (RecordedInvocation + events).
- `Session<'a>` (list of InvocationWithEvents).
- `AlignmentKey` (Invocation | Trace); `AlignmentKeyKind` (Invocation | Trace | Mixed).
- `align_sessions(a: &Session, b: &Session, spec: &EquivalenceSpec) -> Result<AlignmentReport, AlignmentError>`.
- `AlignmentError::AmbiguousInvocationId { invocation_id, occurrences }` — **rejected, never silent pick** (No Silent Lies, ADR-0004).
- 4-way classification: `Matched` (primary), `Mismatched` (primary, content differs), `OnlyInA`, `OnlyInB`.

### 2.3 Behaviour fingerprint (M7.3)

- `BehaviourFingerprint { aggregate_hash, fingerprint_hash, counts }`.
- `session_fingerprint` consumes `AlignmentReport`; produces `matched_count`, `mismatched_count`, `only_in_a`, `only_in_b`, `aggregate_hash`, `fingerprint_hash`.
- Per-pair emit bytes `aggregate_bytes_for_pair(p) = [status_tag: u8][hash_a: u64][hash_b: u64][delta_or_sentinel: u64]` (25 bytes/pair).
- Per-pair emit bytes `shape_bytes_for_pair(p) = [status_tag: u8][primary_hash: u64][delta_or_sentinel: u64]` (17 bytes/pair).
- Sort by `(status_tag, primary_hash)` ascending; FNV-1a 64-bit fold each stream → `aggregate_hash` and `fingerprint_hash`.
- `fingerprint_equiv` + `fingerprint_shape_only_equiv` (chaos/replay patterns).

### 2.4 UAT-M7-01/02 + cost/memory/collision (M7.4)

- `CostBaseline` + `measure_cost` (`Instant::now` for p50/p95/p99, **NO wall-clock**).
- `MemoryBaseline` + `measure_memory` (`Vec::with_capacity(exact)` → 0 reallocations, aggregate_stream_size 25 bytes/pair, shape_stream_size 17 bytes/pair).
- `CollisionReport` + `collision_hunt` (splitmix64 deterministic PRNG).
- `UatOutcome::FoundDivergence | UnknownUnsupported | FalseEquality | NoDivergence`.
- `run_uat_m7_01` (single-inv shared trace T1 + state bug 3150 vs 3500 → `FoundDivergence`).
- `run_uat_m7_02` (single-inv trace T1 vs T2 distinto → `UnknownUnsupported`).
- Smoke: S1 FoundDivergence agg=`727489852173063677` fp=`16296333918912416704`; S2 UnknownUnsupported agg=`15527282866049128162` fp=`10611589587867544546`; S3 cost 1000×100 iters per_iter=890.023µs invocations_per_sec=1123566; S4 memory peak=25000 total=42000 reallocations=0; S5 stream sizes agg=25000 shape=17000 combined=42000; S6 collision hunt seed=0xC0FFEE corpus=10000 unique_aggregates=10000 unique_fingerprints=10000 agg_collisions=0 fp_collisions=0 (0/10K); S7 fingerprint_equiv=true fingerprint_shape_only_equiv=true; `ALL CHECKS PASSED`.

## 3. Source of truth

| Item | Path | Notes |
|---|---|---|
| ADR-0022..0025 | `docs/chronos-agentic-reconstruction/docs/adr/0022-m7.1-differential-equivalence.md` .. `0025-m7.4-cost-memory-collision.md` | 4 ADRs, 289..451 lines each |
| Spike source code | `/home/rubentxu/m7-spikes/` (durable, NOT scratch) | 4 sub-projects with Cargo.toml + src/ + tests/ + examples/ + evidence/ |
| Source SHA-256 | per sub-cycle `evidence/source-shas.txt` | preserved bit-exact |

## 4. Orphan rule

M7.4 spike path-depends on M6.1 + M6.3 + M7.1 + M7.2 + M7.3 (all at durable paths). M7.2 + M7.3 + M7.4 do NOT modify M6.x or earlier M7.x code. M7.x accessors added non-breaking to M6.1 + M6.3 for cross-spike consumption.

## 5. Honest limitations (ADR-0004 §2.2 No Silent Lies)

The M7 chapter is **closed for the reconstruction roadmap scope** but the following items are explicitly out-of-scope per ADR-0022..0025:

- **Persistent storage of canonical bytes** (M7.4 in-memory only).
- **Cryptographic hash** (FNV-1a is NOT cryptographic; M7 is internal regression detection, no auth).
- **128-bit hash** (FNV-1a 64-bit sufficient for typical session sizes).
- **Per-probe canonicalisation** (M7 is session-level).
- **Streaming hash** (cap by session size; no incremental fold).
- **Field-level diff** (M7 reports aggregate equivalence).
- **Cross-session fingerprint aggregation** (over-session aggregation is M7.3 internal; cross-session is M1+).
- **Alignment across heterogeneous event sources** (M4R.3 overlay integration is M1+).
- **Memory-pressure-aware streaming** (M7 cap by session size, no back-pressure).
- **Productionization** (lift spikes into `chronos-core`; wire to `chronos-services` dispatcher; wire to `chronos-mcp` tool surface).
- **Cross-host / cross-process** session comparison.

## 6. Verification chain

| Step | Command | Result |
|---|---|---|
| SHAs preserved | `git cat-file -e f6e13843 acc44d2e 8800f850 b8f79630 <each M7.x ADR merge>` | exit=0 (8 SHAs OK) |
| Workspace T0 | `cargo fmt --all -- --check` | exit=0 |
| Workspace T0 | `cargo clippy -p chronos-mcp --tests --no-deps -- -D warnings` | exit=0 |
| Workspace T1 subset | `cargo test -p chronos-mcp -p chronos-services --no-fail-fast` | 84/84 + 519/519 PASS (proportional) |
| CC#4 | 102 manifests clean | preserved |
| Cargo.lock sha256 | `68ee81f284bf115529c7060e80d6e2193e109f956d51dc8767a69d48febb0e4f` | unchanged |
| v0.7.112 tag | peels `0be2ec2d` | intact |

## 7. Chapter close tag

`m7-differential-v2.0` (annotated, NOT GPG-signed per env limitation per ADR-0004 §2.2), peels `f6e1384321450ce4694663f35eecd7184287db46` (the M7.4 merge commit).

## 8. Mapping to UAT

- UAT-M7-01 (M7.1 covered + M7.2 confirms per-invocation survival)
- UAT-M7-02 (Scenario 2 + test 04 `mismatched_content_same_invocation_id`)
- UAT-M7-03 (Scenario 3 + tests 05/06 `only_in_a/b`)
- UAT-M7-04 (tests 03/11/17 `timestamp_drift_lenient + ambiguous + delta_event_count`)
- UAT-M7-05 (test 19 `hash_propagation_through_alignment`)
- UAT-M7-06 (test 18 `large_session_1000_invocations`)

## 9. Next autonomous work

After M7 chapter close, the natural next step is **M8 — Counterexample shrinking y test intelligence** (ROADMAP §M8 §79, already `CLOSED 2026-09-11` per `docs/milestones/M8-CLOSE.md`). Side-tracks H1.4-B / H1.5-B / H1.1.2 available.

After the larger ROADMAP (H1.x + M4 + M6 + M7 + M8 + M9 + M10 + M11 + OPS) is documented CLOSED, future work candidates:

- **H1.4 slice B** — extract `ChronosServer` cohesive sub-contexts per `docs/architecture/H1.4-chronos-server-cohesion-map.md`.
- **H1.5 slice B** — runtime capability matrix + perf baselines (deferred per env if runtimes not installed).
- **H1.1.2** — CVE remediation (reqwest 0.11 → 0.12 + MSRV 1.75 → 1.78).
- **M6 productionization** — lift M6.x spikes into `chronos-core` + wire to dispatchers/MCP.
- **M7 productionization** — lift M7.x spikes into `chronos-core` + wire to dispatchers/MCP.
- **G0.x CI/Coverage/Vault Drift** — requires CI infra (deferred per env).
