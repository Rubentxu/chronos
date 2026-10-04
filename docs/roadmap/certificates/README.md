# Certificates Index

**Authoridad:** [docs/roadmap/CERTIFICATION.md](../CERTIFICATION.md) §4.
**Propósito:** registro central de todos los certificados emitidos por capacidad/perfil, enlazables desde STATE/JOURNAL/recertificación.
**Fecha de instauración:** 2026-09-21 (G0.6).

## Plantilla

Cada certificado es un archivo `.md` en este directorio con la convención:

```text
<ID>-<profile>.md
```

Donde:
- **`<ID>`** = identificador del requisito/hito/capacidad (e.g. `REC-C7`, `UAT-G0-01`, `UAT-H1-02`).
- **`<profile>`** = perfil de despliegue (`base`, `sandbox`, `privileged`, `remote`, etc.).

La plantilla mínima está definida en `CERTIFICATION.md` §4 e incluye:
- ID, alcance, versión, SHA, perfil, limitaciones.
- Niveles CERT-0..4 con razón y estado.
- Mapa UAT IDs → fixture/comando/duración/resultado.
- Pruebas negativas, seguridad, perf, recovery/rollback.
- Incompatibilidades, deuda residual, fecha de recertificación.

## Certificados emitidos

G0 chain (2026-09-21) + R6.0 (2026-10-04).

| ID | Perfil | Archivo | SHA base | Estado | Fecha emisión | Próxima recertificación |
|---|---|---|---|---|---|---|
| **REC-C7** | base | [REC-C7-base.md](REC-C7-base.md) | `afa14fd2` | CERT-0..2 `passed`; CERT-3..4 `not_run` | 2026-09-21T16:30Z | Al cambiar `crates/chronos-mcp/src/lib.rs`, `chronos-sandbox/src/client/tools.rs`, o `crates/chronos-services/src/output.rs:1587`; o bump de `rmcp`/`serde`/`serde_json`/`schemars`/`tokio` que afecte wire-shape. |
| **UAT-G0-01** | base | [uat-g0-01-events-read-kind-base.md](uat-g0-01-events-read-kind-base.md) | `afa14fd2` | `passed` | 2026-09-21T16:18Z | Al cambiar `EventsReadKind` enum o el handler que lee el discriminador. |
| **UAT-G0-02** | base | [uat-g0-02-observe-uprobe-base.md](uat-g0-02-observe-uprobe-base.md) | `afa14fd2` | `passed` | 2026-09-21T16:19Z | Al cambiar `ObserveVerb`/`ObserveScopeWire` o el wrapper `observe`. |
| **UAT-G0-03** | base | [uat-g0-03-cursor-gap-replay-base.md](uat-g0-03-cursor-gap-replay-base.md) | `afa14fd2` | `passed` | 2026-09-21T16:19Z | Al cambiar `TraceEvent`/`GetEventResponse`/`V2Query`/`V2Result` o el wire shape de `events_read`. |
| **UAT-G0-04** | privileged | [uat-g0-04-uprobe-privileged-not_run.md](uat-g0-04-uprobe-privileged-not_run.md) | n/a | `not_run` per directiva (env sin ptrace+eBPF) | 2026-09-21T16:21Z | Cuando exista host Linux con ptrace+eBPF + capabilities CAP_SYS_PTRACE+CAP_BPF. |
| **UAT-G0-05** | base | [uat-g0-05-ci-architecture-vault-base.md](uat-g0-05-ci-architecture-vault-base.md) | `afa14fd2` | `passed` local con 2 exclusiones (Tarpaulin, CI remoto) | 2026-09-21T16:21Z | Al cambiar código que invalide clippy/fmt; al bump `Cargo.lock`; al introducir nuevos CC al vault. |
| **UAT-M6-01** | base | [uat-m6-01-base.md](uat-m6-01-base.md) | `61efb0f0` | CERT-0..2 `passed`; CERT-3..4 `not_run` | 2026-10-04T06:12Z | Al cambiar `run_uat_m6_01`, `UatResult` o `otlp::correlation`; al añadir `cross_service_wire`; al haber collector OTLP real. |
| **UAT-M6-02** | base | [uat-m6-02-base.md](uat-m6-02-base.md) | `61efb0f0` | CERT-0..2 `passed`; CERT-3..4 `not_run` | 2026-10-04T06:12Z | Al cambiar `run_uat_m6_02` o la semántica de retry/idempotencia de `otlp::ingest`; al añadir `cross_service_wire`. |
| **UAT-M7-01** | base | [uat-m7-01-base.md](uat-m7-01-base.md) | `61efb0f0` | CERT-0..2 `passed`; CERT-3..4 `not_run` | 2026-10-04T06:12Z | Al cambiar `run_uat_m7_01`, `UatResult`/`UatResultWire` o `wire_version`; al cerrar una limitación de `M7-CLOSE.md`. |
| **UAT-M7-02** | base | [uat-m7-02-base.md](uat-m7-02-base.md) | `61efb0f0` | CERT-0..2 `passed`; CERT-3..4 `not_run` | 2026-10-04T06:12Z | Al cambiar `run_uat_m7_02`, `UatOutcome::UnknownUnsupported` o su contador de regiones; al cambiar `wire_version`. |
| **REC-C7 (recert R6.1)** | base | [REC-C7-base-r6-recert.md](REC-C7-base-r6-recert.md) | `5994843e` | CERT-0..2 `passed`; CERT-3..4 `not_run` | 2026-10-04T07:22Z | Al cambiar los contratos G0.1/G0.2/G0.4 o su wiring; al entrar o salir un tool de `tools/list`; al cambiar la toolchain local o la de CI. |
## Estado agregado (G0 chain)

