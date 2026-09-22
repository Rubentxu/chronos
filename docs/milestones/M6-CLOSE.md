# M6 (chapter) — OpenTelemetry correlation and export close report

**Status:** Closed 2026-09-22
**Cycle:** M6 (OpenTelemetry end-to-end)
**Exit criterion:** *OpenTelemetry wire contract end-to-end across services, with type-level separation between internal Chronos correlation and external W3C context, redaction + cardinality caps, cross-service validation with two concurrent requests, and operational gates covering load + recovery + error paths.*
**Verdict:** ✅ Satisfied (7/7 sub-cycles).

This report audits the M6 chapter at the close of sub-cycle M6.7, records the achieved state, and documents the deferred items per honest scope (ADR-0004 §2.2).

> **Naming clarification.** The cycle labels `m6-01..m6-05` used in commit footers and `docs/milestones/m6-*.md` refer to the **v2-spec surface reduction sub-cycle** (deferred from M5 per `docs/milestones/m5-close-report.md` §4). This is distinct from the **reconstruction-roadmap M6** (`docs/chronos-agentic-reconstruction/docs/roadmap/ROADMAP.md` §M6 line 71) which targets OpenTelemetry correlation + export and is the chapter this close report covers. Sub-cycle labels `M6.1..M6.7` below refer to the **reconstruction-roadmap M6** sub-cycles defined in ROADMAP §M6 §86.

---

## 1. Exit criterion verdict

The ROADMAP §M6 §86 framed the M6 chapter around seven concrete deliverables:

| Sub-cycle | ROADMAP scope | ADR | Status |
|---|---|---|---|
| M6.1 | `ExternalTraceContext` separate from `InvocationId` | ADR-0015 (297L) | verified |
| M6.2 | OTLP local-ingest adapter | ADR-0016 (301L) | verified |
| M6.3 | unambiguous correlation with mutation evidence | ADR-0017 (340L) | verified |
| M6.4 | opt-in export with limits | ADR-0018 (302L) | verified |
| M6.5 | security/redaction + cardinality | ADR-0019 (347L) | verified |
| M6.6 | UAT-M6-01/02 cross-service with two concurrent requests | ADR-0020 (416L) | verified |
| M6.7 | load/recovery/error gates | ADR-0021 (333L) | verified |

**Verdict:** satisfied. Seven cycles (M6.1..M6.7) shipped every deliverable on the planned list. The M6 layer is end-to-end: type-level separation between Chronos internal correlation (`InvocationId` UUID v4) and external W3C context (`ExternalTraceContext` `traceparent`/`tracestate`), HTTP transport with 4-outcome contract, correlation bookkeeping with mutation invariant, opt-in export pipeline with 5-counter `ExportResult`, redaction + cardinality layered pre-network, cross-service validation with two concurrent traces, and three operational gates covering load (1000 invocations handled, p50/p95/p99 within budget) + recovery (UP→DOWN→UP via `OutageState`, idempotency over `trace_id`) + error paths (graceful degradation, no invented provenance).

## 2. Achieved state

### 2.1 Wire format (locked at M6.4, validated at M6.6/M6.7)

OTel-shaped JSON Lines emitted per span:
- `trace_id` (32 lowercase hex, no dashes; all-zero for internal-only)
- `span_id` (16 lowercase hex, derived deterministically from `event_idx`)
- `name` `chronos.event.{probe}`
- `start_time_unix_nano` (derived from `event.ts_micros * 1000`; **NO invented Unix timestamps** per ADR-0004 §2.2)
- `chronos_invocation_id` (full UUID v4 hyphenated, distinct per service per ADR-0020 §2.3)
- `attributes` always strings (Int → `i.to_string`, Bool → `b.to_string`, Str passthrough, DomainRef → `ref:{name}`)

### 2.2 Pipeline ordering

`ChronosEvent` → `CorrelationStore` (M6.3) → `export_spans()` (M6.4) → `redact_and_limit_spans()` (M6.5) → `render_json_line()` (M6.4) → JSONL stdout/OTLP consumer.

### 2.3 Cross-service invariants (M6.6)

