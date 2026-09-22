# OPS-SCOPING — Production-ready por perfil: consolidación de H1.1.1 + H1.2 + H1.6 + H1.5 como foundation

> **Estado del slice (2026-09-22)**: docs-only scoping. NO se ha ejecutado ningún sub-cycle de OPS en `main @ 43bca7af`. Foundation pre-existente **sustancial** identificada — H1.1.1 supply-chain + H1.2 threat-model + H1.6 install/upgrade/rollback + H1.5 runtime/capability matrix cubren OPS.1..OPS.8 parcialmente. Scoping propuesto con 4 sub-cycles OPS.2..OPS.5 + OPS.1 inventory.

## §1 ROADMAP §OPS §101-§103 ref

ROADMAP §OPS §101-§103 — "OPS — Production-ready por perfil, no como eslogan general. Definir primero perfiles `local/stdio`, `Linux privileged capture` y cualquier futuro `remote/multi-tenant` **por separado**. Checklist OPS.1–OPS.8: amenaza/acceso, supply chain/SBOM, aislamiento y secretos, límites y rendimiento, backup/restore y schema migration, telemetry y diagnóstico, instalación/upgrade/rollback, soporte y respuesta a incidentes. Publicar solo el perfil que alcance CERT-4 con pruebas y artefactos del mismo commit/release."

## §2 Estado actual del repo (inspección directa)

`main @ 43bca7af` tiene **foundation pre-existente sustancial** para OPS. NO es "construir desde cero"; es "consolidar perfiles de deployment + certification gates". Inventario:

### §2.1 H1.1.1 Supply-chain baseline

`docs/security/H1.1.1-supply-chain-baseline.md` (305L, 9 secciones):
- `deny.toml` (90L, cargo-deny 0.20.x: licenses/sources/bans/advisories)
- `scripts/generate_chronos_sbom.py` (240L, CycloneDX 1.6 generator con fallback sintético)
- `.github/workflows/supply-chain.yml` (113L, CI job licencias+sources+bans+advisories+SBOM)
- 14/19 crates ganan `license.workspace = true`
- SBOM CycloneDX 1.6 con 47 componentes dedup'd + 19 per-crate JSONs
- 9 advisories en `reqwest 0.11.27` chain documentados
- 17 duplicate transitive crates documentadas

Cubre **OPS.2 (supply chain/SBOM)**.

### §2.2 H1.2 Threat model

`docs/security/H1.2-threat-model.md` (338L, 10 secciones):
- 3 deployment profiles documentados: `local/stdio`, `Linux privileged`, `remote/multi-tenant NO IMPLEMENTADO`
- Surface enumeration (qué está expuesto)
- Threat catalogue T-01..T-07
- OPS.1..OPS.8 checklist por perfil
- Systemd unit de referencia
- 9 `CapXxx` items con target phase

Cubre **OPS.1 (amenaza/acceso)** + base para **OPS.3 (aislamiento y secretos)**.

### §2.3 H1.6 Install/Upgrade/Rollback runbook

`docs/runbooks/H1.6-install-upgrade-rollback.md` (389L, 8 secciones):
- Install matrix (cargo install / container / prebuilt M1+ / dev)
- Artifact verification (4 layers: Cargo.lock SHA, tag peel, binary SHA-256, runtime smoke)
- Upgrade paths (patch/minor/major/canary)
- Rollback paths (known-bad / data-corruption / capability-loss)
- Schema-version compatibility (`schema_version = 1` cited at 16 real sites across 4 files)
- 10-item support bundle + RUST_LOG tips
- 4 proactive gap captures

Cubre **OPS.7 (instalación/upgrade/rollback)** + base para **OPS.5 (backup/restore y schema migration)**.

### §2.4 H1.5 Runtime/Capability matrix

`docs/architecture/H1.5-runtimes-capabilities-benchmarks.md` (260L, 8 secciones):
- 3 capability slots `ebpf-uprobe`/`ptrace-attach`/`browser-probe` matrix
- Performance budgets (6 metas p95 con tolerancia first-cycle)
- 4 OPEN follow-ups memoria/perturbación/replay/captura

Cubre **OPS.4 (límites y rendimiento)** base.

### §2.5 Documentación adicional

- `docs/runbooks/` (varios runbooks)
- `.github/workflows/*.yml` (7 GitHub Actions workflows, todos con `dtolnay/rust-toolchain@stable`)
- `Dockerfile` + `docker-compose.yml`
- `bin/chronos-mcp.rs:24-34` (try_new fail-closed)

### §2.6 Lo que falta (gaps identificados)

H1.2 threat model + H1.6 runbook + H1.1.1 supply-chain son docs comprehensivos pero:

1. **No ejecutables como certification gate**: cada perfil (`local/stdio`, `Linux privileged`) requiere **certificación end-to-end** que produzca evidencia verificable. H1.2 lista OPS.1..OPS.8 por perfil pero no hay `run_cert.sh` o equivalente.
2. **No telemetry pipeline real**: H1.2 §10 menciona telemetry/diagnóstico pero no hay wiring concreto (no OpenTelemetry collector configurado, no log shipping, no health check endpoint documentado).
3. **No support runbook**: H1.6 cubre install/upgrade/rollback pero no "qué hacer cuando el binario falla en producción". Falta un support-response playbook.
4. **Certificación tier no conectado a profiles**: H1.2 menciona OPS.1..OPS.8 pero no hay mapping `profile → certification tier (CERT-1..CERT-4) → release gate`.
5. **Multi-tenant profile NO IMPLEMENTADO**: H1.2 explicito que NO está implementado en este release. ROADMAP §OPS §103 dice "remote/multi-tenant NO IMPLEMENTADO" — es scope futuro, NO M-OPS actual.

## §3 Sub-cycles propuestos OPS.2..OPS.5

Cada sub-cycle entrega capacidad verificable + tests incrementales (per ADR-0004). Patrón seguido: m8-01..m8-06, M9.2..M9.6, M10.2..M10.6, M11.2..M11.6.

| Sub-cycle | Scope | Deliverables | Tests |
|---|---|---|---|
| **OPS.1** (inventory) | ESTE slice | OPS-SCOPING.md + ADR-0032 foundation inventory (incluye H1.1.1 + H1.2 + H1.6 + H1.5 = ~1,292 LoC docs + scripts/workflow pre-existentes) | (ADR docs-only) |
| **OPS.2** | Certification tier por perfil + ejecutable | `chronos-services::certification::CertificationTier` enum (CERT-1..CERT-4) + `run_cert.sh` ejecutable por profile (`local/stdio`, `Linux privileged`); sandbox test ejecutando cert contra el binario actual; gap analysis report `evidence/ops/cert-report.json` | +10 unit + 3 sandbox + 1 shell test |
| **OPS.3** | OPS.1..OPS.8 evidence per profile | Por cada OPS.1..OPS.8: evidencia ejecutable (tests + receipts + artefactos). H1.2 ya documenta el checklist; OPS.3 genera evidencia concreta. Output: `evidence/ops/ops.{1..8}.{local-stdio,linux-privileged}.json` (16 archivos). | +8 unit + 2 integration |
| **OPS.4** | Support runbook + telemetry health check | Support-response playbook (qué hacer cuando falla); health check endpoint documentado (no implementado, solo contrato); telemetry pipeline blueprint (OTel collector config + log shipping); ADR (OPS) con findings | +6 unit + 2 sandbox |
| **OPS.5** (close) | OPS chapter close + tag | full T1+T2/T3 sobre `main`; close report `docs/milestones/OPS-CLOSE.md`; tag `ops-production-ready.0`; ROADMAP §OPS §101-§103 con check de cierre; STATE + JOURNAL actualizados; remote/multi-tenant explícitamente marcado como **NO IMPLEMENTADO en este release** (per H1.2 + ROADMAP) | T1+T2+T3 |

**Rationale para 4 sub-cycles (OPS.2..OPS.5)**: sigue el patrón. OPS.5 es close-of-record. Foundation masiva (H1.1.1 + H1.2 + H1.6 + H1.5 = ~1,292 LoC docs + scripts + workflows) reduce scope de OPS.2 (certification tier scaffolding, NO construcción desde cero) + OPS.3 (evidence generation, los tests base ya existen).

## §4 Architecture decisions

### §4.1 D1: Per-profile certification, NO single global cert

**Decisión**: Certification tier se evalúa per-profile (`local/stdio`, `Linux privileged`). Un perfil puede ser CERT-3 mientras otro es CERT-1. NO se certifica el proyecto como un todo.

**Rationale**: ROADMAP §OPS §101-§103 explicito: "Definir primero perfiles... por separado". Un binario que funciona en `local/stdio` puede fallar en `Linux privileged` (eBPF no carga sin CAP_BPF).

### §4.2 D2: Reusar H1.2 OPS.1..OPS.8 checklist como base

**Decisión**: OPS.3 genera evidencia para los OPS.1..OPS.8 que H1.2 §6 ya documenta. NO introducir checklist nuevo.

**Rationale**: H1.2 es comprehensivo (338L, 7 amenazas + 8 OPS checks + 3 profiles). Reemplazar = mentira arquitectónica.

### §4.3 D3: H1.1.1 + H1.6 son source-of-truth de artefactos

