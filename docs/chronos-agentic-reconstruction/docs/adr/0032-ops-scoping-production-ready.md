# ADR-0032 — OPS chapter scoping ADR: Production-ready per profile over H1.1.1 + H1.2 + H1.6 + H1.5 foundation

**Cycle:** OPS / OPS.0-prep (Production-ready per profile — formal scoping ADR before execution)
**Status:** `verified` post-write (docs-only; no code change; branch HEAD == `main @ 9d586ddf`)

---

## 1. Context

ROADMAP §OPS §101-§103 — "OPS — Production-ready por perfil, no como eslogan general. Definir primero perfiles `local/stdio`, `Linux privileged capture` y cualquier futuro `remote/multi-tenant` **por separado**. Checklist OPS.1–OPS.8: amenaza/acceso, supply chain/SBOM, aislamiento y secretos, límites y rendimiento, backup/restore y schema migration, telemetry y diagnóstico, instalación/upgrade/rollback, soporte y respuesta a incidentes. Publicar solo el perfil que alcance CERT-4 con pruebas y artefactos del mismo commit/release."

Tras M9 (ADR-0027 + ADR-0028 + ROADMAP refactor) + M10 (ADR-0029 + ROADMAP refactor) + M11 (ADR-0031 + ROADMAP refactor), OPS es el último capítulo "ejecutable" del ROADMAP. A diferencia de los anteriores (que tenían foundation masiva pre-existente), OPS tiene **foundation sustancial** pre-existente:

- **H1.1.1** `docs/security/H1.1.1-supply-chain-baseline.md` (305L) + `deny.toml` (90L) + `scripts/generate_chronos_sbom.py` (240L) + `.github/workflows/supply-chain.yml` (113L) — supply-chain + SBOM + license inheritance.
- **H1.2** `docs/security/H1.2-threat-model.md` (338L) — threat model + 3 deployment profiles + OPS.1..OPS.8 checklist + 7 threats T-01..T-07.
- **H1.6** `docs/runbooks/H1.6-install-upgrade-rollback.md` (389L) — install matrix + 4-layer artifact verification + upgrade/rollback paths + schema-version compatibility.
- **H1.5** `docs/architecture/H1.5-runtimes-capabilities-benchmarks.md` (260L) — runtime × capability matrix + 6 perf budgets + 4 OPEN follow-ups.

Total: ~1,292 LoC docs comprehensivos + scripts + workflows. ROADMAP §OPS §103 explicito: "Publicar solo el perfil que alcance CERT-4 con pruebas y artefactos del mismo commit/release" — certificación tier es el gate.

Este ADR formaliza la decisión arquitectónica de OPS, incluyendo qué se construye (certification tier executable + per-profile evidence + support runbook + telemetry blueprint), qué se reusa (H1.1.1 + H1.2 + H1.6 + H1.5), qué NO se hace (remote/multi-tenant NO IMPLEMENTADO), y cómo se subdivide en 5 sub-cycles verificables (OPS.1 inventory + OPS.2..OPS.4 execution + OPS.5 close).

## 2. Decision

OPS (Production-ready per profile) se construye como **5 sub-cycles verificables** (OPS.1 inventory + OPS.2..OPS.4 execution + OPS.5 close), consolidando **sobre** la foundation masiva pre-existente (H1.1.1 + H1.2 + H1.6 + H1.5 + scripts/workflows) — NO desde cero.

### §2.1 Foundation pre-existente (reusable)

**4 docs comprehensivos + artefactos** en `main @ 9d586ddf`, totalizando **~1,292 LoC docs + scripts + workflows**:

1. **H1.1.1 Supply-chain** (305L):
   - `deny.toml` (90L, cargo-deny 0.20.x: licenses/sources/bans/advisories)
   - `scripts/generate_chronos_sbom.py` (240L, CycloneDX 1.6 generator con fallback sintético)
   - `.github/workflows/supply-chain.yml` (113L, CI job licencias+sources+bans+advisories+SBOM)
   - 14/19 crates con `license.workspace = true`
   - SBOM CycloneDX 1.6 con 47 componentes dedup'd + 19 per-crate JSONs
   - 9 advisories en `reqwest 0.11.27` chain documentados

