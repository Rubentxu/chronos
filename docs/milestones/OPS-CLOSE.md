# OPS close report

**Branch:** `main` at `0d68cac7` (post OPS.5 close-of-record)
**Cycle:** OPS (Production-ready per profile) — **CLOSED** 2026-09-22
**Tag:** `ops-production-ready.0` (signed, annotated)
**Precedence:** `docs/milestones/M9-CLOSE.md` §1; `docs/milestones/OPS-SCOPING.md`; ADR-0032; ADR-0033
**Status:** COMPLETE — 2026-09-22 (5/5 sub-ciclos verified)

## §1 Executive summary

The OPS sub-cycle ships the **production-ready per profile** surface for Chronos: agents can run Chronos in either `local/stdio` or `linux-privileged` profile with a known certification tier, a per-profile evidence dossier, an actionable support playbook, and a forward-compatible telemetry contract. Five sub-cycles (OPS.1 through OPS.5) deliver:

- **1 executable certification runner** (`scripts/run_cert.sh` + `scripts/test_run_cert.sh`).
- **1 typed Rust aggregate view** (`crates/chronos-services/src/ops_evidence.rs`).
- **16 per-profile evidence files** (`evidence/ops/ops.{1..8}.{local-stdio,linux-privileged}.json`).
- **1 support playbook** (`docs/runbooks/OPS-support-playbook.md`, 5 cases).
- **1 telemetry blueprint** (`docs/runbooks/OPS-telemetry-blueprint.md`).
- **1 health check contract** (`crates/chronos-services/src/health_check.rs`, 12 unit tests).
- **2 ADRs** (OPS scoping `0032` + OPS.4 support+telemetry `0033`).
- **Tag** `ops-production-ready.0` (signed).

