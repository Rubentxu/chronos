# Certificate — UAT-M6-02-base

**Capacidad:** UAT-M6-02 — procedencia completa bajo contexto ausente, span huérfano, reloj monotónico divergente y OTLP temporalmente indisponible; ingestión y exportación recuperables e idempotentes.
**Perfil:** `base` (no privileged; sin red externa).
**SHA validado:** `61efb0f05b3c4b4988b2634af52128d34b4fb908` (`main` @ R5.0).
**Fecha:** 2026-10-04T06:12Z.
**Propietario:** AGENT (modo AUTO). **Revisor:** mismo agente, mismo SHA, recibo reproducido 3 veces byte a byte.

## Aserción observable (del UAT_CATALOG.md)

> Sin contexto, span huérfano, reloj monotónico diferente o OTLP temporalmente indisponible: error/procedencia completos, sin timestamps Unix inventados; ingestión y exportación recuperables/idempotentes dentro del contrato.

## Niveles CERT

| Nivel | Estado | Razón |
|---|---|---|
| CERT-0 | `passed` | Requisito con ID e invariantes: la ausencia de contexto produce error con procedencia, nunca un timestamp inventado; el retry es idempotente. |
| CERT-1 | `passed` | `run_uat_m6_02` en `chronos_domain::otlp::cross_service:312`, sin dependencias de wire. |
| CERT-2 | `passed` | Los ejecutores, las 24 pruebas de correlación y el escenario e2e pasan sobre el binario real del mismo SHA. |
| CERT-3 | `not_run` | La indisponibilidad real de OTLP y el reloj monotónico de otra máquina no se reproducen en este host. |
| CERT-4 | `not_run` | Sin threat model por capacidad. |

## Mapa UAT

Mismo binario y mismo total que UAT-M6-01 — los dos ejecutores viven en la misma suite — más los que cubren los dos legs de esta aserción:

| Test | Binario | Resultado |
|---|---|---|
| `otlp_cross_service` (incluye `uat_m6_02_passes_when_drift_preserved_and_retry_holds_trace_id` y `uat_m6_02_internal_assertion_drift_distinct_in_chronos_event`, `uat_m6_02_internal_assertion_retry_produces_new_uuid_same_trace`) | `.../tests/otlp_cross_service.rs` | **9/9** |
| `otlp_correlation` (procedencia en error) | `.../tests/otlp_correlation.rs` | **24/24** |
| `otlp_ingest` (idempotencia de ingesta) | `.../tests/otlp_ingest.rs` | **17/17** |
| `otlp_exporter` (exportación recuperable) | `.../tests/otlp_exporter.rs` | **36/36** |
| `otlp_redaction` | `.../tests/otlp_redaction.rs` | **15/15** |
| `otlp` / `otlp_gates` | `.../tests/otlp.rs`, `.../tests/otlp_gates.rs` | **24/24**, **27/27** |
| e2e por el cable MCP | `chronos-sandbox/tests/otlp_cross_service_e2e.rs` | **1/1** |
| **Total** | | **153/153** |

Comando canónico: idéntico al de UAT-M6-01 (campo `verify` de `OTEL-001`).

## Pruebas negativas (cubiertas)

- `drift_is_preserved` — la deriva del reloj monotónico se **conserva** en la cronología; no se "normaliza" a un Unix inventado. Es la mitad de la aserción que más fácil se falsearía.
- `retry_produces_new_uuid_same_trace` — el retry idempotente genera identidad nueva de invocación conservando el `trace_id`, que es la definición operativa de "idempotente" aquí: reintentar no debe duplicar la correlación.
- `mutation_is_correlated_errors_when_one_side_unbound` y `unambiguous_correlation_errors_on_unbound` — sin contexto, error; nunca un id adivinado.
- `error_paths_gate_passes_on_the_full_battery` — el gate de rutas de error **pasa con la batería completa**, que es lo contrario de un gate que solo se cumple por defecto.

## Recibo inmutable

`cargo run -p chronos-sandbox --example uat_receipt`, salida de UAT-M6-02 (estable en 3 corridas):

```
UatResult {
    scenario: "UAT-M6-02 / monotonic drift + idempotent retry preserves trace_id",
    outcome: DriftAndIdempotencyHold,
    divergence_count: 0,
    unsupported_region_count: 0,
    aggregate_hash: None,
    fingerprint_hash: None,
}
```

`DriftAndIdempotencyHold` nombra las dos mitades de la aserción. Ninguna de las dos es decorativa: un resultado `Pass` genérico habría SECRETADO si la deriva se perdía o si el retry duplicaba la correlación, y el nombre obliga a que la prueba discrimine.

## Limitaciones declaradas

- Recibo en `Debug`, no en JSON canónico: no existe `cross_service_wire`. Misma causa y mismo tratamiento que en UAT-M6-01; **no es un descuido de esta ficha sino una carencia de producto registrada**.
- El indisponibilidad real de OTLP y un reloj monotónico de otro host no se reproducen aquí; el fixture los sintetiza.
- `CHRONOS_CONTRACT_BASE_REF` sin fijar: el escaneo de líneas añadidas del gate de arquitectura no se ejecutó.

## Deuda residual aceptada

- Ninguna atribuible a M6-02.

## Fecha / condición de recertificación

1. Cambie `run_uat_m6_02` o la semántica de retry/idempotencia de `otlp::ingest`.
2. Se añada `cross_service_wire`.
3. Aparezca un entorno con OTLP real e indisponibilidad inyectada, lo que habilitaría CERT-3.
4. Bump de `serde`/`serde_json` que afecte al wire shape.

## Historial

- 2026-10-04T06:12Z (R6.0, sobre `61efb0f0`): **emitido**. Trigger cumplido por `OTEL-001` `verified` desde 2026-10-02.