2. **H1.2 Threat model** (338L):
   - 3 deployment profiles: `local/stdio` (VERIFIED), `Linux privileged` (NOT IMPLEMENTED on CapEff=0 host), `remote/multi-tenant` (NO IMPLEMENTADO en este release)
   - Surface enumeration
   - 7 threats T-01..T-07 catalogued
   - OPS.1..OPS.8 checklist **per profile**
   - 9 `CapXxx` items con target phase
   - Systemd unit de referencia

3. **H1.6 Install/Upgrade/Rollback** (389L):
   - Install matrix (cargo install / container / prebuilt M1+ / dev)
   - 4-layer artifact verification (Cargo.lock SHA, tag peel, binary SHA-256, runtime smoke)
   - Upgrade paths (patch/minor/major/canary)
   - Rollback paths (known-bad / data-corruption / capability-loss)
   - Schema-version compatibility (`schema_version = 1` cited at 16 real sites across 4 files)
   - 10-item support bundle + RUST_LOG tips
   - 4 proactive gap captures

4. **H1.5 Runtime/Capability matrix** (260L):
   - 3 capability slots `ebpf-uprobe`/`ptrace-attach`/`browser-probe` matrix
   - 6 perf budgets p95 con tolerancia first-cycle
   - 4 OPEN follow-ups (memoria/perturbación/replay/captura)

### §2.2 Sub-cycles OPS.2..OPS.4 execution

| Sub-cycle | Scope | Deliverables | Tests |
|---|---|---|---|
| **OPS.2** | Certification tier executable | `chronos-services::certification::CertificationTier` enum (CERT-1 stub / CERT-2 partial / CERT-3 certified / CERT-4 production); `run_cert.sh` ejecutable per profile (`local/stdio`, `Linux privileged`); sandbox test ejecutando cert contra el binario actual; gap analysis report `evidence/ops/cert-report.json`. | +10 unit + 3 sandbox + 1 shell test |
| **OPS.3** | OPS.1..OPS.8 evidence per profile | Por cada OPS.1..OPS.8: evidencia ejecutable (tests + receipts + artefactos referenciando H1.2/H1.1.1/H1.6/H1.5). Output: `evidence/ops/ops.{1..8}.{local-stdio,linux-privileged}.json` (16 archivos). | +8 unit + 2 integration |
| **OPS.4** | Support runbook + telemetry blueprint | Support-response playbook (5 casos más comunes: binario no inicia, captura perdida, MCP timeout, schema migration falla, OOM); telemetry blueprint (OTel collector config + log shipping + health check contract documentado, NO implementado); ADR (OPS) con findings. | +6 unit + 2 sandbox |
| **OPS.5** (close) | OPS chapter close + tag | full T1+T2/T3 sobre `main`; close report `docs/milestones/OPS-CLOSE.md`; tag `ops-production-ready.0`; ROADMAP §OPS §101-§103 con check de cierre; STATE + JOURNAL actualizados; `remote/multi-tenant` explícitamente marcado como **NOT IMPLEMENTED en este release**. | T1+T2+T3 |

### §2.3 Naming convention

Tras M9.1 ADR-0027 §2.3 + ADR-0028 §2.4 + ADR-0029 §2.4 + ADR-0031 §2.3 + este ADR §2.3:

- **Vault cycles / storage refactors** usan prefijo `cc-m9-NN` o `vault-m9-NN`.
- **ROADMAP §M9 sub-cycles** usan prefijo `M9.N`.
- **ROADMAP §M10 sub-cycles** usan prefijo `M10.N`.
- **ROADMAP §M11 sub-cycles** usan prefijo `M11.N`.
- **ROADMAP §OPS sub-cycles** usan prefijo `OPS.N`.
- **OPS.1 inventory + este ADR** son docs-only sin tag.
- **OPS.2..OPS.4** son sub-cycles de ejecución; **OPS.5** es close-of-record con tag `ops-production-ready.0`.