**Total certificados:** 11
**`passed`:** 10 (REC-C7 con CERT-0..2, **su recertificacion R6.1 sobre `5994843e`**, UAT-G0-01/02/03/05, UAT-M6-01/02, UAT-M7-01/02 + UAT-G0-01 + UAT-G0-02 + UAT-G0-03 + UAT-G0-05 + UAT-M6-01 + UAT-M6-02 + UAT-M7-01 + UAT-M7-02)
**`not_run` per directiva:** 1 (UAT-G0-04 privileged)
**`failed`:** 0
**`blocked`:** 0

**Antiguedad del registro, y por que R6.0 existe.** Hasta el 2026-10-04 los seis certificados
emitidos colgaban todos de `afa14fd2` (2026-09-21) y el indice no habia seguido al ledger. Cuatro
de ellos tenian su trigger cumplido en `reconstruction-contracts.toml` desde el 2026-09-22
(`OTEL-001` y `DIFF-001` promoteidos a `verified`) y seguian sin emitir. **La asercion que
importa: el registro de certificacion es derivado, el ledger es la autoridad.** Cuando el
registro y el ledger discrepan, el ledger gana y el registro es deuda.

## Cobertura por UAT_CATALOG.md

| UAT ID | Estado certificado | Notas |
|---|---|---|
| UAT-G0-01 (events_read discriminator) | `passed` base | wire smoke + 9/9 contract tests |
| UAT-G0-02 (observe uprobe typed errors) | `passed` base | wire smoke + 2/2 negative tests |
| UAT-G0-03 (cursor/gap/replay) | `passed` base | wire smoke confirma wire shape C5.2 |
| UAT-G0-04 (uprobe real privileged) | `not_run` | env-locked; documentado en DEBT-G0-04 |
| UAT-G0-05 (CI/Arch/Vault same SHA) | `passed` base local | Tarpaulin + CI remoto excluidos per scope |
| UAT-M6-01 (correlación extremo a extremo) | `passed` base | 153/153 con el ejecutor; e2e por el cable MCP real. Fixture, sin collector OTLP en vivo. |
| UAT-M6-02 (procedencia y retry idempotente) | `passed` base | 153/163 shared suite; deriva conservada y retry con `trace_id` estable. |
| UAT-M7-01 (primera divergencia semántica) | `passed` base | 925/925; recibo JSON canónico reproducible byte a byte. |
| UAT-M7-02 (unknown/unsupported, sin igualdad falsa) | `passed` base | 925/925; recibo declara 2 regiones sin cobertura y `UnknownUnsupported`, no igualdad. |

## Exclusiones comunes a los certificados R6.0 (2026-10-04)

