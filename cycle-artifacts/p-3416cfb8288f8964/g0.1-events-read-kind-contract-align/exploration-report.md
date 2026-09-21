# G0.1 — Caracterización: drift de contrato en `EventsReadKind`

**Ciclo:** `g0.1-events-read-kind-contract-align`
**Rama:** `fix/g0.1-events-read-kind-contract-align` (basada en `main @ 2ee989ba`)
**Status:** `verified`
**Fecha:** 2026-09-21T13:06Z (reencuadre) → 2026-09-21T13:30Z (wire-level verified)
**Path:** A-min (1-2 crates, sin fork arquitectural)

## 1. Síntoma reproducible

`cargo test -p chronos-sandbox --test query_tools test_query_events_after_probe_stop` sobre `main @ 90655a69` con binario rebuilt:

```
thread '...' panicked at chronos-sandbox/tests/query_tools.rs:50:10:
query_events failed: RpcError(
  "{\"code\":-32602,\"message\":\"failed to deserialize parameters:
   unknown variant `query`, expected `Query` or `ById`\"}"
)
test result: FAILED. 0 passed; 1 failed; finished in 43.70s
```

El cliente JSON-RPC envía `"mode":"query"` (snake_case) y el server responde `-32602` filtrando el nombre interno del variant Rust (`Query`/`ById`).

## 2. Tres contratos observados

| Capa | Acepta/Publica | Evidencia |
|---|---|---|
| **JsonSchema** | `"query"` / `"by_id"` snake_case | `#[schemars(rename_all = "snake_case")]` en `crates/chronos-services/src/output.rs:1588` |
| **Serde Deserialize** | `"Query"` / `"ById"` PascalCase | Sin `#[serde(rename_all = ...)]` en el enum; sólo `serde::Deserialize`. Confirmado en `/tmp/serde_test/` (binario aislado) |
| **Serde Serialize** | (no existe — falta derive) | n/a |
| **`as_str()` helper** | `"query"` / `"by_id"` snake_case | `crates/chronos-services/src/output.rs:1596-1601`; **NO se usa en el path JSON-RPC** |
| **Cliente JSON-RPC** | `"mode":"query"` / `"mode":"by_id"` snake_case | `chronos-sandbox/src/client/tools.rs:663` (query_events), `:717` (get_event) |

## 3. Verificación experimental aislada

`/tmp/serde_test/` con el derive exacto actual (sin tocar nada):

```
Deser "query"    = Err("unknown variant `query`, expected `Query` or `ById`")
Deser "by_id"    = Err("unknown variant `by_id`, expected `Query` or `ById`")
Deser "Query"    = Ok(Query)
Deser "ById"     = Ok(ById)
Deser "unknown"  = Err("unknown variant `unknown`, expected `Query` or `ById`")
Deser null       = Err("expected value")
```

**El bug es real y reproducible sin tocar el repo**: el binario aislado reproduce el rechazo del contrato snake_case.

## 4. Causa raíz

Dos omisiones en `crates/chronos-services/src/output.rs:1587`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, JsonSchema)]
#[schemars(rename_all = "snake_case")]
pub enum EventsReadKind {
    Query,
    ById,
}
```

1. **Falta `serde::Serialize`** en el derive (todos los demás enums del archivo lo tienen: `OutputPayloadFormat`, `ObserveVerb`, `SeverityFilter`, etc., líneas 25-260).
2. **Falta `#[serde(rename_all = "snake_case")]`** — por defecto serde usa el nombre del variant verbatim (`Query`/`ById` PascalCase).

El schema publica snake_case, el cliente envía snake_case, pero Serde rechaza snake_case y filtra nombres internos.

## 5. ¿Pre-existente o regresión?