## 3. Alternatives considered

Seis alternativas consideradas; una aceptada, cinco rechazadas.

### §3.1 Construir OPS desde cero (rechazado)

Ignorar H1.1.1 + H1.2 + H1.6 + H1.5; escribir nuevos docs OPS. **Por qué rechazada**: (a) duplica ~1,292 LoC comprehensivos + scripts + workflows; (b) introduce drift entre docs comprehensivos (H1.x) y ejecutables (OPS); (c) ROADMAP §0.4 violation (no reinventar abstracciones existentes); (d) ADR-0004 violation (claim "construimos OPS" cuando ya estaba construido como H1.x).

### §3.2 Single global certification (rechazado)

Certificar el proyecto entero, no por profile. **Por qué rechazada**: (a) ROADMAP §OPS §103 explicito: "Definir primero perfiles... por separado"; (b) `local/stdio` y `Linux privileged` tienen capability gaps distintos (eBPF requiere CAP_BPF); (c) per-profile cert es honestidad estructural.

### §3.3 Skip evidence generation (rechazado)

Confiar en H1.2/H1.6 docs como evidencia sin ejecutar. **Por qué rechazada**: (a) ROADMAP §OPS §103: "Publicar solo el perfil que alcance CERT-4 con pruebas y artefactos del mismo commit/release" — pruebas EJECUTABLES son requisito; (b) docs comprehensivos != evidencia ejecutable; (c) ADR-0004: docs describing ≠ capability verified.

### §3.4 Implement `remote/multi-tenant` (rechazado)

Construir multi-tenant profile en OPS chapter. **Por qué rechazada**: (a) ROADMAP §OPS §103 + H1.2 §10 ambos explicitos: NO IMPLEMENTADO en este release; (b) multi-tenant requiere TM separado + auth + transport security + redaction + isolation; (c) ADR-0004 violation (claim "multi-tenant ready" cuando NO está implementado).

### §3.5 Implement telemetry collector real (rechazado)

Construir OTel collector + log shipping + health check endpoint en OPS.4. **Por qué rechazada**: (a) telemetry infra es scope meta — depende de deployment del operador; (b) OPS.4 entrega blueprint forward-compatible; (c) collector real es per-deployment decision (no todos los operadores quieren OTel).

### §3.6 Consolidate over H1.x foundation + 4 execution sub-cycles + 1 close (aceptado)

Opción adoptada. Justificación:

1. **Honesta**: reconoce foundation ~1,292 LoC + scripts + workflows, no la reinventa.
2. **Per-profile certification**: aligns con ROADMAP §OPS §103 directive.
3. **Composable**: OPS.3 depends-on OPS.2 (certification tier); OPS.4 depends-on OPS.2 + OPS.3; OPS.5 depends-on all.
4. **Risk-managed**: per-profile cert (R1) + CapEff-aware reporting (R2) + blueprint vs implementation distinction (R5).
5. **ADR-0004 aligned**: remote/multi-tenant explícitamente NOT IMPLEMENTED; cert gaps documentados honestamente; telemetry como blueprint, no como claim de implementación.

## 4. Consequences

### §4.1 Positive

- **Scope reducido por foundation masiva**: ~1,292 LoC pre-existentes cubren OPS.1 (inventory comprehensivo) + base para OPS.2 (certification scaffolding) + OPS.3 (evidence generation referencing H1.x). 4 sub-cycles execution vs ~8+ si construyéramos desde cero.
- **Menos código nuevo**: ~1,000-1,500 LoC estimados para OPS.2..OPS.4 (vs ~3,000+ desde cero).
- **Más tests por menos código**: ratio tests/LoC > 0.5 mantenido.
- **Onboarding runbook**: 5-case support playbook (D6) — actionable, no exhaustivo.
- **Telemetry blueprint forward-compatible**: funciona con o sin M9/M10/M11 CLOSED (R5 mitigation).

