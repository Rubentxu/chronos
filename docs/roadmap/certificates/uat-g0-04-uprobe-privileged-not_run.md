# Certificate — UAT-G0-04-uprobe-privileged-not_run

**Capacidad:** UAT-G0-04 — Capturar e instalar/detener un uprobe real en host privilegiado y verificar liberación de recursos.
**Perfil:** `privileged` (REQUIRED, NO EJECUTADO en este entorno).
**SHA validado:** n/a — UAT no ejecutado.
**Tag remoto:** `v0.7.112` peel `0be2ec2d53d9698956ae705938b32b80d7365ad7` (intacto).
**Fecha:** 2026-09-21T16:21Z.
**Propietario:** AGENT (modo AUTO). **Estado:** `not_run`.

## Aserción observable (del UAT_CATALOG.md)

> Capturar e instalar/detener un uprobe real en host privilegiado y verificar liberación de recursos. Sin privilegios -> `blocked`; no simular el resultado.

## Estado

**`not_run`** (no `blocked` y no `failed`).

## Razón

Este entorno no tiene privilegios ptrace + eBPF requeridos para ejecutar la captura real de uprobe:

- `chronos-mcp` no es setuid root; el binario corre con el UID del usuario actual.
- `kernel.yama.ptrace_scope` en este host no permite ptrace de procesos arbitrarios.
- Las capabilities `CAP_SYS_PTRACE`/`CAP_BPF` no están asignadas al binario.

El smoke G0.4 (`/tmp/g0.4-wire-smoke/`) confirma esto: cuando invoca `events_read`, `session_status` polling × 25 devuelve `events=[]` porque `probe_inject` no puede ejecutar (sin privilegios) y por tanto el ExecutionLog permanece vacío.

## Política aplicada

Per `docs/roadmap/CERTIFICATION.md §3.2`: "Nunca `skip`, `#[ignore]` o mocks como sustituto de la UAT real. La separación por privilegios es explícita y exige una plataforma donde ejecutar T5 antes de certificar el backend."

→ Este UAT queda como `not_run` y **no se promueve a passed**. La capacidad `uprobe-real` queda explícitamente NO certificada en este perfil/host.

## Acción de recuperación / propietario

- **Próximo paso (M1+, fuera de scope G0):** ejecutar este UAT en un host Linux con ptrace+eBPF habilitado, root o capabilities CAP_SYS_PTRACE+CAP_BPF, kernel ≥ 5.8 (BTF + libbpf moderno).
- **Acción inmediata:** ninguno. El estado `not_run` es el resultado honesto y se publica explícitamente.
- **Propietario:** cualquier agente M1+ que opere un host privileged o el equipo de operaciones que designe CI privileged runner.

## Tests indirectos (no son evidencia para este UAT, pero confirman el contrato base)

- `tests/observe_uprobe.rs::test_observe_uprobe_against_nonexistent_session_returns_typed_error` — verifica que el wrapper `observe` devuelve error tipado contra sesión inexistente. NO cubre la captura real.
- DEBT-M7-02-01: 4 `probe_inject` legacy tests pre-C5.2 fallan porque esperan prefijo v1 (`probe_inject: capability: ebpf-uprobe`) y el wrapper actual emite `observe: probe still starting up`. Esta deuda documenta que el contrato real uprobe NO está validado en el CI base.

## Fecha / condición de recertificación

Recertificar cuando:

1. Exista un host Linux con ptrace+eBPF + capabilities CAP_SYS_PTRACE+CAP_BPF o root.
2. Exista un binario `chronos-mcp` instalado con esas capabilities (o setuid root).
3. El CI/GitHub Actions incluya un job privileged que ejecute este UAT.
4. Recertificación programada al menos cada release candidate.

## Historial

- 2026-09-21T14:52Z (G0.4 ver): reconocido como `not_run` per directiva.
- 2026-09-21T16:21Z (G0.6 recertificación): re-confirmado `not_run` — el host actual no tiene privilegios ptrace/eBPF.
