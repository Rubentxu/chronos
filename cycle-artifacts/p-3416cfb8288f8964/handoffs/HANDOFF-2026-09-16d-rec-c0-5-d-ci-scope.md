# REC-C0.5-D — CI surface architecture decision

**Estado**: Pendiente decisión del usuario antes de cerrar REC-C0.
**Fecha**: 2026-09-16
**SHAs**: 34781fb2 (HEAD en `feat/rec-convergence-truth-gate`)

## Resumen

REC-C0.5-D cerró 3 fallos pendientes:

| Test | Estado pre | Acción | Estado post (CI 35074696707) |
|---|---|---|---|
| `ce1_shrink_constant_target` | FALLA en CI | b4fe4e8d: `probe_drain().len()+1` | **PASS** (CI confirmado) |
| `ce7_shrink_response_uses_new_saved_envelope` | FALLA en CI | b4fe4e8d: idem | **PASS** (CI confirmado) |
| `ce10_shrink_number_target_real_shrinking` | FALLA en CI | b4fe4e8d: idem | **PASS** (CI confirmado) |

**Ledger actualizado** en 34781fb2 con `CE-001` y reconciliación de cabeceras (38 = 20 privileged + 18 deferred).

## Problema: cierre de CI no llega a verde

Tras b4fe4e8d + 34781fb2, el run 35074696707 reporta:

```
Test (unit + doc tests)        — success
Test (integration, skip ignored) — failure (exit 101)
```

Los pasos 1-8 (fmt, clippy, build, unit) están todos verdes. El step 9 falla **únicamente** por DEF-001:

```
test_query_events_offset_beyond_total     ... FAILED (query_edge_cases.rs:58)
test_query_events_pagination_all_events   ... FAILED (query_edge_cases.rs:507)
```

DEF-001 está clasificado en `reconstruction-contracts.toml` con:
- `owner_gate = "REC-C1"`
- `reason = "events_read pagination/offset correctness; TRUTH-002/003 gap"`

Esto **no se puede arreglar en REC-C0**. TRUTH-002/003 son propiedad de REC-C1, que empieza **después** de fusionar PR #19.

## Diagnóstico arquitectónico

El criterio de cierre dice:

> "REC-C0 closure criterion: 4 mandatory workflows GREEN on remote CI (Architecture Contracts, CI, Coverage, Vault Drift Sweep)."

Pero el workflow `CI` actual ejecuta `cargo test --workspace --tests -- --test-threads=1`, que **incluye toda la matriz sandbox** — incluyendo el bucket deferred. El workflow CI no está scoped a la superficie mandatoria REC-C0; corre la superficie completa (mandatory + deferred) y falla cuando cualquier test deferred falla.

Esto viola el spec del propio REC-C0:

> "CI mandatory gate being GREEN must not lie — meaning not by hiding 35 failures, but because CI checks the deterministic surface REC-C0 declares, while other workflows check privileged/deferred explicitly"
>
> "Three explicit surfaces: Mandatory deterministic baseline (must be GREEN), Privileged UAT (real eBPF/ptrace with capabilities, never fake-pass), Deferred debt sentinel (knows exact tests, fails on new failures AND on obsolete waivers)"

El workflow CI actual **confunde las dos superficies** (mandatory y deferred) en un solo gate.

## Opciones para cerrar REC-C0 sin mentir

### Opción A — Split del workflow CI en mandatory + sentinel (recomendado)

1. **`ci.yml` — scope `Test (integration)` a la superficie mandatoria**: excluir los ficheros listados en `baseline_scope.deferred` (`query_edge_cases.rs`, `query_filters.rs`, `tripwire_tools.rs`).
2. **Crear `sandbox-debt-sentinel.yml`**: workflow explícito para la deuda deferred. Debe ejecutarse y **fallar** cuando los tests deferred fallen (es un sentinel — su fallo es señal de deuda viva, no bug).
3. Los 4 workflows mandatorios siguen siendo: Architecture Contracts, **CI** (scope mandatory), Coverage, Vault Drift Sweep.
4. Sentinel NO es mandatorio — es la **señal de deuda**.

**Pro**: alinea el spec del usuario con la realidad. La deuda deferred queda visible en un workflow dedicado en lugar de "romper CI".
**Contra**: cambia el `cargo test --workspace --tests` por scope — pero el sentinel workflow sigue ejecutando los deferred tests en su propio job, así que no se esconde nada.

### Opción B — Reemplazar "CI" por "Sandbox Mandatory Surface"

1. Dejar `ci.yml` tal cual (corre toda la matriz, falla por DEF-001).
2. Crear un nuevo workflow `sandbox-mandatory-surface.yml` que corre solo la superficie mandatoria.
3. Cambiar la lista de 4 mandatorios a: Architecture Contracts, **Sandbox Mandatory Surface**, Coverage, Vault Drift Sweep.
4. Sentinel separado.

**Pro**: el workflow CI queda intacto.
**Contra**: contradice el spec del usuario que explícitamente lista "CI" como mandatorio.

### Opción C — Aceptar CI rojo y cerrar REC-C0 igualmente

Documentar que el CI rojo es por DEF-001 (deuda REC-C1, no REC-C0) y declarar REC-C0 cerrado con 3/4 mandatorios verdes. PR #19 merge igual.

**Pro**: mínimo esfuerzo.
**Contra**: viola el criterio "4 mandatory GREEN" explícito del usuario.

## Recomendación: Opción A

Mantiene los 4 mandatorios exactos que el usuario pidió (Architecture Contracts, CI, Coverage, Vault Drift Sweep) y respeta los tres surfaces (mandatory, privileged, deferred). El sentinel workflow separado garantiza que la deuda deferred **no se esconde** — falla cuando debería fallar y emite una señal clara.

## Cambios concretos si se aprueba Opción A

1. **`ci.yml`**: cambiar `cargo test --workspace --tests -- --test-threads=1` por un comando que excluya `query_edge_cases.rs`, `query_filters.rs`, `tripwire_tools.rs` de la superficie mandatoria. Alternativa: usar `--test-threads=1` y listar cada fichero mandatorio individualmente (más verboso pero más auditable).

2. **`.github/workflows/sandbox-debt-sentinel.yml`** (nuevo): ejecuta solo los ficheros deferred listados en el ledger. **Falla** cuando fallan — eso es el sentinel.

3. **`reconstruction-contracts.toml`**: actualizar header para mencionar el sentinel workflow.

4. **`cycles/index.md`** + handoff REC-C0-CLOSURE: registrar la decisión arquitectónica.

## Estado actual de los commits

- `b4fe4e8d` — fix de ce1/ce7/ce10 (PASS en CI confirmado)
- `34781fb2` — entrada CE-001 en ledger (sin CI run dedicado, solo el run del commit anterior)
- HEAD está pusheado a `feat/rec-convergence-truth-gate`

Próximo paso: decidir Opción A/B/C y aplicar.
