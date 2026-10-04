# Certificate — UAT-M7-01-base

**Capacidad:** UAT-M7-01 — primera divergencia semántica sustentada por eventos, con ruido de orden y timestamps, sin confundir símbolo con invocación.
**Perfil:** `base` (no privileged).
**SHA validado:** `61efb0f05b3c4b4988b2634af52128d34b4fb908` (`main` @ R5.0).
**Fecha:** 2026-10-04T06:12Z.
**Propietario:** AGENT (modo AUTO). **Revisor:** mismo agente, mismo SHA, recibo reproducido 3 veces byte a byte.

## Por que este certificado existe ahora

`DIFF-001` (requisito canónico de M7) está `status = "verified"` en `reconstruction-contracts.toml` desde el cierre de R4.1 (`5b632a92`). El trigger de emisión que `certificates/README.md` declaraba pendiente —que un ciclo de M materializara la capacidad— **se cumplió y el certificado nunca se emitió**. Verificado antes de tratarlo como deuda: los ejecutores existen (`crates/chronos-domain/src/otlp/cost_memory_collision.rs:413` y `:472`), el adaptador de wire existe (`crates/chronos-mcp/src/cost_memory_wire.rs`, 12 tests propios), y ambos ejecutan.

## Aserción observable (del UAT_CATALOG.md)

> Ejecución good y bad con ruido en orden/timestamps y un bug de estado conocido: encuentra primera divergencia semántica sustentada por eventos y no confunde símbolo con invocación.

## Niveles CERT

| Nivel | Estado | Razón |
|---|---|---|
| CERT-0 | `passed` | Requisito con ID e invariantes: la divergencia se declara solo con sustento de eventos, y símbolo no equivale a invocación. |
| CERT-1 | `passed` | `run_uat_m7_01` en `chronos_domain::otlp::cost_memory_collision`, sin dependencias de wire; `UatResult` deliberadamente sin derives `serde` (dominio libre de wire-shape). |
| CERT-2 | `passed` | Ejecutor, adaptadores de equivalencia/alineación/fingerprint, suite de dominio y de servicios, más el adaptador MCP, verdes sobre el mismo SHA. |
| CERT-3 | `not_run` | El ruido y el bug de estado los genera un fixture, no una traza real de dos procesos. |
| CERT-4 | `not_run` | Sin threat model por capacidad. |

## Mapa UAT

| Test | Binario | Resultado | Duración |
|---|---|---|---|
| `cost_memory_uat_wire` (adaptador MCP, **incluye la prueba de que el wire reproduce la semántica canónica**) | `crates/chronos-mcp/tests/cost_memory_uat_wire.rs` | **12/12** | 0,00 s |
| `otlp_cost_memory_collision` (ejecuta `run_uat_m7_01/02`) | `crates/chronos-domain/tests/otlp_cost_memory_collision.rs` | **22/22** | 0,00 s |
| `otlp_equivalence` | `.../tests/otlp_equivalence.rs` | **24/24** | 0,01 s |
| `otlp_alignment` (invocación ≠ símbolo) | `.../tests/otlp_alignment.rs` | **21/21** | 0,01 s |
| `otlp_fingerprint` | `.../tests/otlp_fingerprint.rs` | **25/25** | 0,00 s |
| `chronos-domain --lib` | lib de dominio | **193/193** | 0,03 s |
| `chronos-services --lib` | lib de servicios | **608/608** | 36,35 s |
| **Total** | | **925/925** | |

Comando canónico (campo `verify` de `DIFF-001`, sin modificar):

```
cargo test --manifest-path crates/chronos-mcp/Cargo.toml --test cost_memory_uat_wire --no-fail-fast \
  && cargo test --manifest-path crates/chronos-domain/Cargo.toml \
       --test otlp_equivalence --test otlp_alignment --test otlp_fingerprint \
       --test otlp_cost_memory_collision --no-fail-fast \
  && cargo test --manifest-path crates/chronos-domain/Cargo.toml --lib --no-fail-fast \
  && cargo test --manifest-path crates/chronos-services/Cargo.toml --lib --no-fail-fast \
  && python3 scripts/check_architecture_contracts.py --strict-no-gaps
```

## Pruebas negativas (cubiertas)

- `adapter_m7_02_reproduces_canonical_unsupported_no_false_equality` — el adaptador **no** convierte un `unsupported` en igualdad. Es la prueba que impede que la capa de wire PPIESE la advertencia.
- `adapter_m7_01_reproduces_canonical_divergence_pass` — el adaptador propaga el pass canónico con la etiqueta exacta, `passed=true` y los contadores intactos.
- `uat_m7_02_does_not_claim_false_equality_when_gaps_present` — gaps producen región sin cobertura, nunca pass.
- `adapter_converted_pass_preserves_false_equality_bit` — el bit de FalseEquality sobrevive al round-trip de conversión. Sin esta comprobación, una conversión podría "limpiar" un pass que en realidad advertía, y el adaptador pasaría a mentir en la única dirección que importa.
- `uat_result_wire_json_keys_match_documented_fields` — exactamente 8 claves, sin deriva silenciosa del shape.

## Recibo inmutable

`cargo run -p chronos-sandbox --example uat_receipt` — **JSON canónico**, emitido por el adaptador de producto `run_uat_m7_01_as_json()` (`wire_version: "v1"`), no por una reimpresión de la sonda.

Verificado **byte a byte en 3 ejecuciones consecutivas** del mismo binario:

```json
{
  "aggregate_hash": 727489852173063677,
  "divergence_count": 1,
  "fingerprint_hash": 16296333918912416704,
  "outcome": "FoundDivergence",
  "passed": true,
  "scenario": "UAT-M7-01 / state-bug on shared trace_id",
  "unsupported_region_count": 0,
  "wire_version": "v1"
}
```

Los dos hashes son **reproducibles**, no un artefacto de una ejecución suelta, y por eso pueden archivarse como recibo inmutable. `divergence_count: 1` importa: la aserción pide la **primera** divergencia, y un `0` habría sido un pass vacío.

## Limitaciones declaradas

- **Escenario sobre fixture, no sobre trazas reales** de dos procesos concurrentes. CERT-3 queda `not_run` por esto y no por falta de effort.
- `CHRONOS_CONTRACT_BASE_REF` sin fijar en el gate de arquitectura: el escaneo de líneas añadidas no se ejecutó y no se cuenta como cobertura.

## Deuda residual aceptada

- Las limitaciones honestas de M7 registradas en `M7-CLOSE.md` (hash criptográfico de 128 bits, normalización por probe, fingerprinting entre sesiones, alineación entre lenguajes, y las queigationes de presión de memoria y de host cruzado) siguen vigentes. **Este certificado no las levanta**; certifica el alcance del UAT-M7-01 tal como está escrito en `UAT_CATALOG.md`, no el capítulo completo.

## Fecha / condición de recertificación

1. Cambie `run_uat_m7_01`, `UatResult`/`UatOutcome` o `UatResultWire`.
2. Cambie la versión de wire (`wire_version` deja de ser `"v1"`).
3. Se cierre alguna de las limitaciones de `M7-CLOSE.md` listadas arriba.
4. Bump de `serde`/`serde_json` que afecte a `UatResultWire`.

## Historial

- 2026-10-04T06:12Z (R6.0, sobre `61efb0f0`): **emitido**.
