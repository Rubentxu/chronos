# Certificate — UAT-M6-01-base

**Capacidad:** UAT-M6-01 — correlación de extremo a extremo entre un span externo y una invocación Chronos, sin mezcla bajo concurrencia.
**Perfil:** `base` (no privileged; sin red externa).
**SHA validado:** `61efb0f05b3c4b4988b2634af52128d34b4fb908` (`main` @ R5.0).
**Fecha:** 2026-10-04T06:12Z.
**Propietario:** AGENT (modo AUTO). **Revisor:** mismo agente, mismo SHA, misma máquina, recibo reproducido 3 veces byte a byte.

## Por que este certificado existe ahora

`OTEL-001` (el requisito canónico de M6) está en `reconstruction-contracts.toml` con `status = "verified"` desde 2026-10-02, con su campo `verify` poblado. La sección "Próximos certificados a emitir" de `certificates/README.md` decía que `UAT-M6-01..02` no se emitían "hasta que un ciclo H1/M/M-OPS los materialice". **Ese trigger se cumplió y los certificados nunca se emitieron**: el registro de certificación llevaba dos semanas por detrás del ledger que le sirve de autoridad. La deuda era real, y se verificó antes de tratarla como tal (ejecutores presentes en `crates/chronos-domain/src/otlp/cross_service.rs:137` y `:312`, con suite propia).

## Aserción observable (del UAT_CATALOG.md)

> Petición atraviesa dos servicios y llega a mutación y property violation: correlación trace/span externo -> invocación Chronos -> mutación con IDs distintos; dos peticiones concurrentes no mezclan contexto.

## Niveles CERT

| Nivel | Estado | Razón |
|---|---|---|
| CERT-0 | `passed` | Requisito con ID, alcance e invariantes: la correlación no puede resolverse cuando un lado queda sin ligar, y dos trazas concurrentes no pueden cruzarse. ADR-0018 (M6.4) y ADR-0021 (M6.7) levantados según el propio ledger. |
| CERT-1 | `passed` | Implementación aislada en `chronos_domain::otlp::cross_service` + `otlp::correlation`. Sin dependencias de wire. |
| CERT-2 | `passed` | Integración canónica: los cuatro ejecutores de correlación y el escenario e2e pasan **sobre el binario real** del mismo SHA. Ver mapa. |
| CERT-3 | `not_run` | El UAT pide dos servicios reales y un property violation observado. La evidencia es un fixture determinista, no un collector OTLP en vivo. Declarado como exclusión abajo, no como aprobado. |
| CERT-4 | `not_run` | Sin threat model aplicado a M6 ni SBOM por capacidad. Depende de CERT-3. |

## Mapa UAT

| Test | Binario | Resultado | Duración |
|---|---|---|---|
| `otlp_cross_service` (ejecuta `run_uat_m6_01` y `run_uat_m6_02`) | `crates/chronos-domain/tests/otlp_cross_service.rs` | **9/9** | 0,00 s |
| `otlp` (tipos, separación) | `.../tests/otlp.rs` | **24/24** | 0,00 s |
| `otlp_correlation` (incluye los 3 negativos de ambigüedad) | `.../tests/otlp_correlation.rs` | **24/24** | 0,00 s |
| `otlp_exporter` | `.../tests/otlp_exporter.rs` | **36/36** | 0,00 s |
| `otlp_gates` (4 invariantes, cada una con contrapeso) | `.../tests/otlp_gates.rs` | **27/27** | 0,02 s |
| `otlp_ingest` | `.../tests/otlp_ingest.rs` | **17/17** | 0,00 s |
| `otlp_redaction` | `.../tests/otlp_redaction.rs` | **15/15** | 0,00 s |
| **Escenario e2e por el cable MCP real** | `chronos-sandbox/tests/otlp_cross_service_e2e.rs` | **1/1** | 0,02 s |
| **Total** | | **153/153** | |

Comando canónico (el del campo `verify` de `OTEL-001`, sin modificar):

```
cargo test --manifest-path crates/chronos-domain/Cargo.toml \
  --test otlp_cross_service --test otlp --test otlp_correlation \
  --test otlp_ingest --test otlp_redaction --test otlp_exporter \
  --test otlp_gates --no-fail-fast \
  && cargo test -p chronos-sandbox --test otlp_cross_service_e2e \
  && python3 scripts/check_architecture_contracts.py
```

## Pruebas negativas (cubiertas, no asumidas)

- `unambiguous_correlation_errors_on_unbound` — una correlación sin un lado ligado **erroriza** en vez de devolver un id inventado.
- `mutation_is_correlated_errors_when_one_side_unbound` — mismo contrato desde el lado de la mutación.
- `correlation_error_display_includes_event_idx` — el error nombra el evento, o sea es accionable y no un "internal error" opaco.
- `error_paths_verdict_fails_on_each_violated_invariant` — cada invariante rota produce veredicto de fallo, no un pass por defecto.
- `zero_events_yields_zero_spans_not_an_error` — el caso vacío no se confunde con un fallo.

## Recibo inmutable

`chronos-sandbox/examples/uat_receipt.rs` — sonda nueva, sin aserciones: imprime, para archivar.

```
cargo run -p chronos-sandbox --example uat_receipt
```

Salida de UAT-M6-01 (estable en 3 corridas consecutivas):

```
UatResult {
    scenario: "UAT-M6-01 / two concurrent invocations distinct traces no-mix",
    outcome: CrossServiceUnambiguous,
    divergence_count: 0,
    unsupported_region_count: 0,
    aggregate_hash: None,
    fingerprint_hash: None,
}
```

`CrossServiceUnambiguous` es exactamente el resultado que la aserción exige: correlación no ambigua, cero divergencias, cero regiones sin soporte. Un `Unknown` o un error habría sido la respuesta honesta pero **no** la exigida, así que el valor importa y está archivado.

## Limitaciones declaradas

- **El recibo de M6 se emite en `Debug`, no en JSON canónico.** La causa es una decisión previa y correcta: `UatResult` no deriva `serde` para mantener el dominio libre de wire-shape. M7 sí tiene adaptador (`chronos_mcp::cost_memory_wire::UatResultWire`), M6 no. Crear `cross_service_wire` sería **cambio de producto dentro de una tarea de certificación**, así que la carencia se registra en vez de colarse en el trabajo.
- **Escenario e2e sin red externa.** El test e2e corre contra el binario MCP real, pero el span externo lo sintetiza el propio fixture; no hay collector OTLP en el bucle.
- **El gate de arquitectura corrió con `CHRONOS_CONTRACT_BASE_REF` sin fijar**, y por eso emitió `WARN: ... skipping added-line legacy scan`. El gate pasó, pero **el escaneo de líneas añadidas no se ejecutó**. No se compte como cobertura.

## Deuda residual aceptada

- Ninguna deuda abierta atribuible a M6-01. Las demás alertas del ledger que mencionan M6 están retractadas o son env-locked (`DEBT-G0-04`, uprobe privilegiado).

## Fecha / condición de recertificación

Recertificar cuando:

1. Cambie `run_uat_m6_01`, la estructura de `UatResult` o la lógica de `otlp::correlation`.
2. Se añada `cross_service_wire` (el recibo pasaría de `Debug` a JSON y dejaría de ser una limitación).
3. Se disponga de un collector OTLP real, lo que habilitaría CERT-3.
4. Bump de `serde`/`serde_json` que afecte al wire shape.

## Historial

- 2026-10-04T06:12Z (R6.0, sobre `61efb0f0`): **emitido**. `OTEL-001` ya estaba `verified` en el ledger desde 2026-10-02 y el certificado nunca se emitió.