After OPS, the OPS backlog reduces to:
- **`remote/multi-tenant` profile** (NOT IMPLEMENTED per H1.2 §10 + ROADMAP §OPS §103).
- **OTel collector / metrics backend / Prometheus rules** (operator's deployment decision).
- **HTTP server implementation for `/health`** (operator's choice of axum/hyper/warp/actix).
- **Authentication + rate-limit + redaction** (remote/multi-tenant only).
- **Exhaustive support playbook** (5-case 80/20 per ADR-0033 §2.1; new entries added as failures emerge).

## §2 Cycle log

| Cycle | Scope | Commit | LoC (delta) | Tests (delta) |
|---|---|---|---|---|
| **OPS.1** | ADR-0032 inventory + ROADMAP §OPS marked SCOPED | `8101f812` | 0 (docs) | 0 |
| **OPS.2** | `scripts/run_cert.sh` executable cert tier runner + `test_run_cert.sh` 5 tests + 2 cert reports | `b38add22` | 343 + 129 + 2 reports | 5 shell |
| **OPS.3** | 16 per-profile evidence files + `aggregate_ops_evidence.sh` + `ops_evidence.rs` typed Rust view | `9f287312` | 152 + 192 + 16 JSON | 8 |
| **OPS push** | 100-commit FF-puro sync with `origin/main @ 4796fddc` | `4796fddc` | 0 | 0 |
| **OPS.4** | `health_check.rs` contract + support playbook + telemetry blueprint + ADR-0033 | `802baf1a` | 332 + 235 + 177 + 158 | 12 |
| **OPS.5** | 4 evidence files updated (ops.6+ops.8 → pass) + aggregate re-run + this close report + tag `ops-production-ready.0` | `<this-commit>` | 4 JSON edits + close report | 0 |

Each cycle FF-merged (or post-write for docs-only) to `main` immediately. No PRs (per project convention).

## §3 Final certification tiers

| Profile | Pass | Warn | Fail | Tier | Status |
|---|---|---|---|---|---|
| `local-stdio` | **8/8** | 0 | 0 | **cert-4-production** | production-ready |
| `linux-privileged` | **7/8** | 1 | 0 | **cert-3-certified** | certified (not production-grade; OPS.3 isolation/secretos warn requires CAP_SYS_ADMIN to verify) |

OPS.3 linux-privileged `warn` is structural: the linux-privileged profile isolation/secrets verification requires `CAP_SYS_ADMIN` to read certain `/proc` paths, which is not available in CI sandbox. The evidence file documents this as a known gap; production deployment with CAP_SYS_ADMIN can verify manually and update the evidence file.

## §4 Per-profile OPS checklist

Per ADR-0032 §6 + ADR-0033 §2 mapping, each OPS check has an evidence file per profile:

| Check | local-stdio | linux-privileged |
|---|---|---|
| **OPS.1** amenaza/acceso | pass | pass |
| **OPS.2** supply-chain/SBOM | pass | pass |
| **OPS.3** aislamiento/secretos | pass | warn (CAP_SYS_ADMIN) |
| **OPS.4** límites/rendimiento | pass | pass |
| **OPS.5** backup/restore/schema | pass | pass |
| **OPS.6** telemetry/diagnóstico | pass (post-OPS.4) | pass (post-OPS.4) |
| **OPS.7** instalación/upgrade/rollback | pass | pass |
| **OPS.8** soporte/incidentes | pass (post-OPS.4) | pass (post-OPS.4) |

## §5 Telemetry + support (post-OPS.4)

| Artefact | Path | Purpose |
|---|---|---|
| Support playbook | `docs/runbooks/OPS-support-playbook.md` | 5-case actionable runbook |
| Telemetry blueprint | `docs/runbooks/OPS-telemetry-blueprint.md` | Wire contracts + alert thresholds |
| Health check contract | `crates/chronos-services/src/health_check.rs` | `HealthStatus` + `ComponentHealth` + `HealthReport` types |
| ADR-0033 | `docs/chronos-agentic-reconstruction/docs/adr/0033-ops-support-telemetry.md` | Design choices (contract only, 5-case playbook, blueprint) |

## §6 Out-of-scope (deferred)

- **`remote/multi-tenant` profile** (NOT IMPLEMENTED per H1.2 §10 + ROADMAP §OPS §103).
- **OTel collector / metrics backend / Prometheus rules** (operator's deployment decision).
- **HTTP server implementation for `/health`** (operator's choice; `HealthReport` JSON shape is the contract).
- **Authentication + rate-limit + redaction** (remote/multi-tenant only).
- **Exhaustive support playbook** (5-case 80/20 per ADR-0033 §2.1).
- **Auto-generated metrics** (`tracing` + optional OTLP; Prometheus exposition not in lib).
- **PII redaction** (no PII logged per H1.2 §10; moot).
- **Cross-process distributed tracing** (single-process per H1.2 §10).

## §7 OPS chapter close declaration

**OPS chapter is CLOSED** (5/5 sub-ciclos verified, tag signed):

| Sub-cycle | Status | Commit | Tier impact |
|---|---|---|---|
| OPS.1 ADR-0032 inventory | `verified` | `8101f812` | scope + architecture |
| OPS.2 cert tier runner | `verified` | `b38add22` | cert-3 (local-stdio) + cert-2 (linux-privileged) initial |
| OPS.3 per-profile evidence | `verified` | `9f287312` | cert-3 (both profiles) |
| OPS.4 support + telemetry | `verified` | `802baf1a` | cert-3 (both profiles, OPS.6 + OPS.8 upgraded warn→pass) |
| OPS.5 close-of-record | `verified` | `<this-commit>` | **cert-4 (local-stdio)** + **cert-3 (linux-privileged)** |

**H1.x + M4-F0 + M4-F1 + M6 (7/7) + M7 (4/4) + M8 (1/1) + M9 (5/5) + M10 (1/1) + M11 (1/1) + OPS (5/5) verificados: 54/54 sub-ciclos.**

The OPS chapter covers: certification tier executable (OPS.2) + per-profile evidence files (OPS.3) + support playbook + telemetry blueprint + health check contract (OPS.4) + close-of-record + tag (OPS.5).

## §8 Tag

```
git tag -s ops-production-ready.0 -m "OPS chapter closed: cert-4 (local-stdio) + cert-3 (linux-privileged)"
```

The tag points at the `main` HEAD after this close-of-record commit. It marks the first release where both deployment profiles have a known certification tier + actionable support playbook + forward-compatible telemetry contract.

## §9 Verification summary

| Stage | Command | Result |
|---|---|---|
| T0 build (incremental) | `cargo build -p chronos-services --lib` | Finished (post-OPS.4) |
| T1 unit (full) | `cargo test -p chronos-services --lib --no-fail-fast` | **473/473 PASS** (5.73s) |
| T1 unit (health_check) | `cargo test -p chronos-services --lib health_check --no-fail-fast` | **12/12 PASS** (0.00s) |
| T0 clippy | `cargo clippy -p chronos-services --lib --tests --no-deps -- -D warnings` | exit=0 (0 warnings) |
| Aggregate | `bash scripts/aggregate_ops_evidence.sh` | local-stdio cert-4, linux-privileged cert-3 |
| Push | `git push origin main` | `eebfc44a..0d68cac7` + tag `ops-production-ready.0` |
| HEAD == origin/main | `git rev-parse HEAD` vs `git rev-parse origin/main` | both `0d68cac7` |
| Tag intact | `git rev-parse v0.7.112` | `0be2ec2d` intact |
| New tag | `git rev-parse ops-production-ready.0` | points to `0d68cac7` |
| 4 SHAs cat-file | `git cat-file -e 802baf1a + 0d68cac7 + eebfc44a + 9f287312` | exit=0 |

## §10 Files

| File | Role |
|---|---|
| `crates/chronos-services/src/ops_evidence.rs` | Typed Rust aggregate view (OPS.3) |
| `crates/chronos-services/src/health_check.rs` | Health check contract (OPS.4) |
| `crates/chronos-services/src/lib.rs` | Module registrations |
| `scripts/run_cert.sh` + `scripts/test_run_cert.sh` | Cert tier runner (OPS.2) |
| `scripts/aggregate_ops_evidence.sh` | Evidence aggregator (OPS.3) |
| `evidence/ops/ops.{1..8}.{local-stdio,linux-privileged}.json` | 16 evidence files |
| `evidence/ops/aggregate.json` | Computed aggregate |
| `evidence/ops/cert-report.{json,linux-privileged.json}` | Cert reports (OPS.2) |
| `docs/runbooks/OPS-support-playbook.md` | Support playbook (OPS.4) |
| `docs/runbooks/OPS-telemetry-blueprint.md` | Telemetry blueprint (OPS.4) |
| `docs/runbooks/H1.6-install-upgrade-rollback.md` | Install/upgrade/rollback (pre-existing) |
| `docs/chronos-agentic-reconstruction/docs/adr/0032-ops-scoping-production-ready.md` | OPS scoping ADR |
| `docs/chronos-agentic-reconstruction/docs/adr/0033-ops-support-telemetry.md` | OPS.4 ADR |
| `docs/roadmap/STATE.md` | OPS chapter state (5/5 verified) |
| `docs/roadmap/JOURNAL.md` | Append-only OPS entries (OPS.1..OPS.5) |

## §11 References

- ADR-0004 — Silent Lie Prohibition (every gap is documented, no fake certification).
- ADR-0032 — OPS chapter scoping.
- ADR-0033 — OPS.4 support + telemetry.
- H1.1.1 — supply-chain baseline (SBOM, deny.toml).
- H1.2 §10 — operational guidance (extended by OPS support playbook).
- H1.5 — capability matrix + perf budgets (telemetry alerts reference these).
- H1.6 — install/upgrade/rollback runbook.
- ROADMAP §OPS §101-§103 — Production-ready per profile checklist.

## §12 Closing note

OPS is the **last chapter to ship an executable certification tier + per-profile evidence dossier**. The certification tier is a gate: `cert-4-production` means "ship to production"; `cert-3-certified` means "certified for non-production use"; lower tiers are blockers.

**`local-stdio` reached cert-4-production** (8/8 pass). **`linux-privileged` is cert-3-certified** (7/8 + 1 warn) — the warn is structural (CAP_SYS_ADMIN dependency for OPS.3 isolation verification); production deployment can verify manually.

OPS is the **last "executable" chapter of the roadmap**. Future work on remote/multi-tenant profile (NOT IMPLEMENTED) is gated by H1.2 §10 + ROADMAP §OPS §103 and requires its own threat model + auth + transport security + redaction + isolation.
