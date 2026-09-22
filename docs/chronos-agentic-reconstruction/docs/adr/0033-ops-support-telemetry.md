# ADR-0033 — OPS.4 support runbook + telemetry blueprint + health-check contract

**Cycle:** OPS / OPS.4 (Production-ready per profile — execution sub-cycle 4/5)
**Status:** `verified` post-write (branch HEAD == `main @ eebfc44a`)

---

## §1 Context

ROADMAP §OPS §101-§103 + ADR-0032 §2.2: OPS chapter has 5 sub-cycles. After OPS.2 (cert tier runner, `b38add22`) + OPS.3 (per-profile evidence files, `9f287312`), the certification pipeline produces tier reports. But OPS.6 (telemetry/diagnóstico) and OPS.8 (soporte/incidentes) are still **blueprints only** in `evidence/ops/ops.{6,8}.{profile}.json` — there's no actionable playbook, no health-check endpoint contract, and no defined telemetry wire format.

OPS.4 closes this gap by delivering:

1. **Support playbook** — `docs/runbooks/OPS-support-playbook.md` (235L). Actionable runbook for the 5 most common failure modes (binary not start, capture lost, MCP timeout, schema migration fail, OOM kill). Each case has signal → diagnosis → mitigation → escalation.
2. **Telemetry blueprint** — `docs/runbooks/OPS-telemetry-blueprint.md` (177L). Wire contracts for structured logs (JSON via `tracing`), health endpoint (types only), OTLP endpoint (opt-in). Recommended deployment topologies per profile + alert thresholds cross-referenced to the support playbook.
3. **Health check contract** — `crates/chronos-services/src/health_check.rs` (332L, 12 unit tests). New module defining `HealthStatus` (Healthy/Degraded/Unhealthy) + `ComponentHealth` + `HealthReport` types. NOT implementing the HTTP server (per-deployment decision).

This ADR formalizes the design choices: why contract-only (not full HTTP server), why 5 cases (not exhaustive), why blueprint (not collector implementation), and what we explicitly defer.

## §2 Decision

### §2.1 Support playbook scope = 5 cases (not exhaustive)

Per ADR-0032 §2.2 (OPS.4): "support-response playbook (5 casos más comunes: binario no inicia, captura perdida, MCP timeout, schema migration falla, OOM)". Chosen cases:

| Case | Failure mode |
|---|---|
| §2 Binary not start | redb lock contention / schema mismatch / missing capabilities |
| §3 Capture lost | probe not attached / eBPF not supported / ring buffer pressure / redb writer saturated |
| §4 MCP timeout | slow tool / lock contention / system load |
| §5 Schema migration fail | forward migration / backward (rollback) / corruption |
| §6 OOM kill | unbounded allocation / leak |

**Rationale (80/20)**: these cover ~80% of real failures per ops experience. Anything outside requires investigation + a new playbook entry. Exhaustiveness is a goal that grows forever and never ships.

### §2.2 Health check = contract only (NOT HTTP server)

Per ADR-0033 §3 (this ADR): the `health_check.rs` module defines **types** (`HealthStatus`, `ComponentHealth`, `HealthReport`) but does NOT implement an HTTP server. The HTTP server is operator's choice:

- **local/stdio profile**: no HTTP listener (single-process, logs sufficient).
- **linux-privileged profile**: operator chooses between axum/hyper/warp/actix; binds to unix socket or TCP port.
- **remote/multi-tenant profile (NOT IMPLEMENTED)**: would need auth + rate-limit + redaction (future work).

Why "contract only"?

1. **Deployment-specific**: no single library-side HTTP server works for all 3 profiles.
2. **Forward compatibility**: by exposing only types, the operator can wire any HTTP framework without depending on a particular framework version.
3. **Reusable**: the same `HealthReport` JSON shape can be returned by a CLI subcommand (`chronos health`), a unix-socket responder, or scraped by Prometheus — the operator chooses the transport.

The types in `health_check.rs` are deliberately minimal so they fit any transport.

### §2.3 Telemetry blueprint = forward-compatible OTLP + tracing

Per ADR-0033 §2.3 + ADR-0004: Chronos emits **structured JSON logs** via `tracing` + a JSON formatter (pre-existing) + an optional **OTLP gRPC endpoint** at `:4317` if `CHRONOS_OTLP_ENABLED=1` is set (also pre-existing). The blueprint describes:

- Wire formats (logs JSON schema, OTLP signals).
- Recommended deployment topologies per profile.
- Alert thresholds (8 metrics with runbook cross-references).

**Why blueprint, not implementation?** Per ADR-0032 §4 (alternatives considered): building OTel collector + log shipping + health check endpoint in OPS.4 was **rejected** because (a) telemetry infra is meta-scope — depends on operator's deployment; (b) OPS.4 delivers blueprint forward-compatible with any deployment choice; (c) the actual collector is a per-deployment decision (not every operator wants OTel).

### §2.4 Alert thresholds + runbook cross-references

Per ADR-0004 (every signal must be observable, every mitigation must be actionable): each alert threshold has a corresponding runbook entry. Example: `health.status == unhealthy` → [§2 binary not start] OR [§5 schema migration]. The cross-reference table is in `OPS-telemetry-blueprint.md` §4.

This makes the runbook actionable rather than decorative: an operator who sees an alert knows exactly which playbook entry applies.

## §3 Alternatives considered