**Decisión**: OPS.3 referencia `deny.toml` + `scripts/generate_chronos_sbom.py` + `.github/workflows/supply-chain.yml` (H1.1.1) y `Cargo.lock` sha256 + tag peel (H1.6) como artefactos canónicos. NO duplicar ni regenerar.

**Rationale**: Foundation ya existe; consolidar es honestidad.

### §4.4 D4: remote/multi-tenant NO IMPLEMENTADO (per H1.2 + ROADMAP)

**Decisión**: OPS.5 close marca `remote/multi-tenant` explícitamente como **NOT IMPLEMENTED en este release** en el close report. NO scope-creep a multi-tenant.

**Rationale**: ROADMAP §OPS §103 + H1.2 §10 ambos explicitos. ADR-0004: "Una plataforma no certificada se anuncia como experimental o unsupported, no como equivalente a otra".

### §4.5 D5: Telemetry pipeline blueprint, NO implementation

**Decisión**: OPS.4 entrega blueprint de telemetry (OTel collector config + log shipping + health check contract). NO implementa el collector en este ciclo.

**Rationale**: Telemetry infra es scope meta — depende de deployment del operador. Blueprint sentando bases, no imponiendo solución.

### §4.6 D6: Support runbook accionable, no exhaustivo

**Decisión**: OPS.4 support runbook cubre los 5 casos más comunes (binario no inicia, captura perdida, MCP timeout, schema migration falla, OOM). NO cubrir casos raros.

**Rationale**: Runbook útil > runbook completo. 80/20.

## §5 Risks

### §5.1 R1: `local/stdio` cert puede fallar en este host

**Riesgo**: OPS.2 ejecutando cert contra el binario actual puede revelar gaps (e.g., Cargo.lock sha drift).

**Mitigación**: Cert reporta gaps honestamente; OPS.3 genera evidencia para los gaps resolubles; gaps irresolubles quedan documentados en el close report.

### §5.2 R2: `Linux privileged` cert requiere CapEff > 0

**Riesgo**: H1.5 documenta que este host tiene CapEff=0 + no Chrome. Cert de `Linux privileged` puede ser imposible localmente.

**Mitigación**: Cert reporta `unsupported` honesto; close report documenta "este perfil requiere host con privilegios elevados, certificación pospuesta a environment con CapEff > 0".

### §5.3 R3: H1.2 OPS checklist es comprehensivo pero parcial

**Riesgo**: H1.2 lista OPS.1..OPS.8 pero algunos items no tienen artefacto ejecutable concreto (e.g., "telemetry pipeline" sin blueprint detallado).

**Mitigación**: OPS.4 introduce blueprints para los gaps identificados; OPS.5 documenta qué queda explícitamente como gap vs entrega.

### §5.4 R4: Support runbook puede quedar stale

**Riesgo**: Support runbook documenta comportamiento actual; si el binario cambia, el runbook queda obsoleto.

**Mitigación**: Runbook linked-to-version via tag sha256 + commit reference; M1+ cycle puede re-validar runbook contra binario actual.

### §5.5 R5: OPS push dependencies (M9/M10/M11 certified)

**Riesgo**: ROADMAP §OPS implícito depende de M9/M10/M11 certified para dar telemetry data real. Si esos capítulos NO están CLOSED, OPS telemetry queda blueprint sin data.

**Mitigación**: OPS.4 telemetry blueprint es forward-compatible; funciona con o sin M9/M10/M11 CLOSED.

## §6 Mapping to OPS.1..OPS.8

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

OPS.1..OPS.5 cubiertos por OPS.3 evidence generation. OPS.6-8 parcialmente cubiertos por H1.2 + H1.6, complementados por OPS.4.

## §7 Out-of-scope (OPS chapter)

1. **`remote/multi-tenant` profile implementation** (NO IMPLEMENTADO per H1.2 §10 + ROADMAP §OPS §103; scope futuro).
2. **Telemetry collector deployment** (OPS.4 entrega blueprint, NO collector real; depende del operador).
3. **Custom health check endpoint implementation** (OPS.4 documenta contrato; NO endpoint real en este ciclo).
4. **Auto-update mechanism** (H1.6 cubre upgrade paths, NO auto-update).
5. **Multi-region replication** (single-region only; multi-region es scope futuro).
6. **Disaster recovery automation** (H1.6 cubre rollback paths, NO DR automation).
7. **Replace H1.1.1/H1.2/H1.6/H1.5** (foundation consolidada, NO reemplazada).
8. **Public status page / SLA commitments** (operacional, no API).

## §8 References

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
- ADR-0004 (no falsear una entrega) — aplicado en §4 + §5 + D4.
