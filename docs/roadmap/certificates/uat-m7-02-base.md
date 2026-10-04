# Certificate — UAT-M7-02-base

**Capacidad:** UAT-M7-02 — comparación con gaps, contexto externo ausente o datos incomparables devuelve `unknown/unsupported` con región sin cobertura, **y no declara igualdad falsa**.
**Perfil:** `base` (no privileged).
**SHA validado:** `61efb0f05b3c4b4988b2634af52128d34b4fb908` (`main` @ R5.0).
**Fecha:** 2026-10-04T06:12Z.
**Propietario:** AGENT (modo AUTO). **Revisor:** mismo agente, mismo SHA, recibo reproducido 3 veces byte a byte.

## Aserción observable (del UAT_CATALOG.md)

> Comparación con gaps, contexto externo ausente o datos incomparables devuelve `unknown/unsupported` y región sin cobertura, no declara igualdad falsa.

## Por qué esta ficha merece más scrutiny que UAT-M7-01

Es la única de las cuatro cuya aserción principal es sobre lo que el sistema **NO** afirma. Un detector de divergencias que devuelve `pass` cuando no puede comparar no falla ningún test de éxito: es exactamente el fallo que el resto de la batería no toca, porque todo lo demás está en verde por construcción. Por eso el recibo de esta ficha incluye los contadores de región sin soporte, y no solo el veredicto.

## Niveles CERT

| Nivel | Estado | Razón |
|---|---|---|
| CERT-0 | `passed` | Requisito con ID e invariantes: la igualdad no puede declararse sobre datos incomparables; la carencia se nombra como región sin cobertura. |
| CERT-1 | `passed` | `run_uat_m7_02` en `cost_memory_collision.rs:472`. `UatOutcome::UnknownUnsupported` es una variante explícita del enum, no un string. |
| CERT-2 | `passed` | Ejecutor + adaptador de wire + suites de alineación, fingerprint y equivalencia, verdes sobre el mismo SHA. |
| CERT-3 | `not_run` | Los gaps los inyecta el fixture; no hay traza real truncada por retención. |
| CERT-4 | `not_run` | Sin threat model por capacidad. |

## Mapa UAT

Idéntico al de UAT-M7-01 (los dos ejecutores comparten suite); se reproduce el total para que la ficha sea autosuficiente:

| Test | Binario | Resultado |
|---|---|---|
| `cost_memory_uat_wire` | `crates/chronos-mcp/tests/cost_memory_uat_wire.rs` | **12/12** |
| `otlp_cost_memory_collision` | `crates/chronos-domain/tests/otlp_cost_memory_collision.rs` | **22/22** |
| `otlp_equivalence` | `.../tests/otlp_equivalence.rs` | **24/24** |
| `otlp_alignment` | `.../tests/otlp_alignment.rs` | **21/21** |
| `otlp_fingerprint` | `.../tests/otlp_fingerprint.rs` | **25/25** |
| `chronos-domain --lib` | lib de dominio | **193/193** |
| `chronos-services --lib` | lib de servicios | **608/608** (36,35 s) |
| **Total** | | **925/925** |

Comando canónico: el mismo campo `verify` de `DIFF-001` (ver ficha UAT-M7-01).

## Pruebas negativas (cubiertas)

- `adapter_m7_02_reproduces_canonical_unsupported_no_false_equality` — **la más importante de las cuatro**: comprueba que el adaptador de wire reproduce `unsupported` y que NO lo degrada a igualdad.
- `adapter_converted_pass_preserves_false_equality_bit` — un pass que advertía FalseEquality conserva ese bit al convertirse. Sin esto, la conversión sería el punto donde se pierde la advertencia.
- `to_wire_failed_outcome_surfaces_passed_false` — un outcome no exitoso no puede viajar como `passed=true`.
- `to_wire_round_trip_preserves_all_fields` — la conversión es 1:1, sin campos inventados ni perdidos.
- `uat_m7_02_does_not_claim_false_equality_when_gaps_present`.

## Recibo inmutable

`cargo run -p chronos-sandbox --example uat_receipt`, salida de UAT-M7-02 (JSON canónico vía `run_uat_m7_02_as_json()`, estable en 3 ejecuciones):

```json
{
  "aggregate_hash": 15527282866049128162,
  "divergence_count": 2,
  "fingerprint_hash": 10611589587867544546,
  "outcome": "UnknownUnsupported",
  "passed": true,
  "scenario": "UAT-M7-02 / gaps + missing-context (different trace_ids)",
  "unsupported_region_count": 2,
  "wire_version": "v1"
}
```

**`passed: true` con `outcome: "UnknownUnsupported"` no es una contradicción, y es lo que hay que leer con cuidado.** `passed` responde a *"¿se comportó el sistema como debe ante datos incomparables?"*; `outcome` responde a *"¿qué pudo afirmar?"*. Lo que la aserción prohíbe es `outcome: Passed` con `unsupported_region_count: 0` sobre estos datos, y eso es justo lo que el recibo descarta: **2 regiones sin cobertura y 2 divergencias declaradas, ninguna igualdad.**

Un lector que solo mirase `passed: true` concluiría que el detector de divergencias funciona sobre datos con huecos. No es lo que se certifica. Por eso el bloque va completo en la ficha y no un resumen.

## Limitaciones declaradas

- Escenario sobre fixture inyectado, no sobre retención real que trunque un log. CERT-3 `not_run`.
- `CHRONOS_CONTRACT_BASE_REF` sin fijar: el escaneo de líneas añadidas del gate de arquitectura no se ejecutó.
- El campo `wire_version` es `"v1"`; una v2 obligaría a recertificar por cambio de shape.

## Deuda residual aceptada

- Las limitaciones de `M7-CLOSE.md` siguen vigentes y **no** las levanta este certificado: cobertura de fingerprints inter-sesión, alineación entre lenguajes heterogéneos, y límites de presión de memoria y host cruzado. Esta ficha certifica el alcance escrito de UAT-M7-02.

## Fecha / condición de recertificación

1. Cambie `run_uat_m7_02`, la variante `UatOutcome::UnknownUnsupported` o su contador de regiones.
2. Cambie `UatResultWire` o `wire_version`.
3. Se añada un escenario de pérdida de datos real (retención, rotación) que ejercite la misma ruta.
4. Bump de `serde`/`serde_json` que afecte a `UatResultWire`.

## Historial

- 2026-10-04T06:12Z (R6.0, sobre `61efb0f0`): **emitido**. Trigger cumplido por `DIFF-001` `verified`.