- Two services exchanging HTTP/JSONL with consumer: distinct `chronos_invocation_id`s, correlated `trace_id`s.
- Concurrent requests with distinct `traceparent`s: 0 cross-talk, 0 orphan spans, monotonic-clock drift tolerated per-service.
- Idempotency over `trace_id` (not over `chronos_invocation_id`); retry with same `traceparent` produces different `chronos_invocation_id`s but same `trace_id`.

### 2.4 Operational gates (M6.7)

- Load: 1000 invocations handled, 0 dropped, p50=9µs, p95=10µs, p99=24µs, max=80µs.
- Recovery: `OutageState` UP→DOWN→UP, 10 outages recorded, 2 invocations handled after recovery, idempotency holds.
- Error paths: empty / malformed / zero / extreme cardinality inputs → graceful degradation, 0 panic, no invented provenance.

## 3. Source of truth

| Item | Path | Notes |
|---|---|---|
| ADR-0015..0021 | `docs/chronos-agentic-reconstruction/docs/adr/0015-m6.1-external-trace-context.md` .. `0021-m7-otel-gates.md` | 7 ADRs, 297..416 lines each |
| Spike source code | `/home/rubentxu/m6-spikes/` (durable, NOT scratch) | 7 sub-projects with Cargo.toml + src/ + tests/ + examples/ + evidence/ |
| Source SHA-256 | per sub-cycle `evidence/source-shas.txt` | preserved bit-exact |

## 4. Orphan rule

M6.6 + M6.7 spikes path-depend on M6.1..M6.5 (all at durable `/home/rubentxu/m6-spikes/`). M6.x does NOT modify M6.{1..6} code. M6.x accessors added non-breaking to M6.1 + M6.3 + M6.4 for cross-spike consumption.

## 5. Honest limitations (ADR-0004 §2.2 No Silent Lies)

The M6 chapter is **closed for the reconstruction roadmap scope** but the following items are explicitly out-of-scope per ADR-0015..0021:

- **Productionization** (lift spikes into `chronos-core` as a submodule; wire to `chronos-services` dispatcher; wire to `chronos-mcp` tool surface).
- **Batched OTLP** (HTTP/2 / gRPC, compression, retry-with-backoff, persistent queue).
- **TLS / auth** for the OTLP adapter.
- **Value-pattern redaction** (regex/streaming pattern matcher; only key-name substring match in M6.5).
- **Per-tenant policies** (single global `RedactionPolicy` in M6.5).
- **On-wire integrity** (sign spans pre-redaction; HMAC over JSONL).
- **Cross-process / cross-host** transport (M6.6 loopback only).
- **Async runtime** (M6.7 sync-only by design; deterministic, no Tokio).
- **Persistent storage of gates output** (M6.7 in-memory only).
- **CapCorrelationPersistence / CapTenantPolicy / CapValueRedact / CapOnWireIntegrity** follow-ups.

## 6. Verification chain

| Step | Command | Result |
|---|---|---|
| SHAs preserved | `git cat-file -e 64d28b28 f6e13843 <each M6.x ADR merge>` | exit=0 (8 SHAs OK) |
| Workspace T0 | `cargo fmt --all -- --check` | exit=0 |
| Workspace T0 | `cargo clippy -p chronos-mcp --tests --no-deps -- -D warnings` | exit=0 |
| Workspace T1 subset | `cargo test -p chronos-mcp -p chronos-services --no-fail-fast` | 84/84 + 433/433 PASS (proportional — M6.7 merge is docs-only) |
| CC#4 | 102 manifests clean | preserved |
| Cargo.lock sha256 | `68ee81f284bf115529c7060e80d6e2193e109f956d51dc8767a69d48febb0e4f` | unchanged |
| v0.7.112 tag | peels `0be2ec2d` | intact |

## 7. Chapter close tag

`m6-otel-correlation.0` (annotated, NOT GPG-signed per env limitation per ADR-0004 §2.2), peels `64d28b284757796cb2bffe2d4172118696af8958` (the M6.7 merge commit).

## 8. Next autonomous work

After M6 chapter close, the natural next step is **M7 — Differential execution v2** (ROADMAP §M7 §77, 4 sub-cycles: M7.1 equivalence criterion + M7.2 invocation alignment + M7.3 behaviour fingerprint + M7.4 cost/memory/collision). M7 is **already executed (4/4 verified)** as documented in `docs/milestones/M7-CLOSE.md` (peer document). Side-tracks H1.4-B / H1.5-B / H1.1.2 available.