### §3.1 Implement the HTTP server in `health_check.rs`

Rejected. Would require picking an HTTP framework (axum/hyper/warp) at compile time, locking in dependencies. Operators want different choices per profile.

### §3.2 Implement OTel collector in OPS.4

Rejected per ADR-0032 §4. Telemetry infra is meta-scope. Operators choose their own collector + backend.

### §3.3 Cover >5 cases in the support playbook

Rejected. Exhaustiveness never ships. 80/20 rule: 5 cases cover ~80% of failures; the rest get investigation + new playbook entries as they emerge.

### §3.4 Cross-process distributed tracing

Rejected. `chronos-mcp` is single-process per H1.2 §10. Cross-process spans are out of scope until the remote/multi-tenant profile (NOT IMPLEMENTED) ships.

## §4 Consequences

### §4.1 Positive

- **Forward-compatible**: the health check types are stable contracts; HTTP framework upgrades don't break the wire format.
- **Operator flexibility**: 3 deployment profiles × N transports (CLI / unix socket / TCP / Prometheus) all share the same JSON shape.
- **Reduced scope**: OPS.4 stays within ~700 LoC docs + 332 LoC types. Future OPS.4+ cycles can add HTTP server implementations per profile without re-designing the contract.
- **Actionable runbook**: 5 cases × 4 sections (signal/diagnose/mitigate/escalate) is concrete enough that an on-call operator can use it without further design.

### §4.2 Negative

- **HTTP endpoint not implemented**: operators must wire their own. This is a deployment cost, not a library cost.
- **Playbook gaps**: cases outside the 5 will require investigation. Acceptable per ADR-0032 §2.2.
- **No auto-generated metrics**: Chronos emits via `tracing`; OTLP metrics require the optional `tracing-opentelemetry` feature. Operators must enable it explicitly.

### §4.3 Neutral

- **Health endpoint contract stable**: any future OPS cycle that adds HTTP server implementations reuses `HealthReport` JSON shape, no breaking change.
- **Telemetry blueprint is a doc**: it doesn't enforce any operator behavior; it's guidance. Operators can ignore it (their telemetry won't be optimal, but Chronos still works).

## §5 Verification evidence

| Stage | Command | Result |
|---|---|---|
| T0 build (incremental) | `cargo build -p chronos-services --lib` | Finished 1m 22s |
| T1 unit (full) | `cargo test -p chronos-services --lib --no-fail-fast` | **473/473 PASS** (5.73s) |
| T1 unit (health_check only) | `cargo test -p chronos-services --lib health_check --no-fail-fast` | **12/12 PASS** (0.00s) |
| T0 clippy | `cargo clippy -p chronos-services --lib --tests --no-deps -- -D warnings` | exit=0 (0 warnings) |

OPS.4 evidence file `evidence/ops/ops.6.{profile}.json` and `ops.8.{profile}.json` should be updated post-merge to reflect the new artefacts (this is OPS.5's job; tracking as a follow-up).

## §6 Mapping to UAT

| UAT | Status |
|---|---|
| UAT-OPS-04-01 (HealthReport serialize round-trip) | ✅ via `serialize_round_trip_preserves_all_fields` test |
| UAT-OPS-04-02 (worst_status reduces components) | ✅ via `worst_status_returns_max` test |
| UAT-OPS-04-03 (is_healthy requires all Healthy) | ✅ via `report_all_healthy_is_healthy` test |
| UAT-OPS-04-04 (Unhealthy forces report Unhealthy) | ✅ via `worst_status_returns_max` (3rd case) |
| UAT-OPS-04-05 (Playbook covers 5 cases) | ✅ via `OPS-support-playbook.md` §2-§6 |
| UAT-OPS-04-06 (Blueprint defines OTLP wire format) | ✅ via `OPS-telemetry-blueprint.md` §2 |

## §7 Out-of-scope

- HTTP server implementation (operator's choice — `axum`/`hyper`/`warp`/`actix`).
- Authentication / rate-limit (deployment profile decision).
- Prometheus exposition endpoint (operator's deployment decision).
- Per-component health probes (each component reports its own).
- Probe attach/detach lifecycle (MCP server's job).
- OTel collector implementation (operator's deployment decision).
- PII redaction (per H1.2 §10, no PII is logged; moot).
- Cross-process distributed tracing (single-process per H1.2 §10).
- Sampling rate configuration (opt-in feature).
- OPS evidence files update (OPS.5's job).

## §8 Files

| File | Role |
|---|---|
| `crates/chronos-services/src/health_check.rs` | Health check contract (332L, 12 tests) |
| `crates/chronos-services/src/lib.rs` | `pub mod health_check;` registration |
| `docs/runbooks/OPS-support-playbook.md` | Support playbook (235L, 5 cases) |
| `docs/runbooks/OPS-telemetry-blueprint.md` | Telemetry blueprint (177L, wire contracts + alert thresholds) |
| `docs/chronos-agentic-reconstruction/docs/adr/0033-ops-support-telemetry.md` | This ADR |

## §9 References

- ADR-0004 — Silent Lie Prohibition.
- ADR-0032 — OPS chapter scoping.
- ROADMAP §OPS §101-§103 — Production-ready per profile checklist.
- H1.2 §10 — operational guidance (playbook extends this).
- H1.6 — install/upgrade/rollback runbook (cross-referenced from §5).
- OpenTelemetry specification — OTLP wire format.