- `CHRONOS_CONTRACT_BASE_REF` sin fijar hace que `check_architecture_contracts.py` emita
  `WARN: ... skipping added-line legacy scan`. **El gate pasa, pero el escaneo de lineas anadidas
  no se ejecuto** en ninguna de las cuatro fichas. No se cuenta como cobertura.
- Los cuatro ejecutadores corren sobre **fixtures deterministas**, no sobre trazas reales de dos
  procesos, retencion real ni collector OTLP en vivo. Por eso CERT-3 es `not_run` en las cuatro y
  no `passed` con matices.
- `M6` no tiene adaptador de wire (a diferencia de `M7`), porque `UatResult` no deriva `serde` por
  diseno. Su recibo se emite en `Debug`; el de `M7` es el JSON canonico de producto.
- Recibo reproducible: `cargo run -p chronos-sandbox --example uat_receipt` (sonda sin
  aserciones; las aserciones viven en las suites existentes, no duplicadas aqui).

## Historico

| Fecha | Acción | Resultado |
|---|---|---|
| 2026-09-21T13:30Z | UAT-G0-01 emitido (G0.1 merge `0f773810`) | primer wire smoke GREEN |
| 2026-09-21T14:00Z | UAT-G0-02 emitido (G0.2 merge `13495fae`) | primer wire smoke GREEN |
| 2026-09-21T14:52Z | UAT-G0-03 emitido (G0.4 merge `b44504ed`) | wire shape C5.2 documentado |
| 2026-09-21T15:00Z | UAT-G0-05 emitido (G0.5 merge `a00845eb`) | gates locales verdes |
| 2026-09-21T16:21Z | UAT-G0-04 marcado `not_run` per directiva | env-locked reconocido |
| 2026-09-21T16:30Z | REC-C7 recertificado (G0.6 merge `ab420385`) | CERT-0..2 passed sobre `afa14fd2` |
| 2026-09-21T17:30Z | Este README.md creado (G0.7) | índice central enlazable |

## Próximos certificados a emitir (post-G0)

Estos NO se emiten hasta que un ciclo H1/M/M-OPS los materialice:

| ID potencial | Perfil | Trigger |
|---|---|---|
| UAT-H1-01 (reproducible build) | base | H1.1 cerrada (Cargo.lock + tooling) |
| UAT-H1-02 (permisos ejecución) | base | H1.2 cerrada (threat model) |
| UAT-H1-03 (recovery/restart) | sandbox | H1.3 cerrada (contract tests) |
| UAT-M4G-01..02 (Go instrumentation) | sandbox | M4-F0 cerrada |
| UAT-M6-01..02 (OTLP) | base | M6 cerrada |
| UAT-M7-01..02 (differential) | base | M7 cerrada |
| UAT-M8-01..02 (counterexample) | base | M8 cerrada |
| UAT-OPS-01..08 (production-ready per profile) | privileged/remote | OPS cerrada |

## Cómo añadir un nuevo certificado

1. Identificar el ID canónico del requisito (e.g. `UAT-M4G-01`).
2. Identificar el perfil (`base`, `sandbox`, `privileged`, `remote`).
3. Copiar la plantilla mínima de `CERTIFICATION.md` §4.
4. Crear `<ID>-<profile>.md` con la plantilla rellena, citando SHA, fechas y evidencia observable.
5. Actualizar la tabla "Certificados emitidos" arriba con SHA base, estado, fecha.
6. Si el UAT está en `UAT_CATALOG.md`, marcar el row como "cert emitido" en ese catálogo.
7. Si el certificado implica un cycle, crear los cycle artifacts (`exploration-report.md`, `apply-checkpoint.json`, `verify-findings.json`, `release-receipt.md`) en `cycle-artifacts/p-<project-id>/<cycle-id>/`.

## Política de recertificación

Un certificado NO se sobrescribe: se emite uno nuevo con fecha posterior cuando cambia una de las condiciones de recertificación listadas en su ficha. La transición se registra en la sección "Historial" del propio certificado.

Los certificados `not_run` NO se promueven a `passed` sin evidencia; permanecen `not_run` hasta que la condición de plataforma se cumpla.