### §4.2 Negative

- **`Linux privileged` cert puede ser imposible localmente**: este host tiene CapEff=0 + no Chrome (R2). Mitigation: cert reporta `unsupported` honesto.
- **`local/stdio` cert puede revelar gaps**: Cargo.lock sha drift, missing artifacts (R1). Mitigation: gap analysis report.
- **H1.2 OPS checklist comprehensivo pero parcial**: algunos items no tienen artefacto ejecutable concreto (R3). Mitigation: OPS.3 introduce blueprints para los gaps.
- **Support runbook puede quedar stale**: documentación desactualizada si binario cambia (R4). Mitigation: tag sha256 + commit reference linked-to-version.
- **OPS push dependencies**: ROADMAP §OPS implícito depende de M9/M10/M11 certified (R5). Mitigation: OPS.4 telemetry blueprint es forward-compatible.

### §4.3 Neutral

- **5 sub-cycles = 5 commits** en main (uno por slice) + 1 commit de close = 6 commits totales OPS.x.
- **Tests sandbox crecen** (OPS.2 +3, OPS.3 +2, OPS.4 +2 = +7 tests). Sandbox total pre-OPS: ~135 tests. Post-OPS: ~142.
- **ROADMAP §OPS §101-§103 NO se modifica**: este ADR ejecuta lo que ya está descrito.
- **16 evidence files generated** (8 checks × 2 profiles).
- **`remote/multi-tenant` NO IMPLEMENTED en este release** explícito en close report.

## 5. Verification evidence

Inspección directa sobre `main @ 9d586ddf` (post-OPS-SCOPING commit):

- **T0** `cargo clippy --workspace --all-targets --no-deps -- -D warnings` exit=0 (no se tocó código, ADR es docs-only).
- **T1** `cargo test -p chronos-mcp --lib --no-fail-fast` 84/84 PASS (no regresión post-ADR-0032).
- **`git cat-file -e 9d586ddf + 43bca7af + ec9a60d3`** exit=0; SHAs accesibles.
- **H1.1.1 verificada**: `docs/security/H1.1.1-supply-chain-baseline.md` (305L) + `deny.toml` (90L) + `scripts/generate_chronos_sbom.py` (240L) + `.github/workflows/supply-chain.yml` (113L).
- **H1.2 verificada**: `docs/security/H1.2-threat-model.md` (338L) con 3 profiles + OPS.1..OPS.8 checklist + 7 threats.
- **H1.6 verificada**: `docs/runbooks/H1.6-install-upgrade-rollback.md` (389L) con install matrix + 4-layer verification + upgrade/rollback.
- **H1.5 verificada**: `docs/architecture/H1.5-runtimes-capabilities-benchmarks.md` (260L) con capability matrix + perf budgets.
- **Workflows verificados**: 7 `.github/workflows/*.yml` con `dtolnay/rust-toolchain@stable`.
- **Container verificado**: `Dockerfile` + `docker-compose.yml`.
- **Total foundation**: ~1,292 LoC docs comprehensivos + scripts + workflows + 7 CI workflows.

## 6. Mapping to OPS.1..OPS.8

| OPS check | H1.x source | OPS sub-cycle |
|---|---|---|
| OPS.1 (amenaza/acceso) | H1.2 §2-§5 + T-01..T-07 | OPS.3 |
| OPS.2 (supply chain/SBOM) | H1.1.1 + deny.toml + SBOM script | OPS.3 |
| OPS.3 (aislamiento y secretos) | H1.2 §6-§7 | OPS.3 |
| OPS.4 (límites y rendimiento) | H1.5 perf budgets | OPS.3 |
| OPS.5 (backup/restore y schema migration) | H1.6 §5-§6 + schema_version tracking | OPS.3 |
| OPS.6 (telemetry y diagnóstico) | H1.2 §10 + (blueprint en OPS.4) | OPS.4 |
| OPS.7 (instalación/upgrade/rollback) | H1.6 §2-§5 | OPS.3 |
| OPS.8 (soporte y respuesta a incidentes) | (runbook en OPS.4) | OPS.4 |

