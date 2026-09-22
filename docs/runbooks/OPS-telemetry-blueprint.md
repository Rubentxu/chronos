# OPS telemetry blueprint

**Cycle:** OPS / OPS.4 (support runbook + telemetry blueprint)
**Precedence:** `docs/architecture/H1.5-runtimes-capabilities-benchmarks.md` (perf budgets); `docs/security/H1.2-threat-model.md` §10 (observability requirements)
**Status:** blueprint (forward-compatible; NO collector implemented in this cycle)

This document describes the **target telemetry architecture** for Chronos. It is intentionally a blueprint, not an implementation: actual OTel collector deployment, log shipping, and metrics backend are per-deployment decisions (operator's choice of vendor, retention, sampling rate).

> **Per ADR-0033 §2**: this blueprint defines the **contract** that any deployment-specific telemetry pipeline must satisfy. Chronos itself emits structured logs + an optional OTLP endpoint (if enabled); the operator wires the rest.

---

## §1 Goals

Per ROADMAP §OPS §101-§103 + ADR-0032 §2.2 + ADR-0004:

1. **Observable**: every error has a structured log line + a metric.
2. **Actionable**: every metric has a documented alert threshold + runbook entry.
3. **Forward-compatible**: wire formats follow OpenTelemetry conventions so any collector / backend works.
4. **Local-first**: telemetry must work with no external service (file-based fallback).

---

## §2 Wire contracts

### §2.1 Structured logs (JSON)

Per ADR-0033 §2.1: Chronos emits **structured JSON logs** via the `tracing` crate + a JSON formatter. Format:

```json
{
  "timestamp": "2026-09-22T10:00:00.123Z",
  "level": "INFO",
  "target": "chronos_services::events_read",
  "message": "events read by query, returned 100 events",
  "fields": {
    "session_id": "...",
    "mode": "query",
    "count": 100,
    "duration_ms": 12
  },
  "spans": [
    {"name": "tool_call", "session_id": "...", "tool_name": "events_read"}
  ]
}
```

Operators collect via `journalctl -u chronos-mcp -o json` (systemd journal) or pipe stdout to a file.

### §2.2 Health endpoint contract

Per `crates/chronos-services/src/health_check.rs` + ADR-0033 §2.3:

```
GET /health → 200 OK
Content-Type: application/json

{
  "status": "healthy" | "degraded" | "unhealthy",
  "version": "0.7.112",
  "uptime_seconds": 3600,
  "components": [
    {"name": "redb_store", "status": "healthy"},
    {"name": "ebpf_probes", "status": "healthy"},
    {"name": "mcp_server", "status": "degraded", "details": "high latency"}
  ]
}
```

**NOT IMPLEMENTED in this cycle** (per ADR-0033 §3). The contract is defined in `health_check.rs` for forward compatibility; HTTP server is operator's choice (axum / hyper / warp / actix).

### §2.3 OTLP endpoint contract (optional)

If `CHRONOS_OTLP_ENABLED=1` is set, Chronos exposes an OTLP gRPC endpoint at `0.0.0.0:4317`. Signal types:

| Signal | Source | Purpose |
|---|---|---|
| **Metrics** | `chronos-mcp` process counters | request rate, error rate, latency per tool |
| **Logs** | `tracing` JSON output | structured events with correlation IDs |
| **Traces** | `tracing-opentelemetry` spans | request flow across crates |

OTLP is **opt-in** because not all deployments want OTel collector overhead. Local/stdio profile typically runs without OTLP.

---

## §3 Recommended deployment topology

### §3.1 Local/stdio profile

```
[chronos-mcp process]
    │
    ├── stdout → file (~/.local/share/chronos/log.json)
    │   (rotated daily; 7-day retention)
    │
    └── no OTLP endpoint (CHRONOS_OTLP_ENABLED unset)
```

**Rationale**: local profile is single-process, no remote backend. Logs are sufficient.

### §3.2 Linux privileged profile

```
[chronos-mcp process]                    [OTel Collector]
    │                                          │
    ├── OTLP gRPC :4317 ─────────────────────► │
    │                                          │
    ├── stdout → journald ───────────────────► │ (logs)
    │                                          │
    └── /health (operator's HTTP server) ────► │ (scraper, every 30s)
                                               │
                                               ├── Metrics → Prometheus
                                               ├── Logs → Loki / Elasticsearch
                                               └── Traces → Tempo / Jaeger
```

**Rationale**: production needs aggregation + alerting. OTel collector is the de-facto standard; vendor-neutral.

### §3.3 Remote/multi-tenant profile (NOT IMPLEMENTED)

Out of scope per H1.2 §10 + ROADMAP §OPS §103. Future work would add auth + rate-limit + redaction.

---

## §4 Alert thresholds (recommended)

Per ADR-0033 §4 + ADR-0004: every alert has a **runbook entry** (cross-references `OPS-support-playbook.md`).

| Metric | Threshold | Runbook | Severity |
|---|---|---|---|
| `health.status == unhealthy` | immediate | [§2 binary not start] OR [§5 schema migration] | page |
| `health.status == degraded` | 5 min sustained | [§4 MCP timeout] OR [§3 capture lost] | warn |
| `events_lost_rate > 5%` | 1 min sustained | [§3 capture lost] | page |
| `mcp_tool_latency_p99 > 30s` | 5 min sustained | [§4 MCP timeout] | warn |
| `process_rss > 80% of MemoryMax` | 1 min sustained | [§6 OOM kill] | warn |
| `process_oom_kills > 0` | immediate | [§6 OOM kill] | page |
| `disk_free_bytes < 100MB` | immediate | (operator-defined) | warn |
| `redb_writer_queue_full > 0` | immediate | [§3 capture lost] | warn |

---

## §5 Implementation status (OPS.4 deliverables)

| Deliverable | Status | Location |
|---|---|---|
| Health check contract (types only) | **DONE** | `crates/chronos-services/src/health_check.rs` (332L, 12 tests) |
| Health check HTTP endpoint | **NOT IMPLEMENTED** | operator's deployment decision (ADR-0033 §3) |
| Structured JSON logs | **PRE-EXISTING** | `tracing` + `tracing-subscriber` JSON formatter |
| OTLP endpoint (optional) | **PRE-EXISTING** | `tracing-opentelemetry` (compile-time optional feature) |
| OTel collector config template | **NOT INCLUDED** | operator's deployment decision |
| Metrics backend (Prometheus rules) | **NOT INCLUDED** | operator's deployment decision |
| Alert thresholds doc | **DONE** | this document, §4 |

**Net-new in OPS.4**: `health_check.rs` + this blueprint + the support playbook. No new collector / metrics / logs infrastructure (those are pre-existing or operator's responsibility).

---

## §6 Out of scope (deferred)

- **HTTP server implementation for `/health`**: per ADR-0033 §3, the contract is defined but the server is operator's choice. Operators can wire `axum`/`hyper`/`warp`/etc. against `HealthReport::from_components`.
- **Metrics emission**: Chronos emits via `tracing`; metrics via OTLP are opt-in. No Prometheus exposition endpoint.
- **Log shipping**: systemd journal + OTel collector cover this. No custom log shipper.
- **Distributed tracing across processes**: `chronos-mcp` is single-process per H1.2 §10. No cross-process spans.
- **PII redaction**: per H1.2 §10, no PII is logged. Redaction layer is moot.
- **Sampling rate configuration**: opt-in feature; not in this cycle.

---

## §7 References

- ADR-0004 — Silent Lie Prohibition (every metric has a meaning).
- ADR-0032 — OPS chapter scoping.
- ADR-0033 — OPS.4 support + telemetry (this document).
- `docs/runbooks/OPS-support-playbook.md` — alert thresholds cross-reference.
- `crates/chronos-services/src/health_check.rs` — health endpoint contract (types).
- `docs/architecture/H1.5-runtimes-capabilities-benchmarks.md` — perf budgets (telemetry alerts reference these).
- OpenTelemetry specification — OTLP wire format (https://opentelemetry.io/docs/specs/otlp/).