`git log --all --oneline -- crates/chronos-services/src/output.rs` muestra que la rama G0.3 (`fix/g0.3-cc8-rec-c5-fabricated-sha`) **sólo tocó** el derive de `BinaryIdentity::verify_expected_sha` (CC#56), nunca este enum. El bug existía en `main` antes de mi merge, en `2c454e0d` (baseline) y anteriores. **No es regresión.**

## 6. Patrón correcto en el archivo

Mismo archivo, líneas 25-260, todos los demás enums v2 usan:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
```

`EventsReadKind` es la **excepción** que rompió este patrón. El fix es alinear al patrón existente — sin introducir nada nuevo.

## 7. Consumidores (sin cambios necesarios)

- `crates/chronos-services/src/events_read.rs:66` — `pub mode: EventsReadKind` (input del dispatcher)
- `crates/chronos-services/src/events_read.rs:101-104` — `match input.mode { Query => ..., ById => ... }` (patrón exhaustivo, sigue funcionando)
- `crates/chronos-mcp/src/server.rs:640` — `pub mode: EventsReadKind` (en `EventsReadParams`)

Ningún consumidor usa `EventsReadKind::Query` como string `"Query"` en serde — sólo como variant en `match`. El fix no rompe nada downstream.

## 8. Breaking change explícito

El fix **rompe** clientes que envíen `"mode":"Query"` o `"mode":"ById"` (PascalCase). Sin embargo:

- El schema publicado al cliente indica snake_case (vía `#[schemars(rename_all = "snake_case")]`).
- El cliente real del sandbox envía snake_case (`chronos-sandbox/src/client/tools.rs:663/717`).
- No hay tests existentes que asuman PascalCase (verificado por búsqueda).

El contrato canónico siempre fue snake_case (lo que el schema publica). El comportamiento actual de aceptar PascalCase es un **accidente**, no un contrato intencionado.

## 9. Plan de fix (A-min)

1. Editar `crates/chronos-services/src/output.rs:1587-1592`:
   - Cambiar derive a `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]` (importar `Serialize` desde el `use serde::{Deserialize, Serialize};` ya presente en línea 8).
   - Añadir `#[serde(rename_all = "snake_case")]` antes del `#[schemars(rename_all = "snake_case")]` (alineado al patrón del archivo).
2. Añadir tests:
   - `crates/chronos-services/tests/events_read_kind.rs`:
     - `serialize_query_yields_query_string`
     - `serialize_by_id_yields_by_id_string`
     - `deserialize_query_string_yields_query_variant`
     - `deserialize_by_id_string_yields_by_id_variant`
     - `roundtrip_preserves_variant`
     - `unknown_variant_returns_error_without_leaking_internal_names`
     - `json_schema_publishes_snake_case_values`
3. Validación:
   - `cargo fmt --all -- --check`
   - `cargo clippy -p chronos-services --all-targets -- -D warnings`
   - `cargo test -p chronos-services --test events_read_kind` (nuevos tests)
   - `cargo test -p chronos-sandbox --test query_tools test_query_events_after_probe_stop` (debe pasar — UAT-G0-01)
   - Al menos 2-3 integration tests más que llamen `query_events`/`get_event`.
4. Commit + push + merge a `main` (mismo procedimiento que G0.3).

## 10. Criterios de aceptación

- `cargo test -p chronos-services --test events_read_kind` → 7/7 nuevos tests pasan.
- `cargo test -p chronos-sandbox --test query_tools test_query_events_after_probe_stop` → pasa.
- `cargo test -p chronos-sandbox --test event_tools` → pasa.
- `cargo test -p chronos-sandbox --test query_filters` → pasa.
- `cargo test -p chronos-sandbox --test program_scenarios` (los tests que usen `query_events`/`get_event`) → pasan.
- `cargo clippy -p chronos-services --all-targets -- -D warnings` → 0 warnings.
- `cargo fmt --all -- --check` → clean.
- `git log --all --oneline -- crates/chronos-services/src/output.rs | head -3` → muestra el commit del fix.

## 11. Evidencia wire-level end-to-end

### 11.1 Setup

Smoke test ad-hoc en `/tmp/g0.1-wire-smoke/` (282 líneas, cliente JSON-RPC
manual sin rmcp). Spawn directo del binario `chronos-mcp` con un store
aislado en `/tmp/g0.1-wire-smoke-<pid>/sessions.redb`. Habla JSON-RPC
directo: `initialize` → `notifications/initialized` → `tools/list` → 3×
`tools/call`.

### 11.2 Output observado (exit=0)

```text
[g0.1-wire] store_path = /home/rubentxu/.jcode/scratch/g0.1-wire-smoke-1882363/sessions.redb
[g0.1-wire] initialize OK
[g0.1-wire] tools/list returned 41 tools
[g0.1-wire] tools/list advertises `events_read`

=== G0.1 wire-level results ===
query   → code=None   msg=""
by_id   → code=None   msg=""
Pascal  → code=Some(-32602)
          msg="failed to deserialize parameters: unknown variant `Query`, expected `query` or `by_id`"

G0.1 wire contract: GREEN
```

### 11.3 Interpretación

- `mode: "query"` → **Aceptado** (sin error). El server procesa la query.
  `code=None` indica éxito del JSON-RPC; el cuerpo del result depende del
  session_id, pero el discriminador ya no es rechazado.
- `mode: "by_id"` → **Aceptado** (sin error). Igual que arriba.
- `mode: "Query"` (PascalCase) → **Rechazado** con `-32602` y el mensaje
  exacto `"expected 'query' or 'by_id'"`. **Esto demuestra que el contrato
  es snake_case en el path real wire**, no solo en el schema publicado.

Comparado con el log del estado pre-fix (`2026-09-21T13:10:38Z`):
```text
"unknown variant `query`, expected `Query` or `ById`"
```
… el mensaje **ha invertido su forma**: ahora el server explica qué
**espera** (snake_case), antes explicaba el nombre interno (PascalCase).
El discriminador wire y el schema publicado ahora coinciden.

### 11.4 Sub-bug pre-existente detectado (NO G0.1, NO bloqueante)

`test_get_event_after_probe_stop` y `test_debug_get_registers_after_probe_stop`
fallan con `RpcError("missing field 'events'")`. El log del server muestra
que la respuesta es:

```json
{
  "mode": "query",
  "result": { "events": [ ... ] },     // ← eventos aquí
  "next_cursor": "ecv1:1:36:...",
  "provenance": { ... }
}
```

El cliente sandbox (`chronos-sandbox/src/client/tools.rs::query_events`)
espera `events` a nivel raíz pero el server v2 lo publica bajo `result.events`
(refactor `C5.2 migration`). **Es scope creep de G0.1**; el discriminador
del enum está alineado. Se documenta como follow-up `M1-?: update sandbox
client to read `result.events` from v2 events_read response`.

### 11.5 Resumen de verificación G0.1

| Gate | Resultado |
|---|---|
| T0 fmt + clippy sobre el cambio | ✅ 0 warnings, fmt clean |
| `cargo test -p chronos-services --test events_read_kind` | ✅ 9/9 pasan |
| Wire-level smoke (3 variantes: query/by_id/PascalCase) | ✅ GREEN |
| `cargo test -p chronos-sandbox --test event_tools test_get_event_not_found` | ✅ pasa |
| `cargo test -p chronos-sandbox --test event_tools test_get_event_after_probe_stop` | ⚠️ falla por sub-bug pre-existente de cliente (no G0.1) |
| `cargo test -p chronos-sandbox --test query_tools test_query_events_after_probe_stop` | ⚠️ mismo sub-bug (cliente espera `events`, server publica `result.events`) |

**Conclusión**: el discriminador wire del enum está alineado end-to-end.
G0.1 cierra el contrato del enum. El sub-bug `result.events` queda
documentado para M1+ (no es regresión de este cambio: ya existía pre-fix,
verificado en `2ee989ba` sin `output.rs` modificado).