OPS.1..OPS.5 cubiertos por OPS.3 evidence generation. OPS.6-8 complementados por OPS.4.

## 7. OPS chapter status

- **OPS.1**: `verified` (OPS-SCOPING.md slice previo + este ADR-0032).
- **OPS.2..OPS.4**: NOT STARTED en `main @ 9d586ddf`. Listos para ejecución tras OK operador.
- **OPS.5**: NOT STARTED (close-of-record condicional).

**Post-condición de OPS chapter CLOSED** (OPS.5 close):
- T1+T2+T3 verdes sobre `main`.
- Per-profile certification tier report (`local/stdio` y `Linux privileged`).
- 16 evidence files generated (8 checks × 2 profiles).
- Support runbook + telemetry blueprint committed.
- Close report `docs/milestones/OPS-CLOSE.md`.
- Tag `ops-production-ready.0` firmado apuntando al merge commit.
- ROADMAP §OPS §101-§103 con check mark de cierre + ref a OPS-CLOSE.
- STATE.md con sub-cycle rows prepended + "OPS chapter CLOSED (5/5)".
- **`remote/multi-tenant` explícitamente NOT IMPLEMENTED en este release** documentado.

## 8. Out-of-scope (OPS chapter)

1. **`remote/multi-tenant` profile implementation** (NO IMPLEMENTADO per H1.2 §10 + ROADMAP §OPS §103; scope futuro).
2. **Telemetry collector deployment** (OPS.4 entrega blueprint, NO collector real; depende del operador).
3. **Custom health check endpoint implementation** (OPS.4 documenta contrato; NO endpoint real en este ciclo).
4. **Auto-update mechanism** (H1.6 cubre upgrade paths, NO auto-update).
5. **Multi-region replication** (single-region only; multi-region es scope futuro).
6. **Disaster recovery automation** (H1.6 cubre rollback paths, NO DR automation).
7. **Replace H1.1.1/H1.2/H1.6/H1.5** (foundation consolidada, NO reemplazada).
8. **Public status page / SLA commitments** (operacional, no API).

## 9. References

- ROADMAP §OPS §101-§103 (`docs/ROADMAP.md`).
- MILESTONE_ACCEPTANCE.md §OPS (a crear en OPS.5).
- UAT_CATALOG.md §OPS (a crear en OPS.5; UAT-OPS-XX no definidos en ROADMAP §OPS — son derivados de OPS.1..OPS.8).
- ADR-0031 §7 — M11 chapter status (peer reference).
- `docs/security/H1.1.1-supply-chain-baseline.md` (305L, 9 secciones) — supply-chain + SBOM foundation.
- `docs/security/H1.2-threat-model.md` (338L, 10 secciones) — threat model + OPS.1..OPS.8 checklist + 3 profiles.
- `docs/runbooks/H1.6-install-upgrade-rollback.md` (389L, 8 secciones) — install/upgrade/rollback + support bundle.
- `docs/architecture/H1.5-runtimes-capabilities-benchmarks.md` (260L, 8 secciones) — runtime × capability matrix + perf budgets.
- `deny.toml` (90L) — cargo-deny 0.20.x policy.
- `scripts/generate_chronos_sbom.py` (240L) — CycloneDX 1.6 generator.
- `.github/workflows/supply-chain.yml` (113L) — CI supply-chain.
- `.github/workflows/*.yml` (7 workflows) — CI con `dtolnay/rust-toolchain@stable`.
- `Dockerfile` + `docker-compose.yml` — container deployment.
- `bin/chronos-mcp.rs:24-34` — try_new fail-closed.
- ADR-0004 (no falsear una entrega) — aplicado en §3 + §4 + D4 (remote/multi-tenant NOT IMPLEMENTED).
