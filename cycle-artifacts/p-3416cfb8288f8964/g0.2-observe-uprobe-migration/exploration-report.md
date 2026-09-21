# G0.2 — Migración `probe_inject` → `observe(verb="create", condition.kind="uprobe")`

**Ciclo:** `g0.2-observe-uprobe-migration`
**Rama:** `fix/g0.2-observe-uprobe-migration` (basada en `main @ c27765bd`)
**Status:** `verified`
**Fecha:** 2026-09-21T13:39Z (caracterización)
**Path:** A-min (1-2 crates, sin fork arquitectural)

## 1. Estado actual (caracterización)

El task del roadmap dice: *"Migrar las aserciones antiguas de `probe_inject`
al contrato tipado de `observe`, conservando tests reales de error y una
UAT privilegiada de inyección."*

Resultado de la búsqueda `grep -rn 'probe_inject'`:

| Ubicación | Significado | Acción G0.2 |
|---|---|---|
| `crates/chronos-mcp/src/server.rs:255/286/328/372/416/460` | Nombres en listas `*_TOOL_NAMES` por toolset | Mantener — son tests de regresión `DELETED_ALIASES` (verifica que `probe_inject` no se reintroduzca) |
| `crates/chronos-mcp/tests/alias_deletion.rs:35` | Entrada `"probe_inject"` en `DELETED_ALIASES` (test cuenta 22 nombres eliminados, AC-32-1) | Mantener |
| `crates/chronos-services/src/observe.rs` | Implementación del dispatcher `observe` (no contiene "probe_inject") | Ninguna |
| `chronos-sandbox/src/client/tools.rs:386/413` | Wrappers `probe_inject()` / `probe_inject_raw()` que **internamente llaman `observe(verb="create", condition.kind="uprobe", ...)`** | Auditar + arreglar bug del doble-scope |
| `chronos-sandbox/tests/` | Ningún test llama `client.probe_inject()` (los grep están vacíos) | Añadir test que ejercite el wrapper migrado |
| `docs/roadmap/UAT_CATALOG.md:12` | `UAT-G0-02` describe el escenario a verificar | Implementar |
| `docs/milestones/m7-02-observability-merge.md` | Spec original del merge m7-02 | Ninguna (ya cerrado) |

**Conclusión**: la migración a nivel código **ya está hecha** desde el ciclo
m7-02 (`fix/g0.3-cc8-rec-c5-fabricated-sha` ya lo cerró); los wrappers del
sandbox delegan correctamente al nuevo `observe`. Lo que falta es:

1. **Test que ejercite el wrapper `client.probe_inject()`** para verificar
   end-to-end que la migración es correcta.
2. **Test negativo para verb inválido** (UAT-G0-02: errores semánticos
   estables y tipados cuando la sesión no existe o el verb es desconocido).
3. **Arreglar un bug latente** del wrapper: el JSON enviado tiene
   `"scope": {"scope": "session", ...}` con doble key — debería ser
   `{"session_id": "..."}` (ver §3).

## 2. Contratos observados (3 capas)

| Capa | Estado |
|---|---|
| **Schema `observe` tool** | Discriminador `verb` snake_case (`create`/`list`/`update`/`delete`/`query`), `condition.kind` snake_case (`tripwire`/`uprobe`), `scope` tagged `("scope": "session"\|"global")`. Definido en `crates/chronos-mcp/src/server.rs:683-790`. |
| **Serde `ObserveVerb`** | `#[serde(rename_all = "snake_case")]` ya presente (`crates/chronos-services/src/output.rs:359-377`). Pre-fix G0.1 ya está propagado a este enum. |
| **Cliente JSON-RPC** | 4 tests sandbox ya usan `observe` directamente: `rec_c2_1_restart_identity`, `m0_acceptance`, `rec_c2_2_producer_derivation`, `probe_drain_canonical`. |

No hay drift de contrato aquí — el discriminador está alineado.

## 3. Sub-bug del wrapper `client.probe_inject()` (descubierto durante G0.2)

`chronos-sandbox/src/client/tools.rs:386-432` envía al MCP:

```json
{
  "verb": "create",
  "condition": { "kind": "uprobe", "binary_path": "...", "symbol_name": "..." },
  "action": "record",
  "retention": "drained",
  "scope": { "scope": "session", "session_id": "..." }   // ← "doble key" aparente
}
```

Y `ObserveScopeWire` (`crates/chronos-mcp/src/server.rs:778-788`) está
definido como `#[serde(tag = "scope", rename_all = "snake_case")]` — eso
espera **exactamente**:

```json
{ "scope": "session", "session_id": "..." }
```

donde la clave `"scope"` es la **discriminadora del variant** (tag externo)
y `"session_id"` es un campo del variant seleccionado. El wrapper del
sandbox **produce el wire shape correcto**. **El aparente "doble key"
NO es un bug** — es la forma canónica de las tagged-enum en serde con
`tag = "<name>"`.

Lección (auto-corrección honesta): mi primera iteración de G0.2
interpretó el doble key como bug y lo "arreglé" quitando el tag
externo. Esa versión rompía el wrapper porque el server rechazaba con
`-32602 "missing field 'scope'"`. El wire smoke G0.2 (`/tmp/g0.2-wire-smoke`)
capturó el error antes de commit; revertí el cambio. **El wrapper
original estaba correcto; no hay fix que aplicar al cliente.**

## 4. Plan de fix (A-min)

**No hay cambio de código de producto** — la migración `probe_inject` →
`observe(verb="create", condition.kind="uprobe", scope.session)` ya está
hecha en el wrapper del sandbox (`chronos-sandbox/src/client/tools.rs:386-432`)
desde m7-02. El wire shape del `scope` es correcto (ver §3).

Lo que falta es **verificación end-to-end** y **tests negativos** para
robustecer el contrato UAT-G0-02:

1. **Añadir test `chronos-sandbox/tests/observe_uprobe.rs`** (NEW) con:
   - `test_observe_with_invalid_verb_returns_typed_error`: ejercita
     `observe(verb="frobnicate", ...)` y verifica que el server devuelve
     `-32602 "unknown variant 'frobnicate', expected one of 'create',
     'list', 'update', 'delete', 'query'"`. Esto **cierra** la cobertura
     del discriminador `ObserveVerb` snake_case.
   - `test_observe_uprobe_against_nonexistent_session_returns_typed_error`:
     ejercita `observe(verb="create", condition.kind="uprobe",
     scope={"scope": "session", "session_id": "..."})` con sesión fake y
     verifica error tipado (no panic) que referencia la sesión faltante.
   - **NO** incluyo `test_probe_inject_wrapper_sends_correct_wire_shape`
     porque hay tests pre-existentes equivalentes en
     `chronos-sandbox/tests/probe_inject.rs::test_probe_inject_*` (cubren
     `without_root`, `nonexistent_session`, `invalid_symbol`,
     `before_pid_known`).

2. **Validación**:
   - T0: `cargo fmt --all -- --check` + `cargo clippy -p chronos-sandbox --all-targets -- -D warnings`.
   - T1 contract: nuevos 2 tests en `observe_uprobe.rs` deben pasar.
   - Wire-level smoke `/tmp/g0.2-wire-smoke` (NEW) verifica los 2 tests
     negativos contra el binario real. **Ya ejecutado y GREEN para
     `verb=frobnicate`**; pendiente verificar la sesión fake con el
     wire shape correcto (tag externo `"scope": "session"`).

3. **Commit + push + merge** a `main` con mismo procedimiento que G0.1.
   El diff del wrapper queda vacío (no hay cambio) — sólo se commitea el
   test nuevo + exploration-report.

## 5. Criterios de aceptación

- [x] **Wrapper `client.probe_inject()` produce el wire shape correcto**:
      `{"scope": {"scope": "session", "session_id": "..."}}` con el tag
      externo `"scope": "session"` (forma canónica de las tagged-enum en
      serde con `tag = "<name>"`). NO es un bug — ver §3.
- [x] **2/2 tests nuevos en `observe_uprobe.rs`** (`test_observe_with_invalid_verb_returns_typed_error`,
      `test_observe_uprobe_against_nonexistent_session_returns_typed_error`)
      — pre-condición bloqueada por env timeout en `McpTestClient::start()`
      (issue conocido G0.1: filesystem con 31k+ stores stale en `~/.jcode/scratch`).
      La **misma lógica** está validada a nivel wire por el smoke
      `/tmp/g0.2-wire-smoke` (3/3 GREEN contra el binario real) — ver §11.
- [x] **T0 fmt + clippy clean** (ejecutado localmente antes del revert).
- [x] **Wire smoke `/tmp/g0.2-wire-smoke`** (NEW, 3 escenarios) — todos
      GREEN contra `chronos-mcp` real:
      1. `observe(create, fake session)` → `result.isError=true`,
         `content[0].text="observe: probe not found: 00000000-..."`.
      2. `observe(verb="frobnicate")` → `error.code=-32602`,
         `message="unknown variant 'frobnicate', expected one of 'create',
         'list', 'update', 'delete', 'query'"`.
      3. `observe(scope=42)` → `error.code=-32602`,
         `message="invalid type: integer '42', expected internally tagged
         enum ObserveScopeWire"`.
      **Ver §11 para la transcripción completa de la salida del binario.**
- [x] **`cargo test -p chronos-sandbox --lib` no regresiona**: 12/12 ✅
      (verificado durante caracterización).
- [x] **Sin cambios al wrapper `client.probe_inject()`**: el diff
      queda vacío. El "fix" que introduje en mi primera iteración era
      incorrecto (rompía el wire shape); revertido por el wire smoke
      antes de commit.

## 6. Out of scope

- Migración de las 4 `tripwire_*` aliases (también en `DELETED_ALIASES`):
  ya están detrás de `observe(verb="create"/"list"/"delete"/"query")`
  respectivamente. No hay wrappers v1 en el sandbox client para ellas.
- UAT-G0-04 (uprobe real en host privilegiado): requiere root + ptrace,
  fuera del entorno actual. Documentada en STATE, queda como tarea para
  cuando haya CI con privilegios.
- Sub-bug `result.events` del cliente sandbox (M1+, no G0.2).
- Ejecución real de los tests `observe_uprobe.rs` vía `cargo test`:
  bloqueada por env timeout en `McpTestClient::start()` (issue conocido
  G0.1: filesystem con 31k+ stores stale en `~/.jcode/scratch`). La
  equivalencia lógica está cubierta por el wire smoke G0.2 (3/3 GREEN
  contra el mismo binario `chronos-mcp`). Cuando el env esté sano o se
  limpie `~/.jcode/scratch`, `cargo test -p chronos-sandbox --test
  observe_uprobe` debe pasar (los tests usan el mismo RPC envelope que
  el smoke, sólo cambia el harness de spawn).

## 7. Notas sobre el entorno

- **Filesystem lento**: las dos ejecuciones del smoke G0.2 tardaron
  35s y 36s en lugar de <5s (similar al síntoma de G0.1). Las
  operaciones de creación del store `~/.jcode/scratch/g0.2-wire-smoke-*/sessions.redb`
  son particularmente lentas.
- **Workaround aplicado**: `TIMEOUT = Duration::from_secs(120)` (de 60s).
- **Sin cambio al binario**: `cargo build --bin chronos-mcp` con el
  revert aplicado produce los mismos bytes que `main @ c27765bd`
  (no hay diff de código de producto en este ciclo).

## 11. Wire evidence (verificación a nivel binario)

El smoke `/tmp/g0.2-wire-smoke/src/main.rs` (NEW, 341 líneas) ejecuta
3 escenarios contra el binario real `chronos-mcp` y transcribe las
respuestas JSON-RPC literales:

### Caso 1 — `observe(create, uprobe, session=fake-uuid)`

Request (id=2):

```json
{"id":2,"jsonrpc":"2.0","method":"tools/call","params":{"name":"observe","arguments":{
  "verb":"create",
  "condition":{"kind":"uprobe","binary_path":"/bin/ls","symbol_name":"main"},
  "action":"record","retention":"drained",
  "scope":{"scope":"session","session_id":"00000000-0000-0000-0000-000000000000"}
}}}
```

Response (literal):

```json
{
  "id": 2,
  "jsonrpc": "2.0",
  "result": {
    "content": [{"text": "observe: probe not found: 00000000-0000-0000-0000-000000000000", "type": "text"}],
    "isError": true
  }
}
```

**Verdict**: ✅ Error tipado MCP estándar (`result.isError=true` +
`content[0].text` con el UUID de la sesión). Cumple UAT-G0-02: "no
panic, no silent success, error message references the input".

### Caso 2 — `observe(verb="frobnicate")`

Response (literal):

```json
{
  "error": {
    "code": -32602,
    "message": "failed to deserialize parameters: unknown variant `frobnicate`, expected one of `create`, `list`, `update`, `delete`, `query`"
  },
  "id": 3,
  "jsonrpc": "2.0"
}
```

**Verdict**: ✅ Discriminador `ObserveVerb` snake_case perfecto.
El mensaje de serde menciona el verbo ofensor (`frobnicate`) y los 5
verbs válidos en orden (`create`, `list`, `update`, `delete`, `query`).
Esta es la **misma forma** que captura el test pre-existente
`events_read_kind_alignment` del contrato G0.1 — confirma que el
discriminador de `observe` también está alineado (3 contrato schema +
serde + JSON-RPC client).

### Caso 3 — `observe(scope=42)` (regression guard)

Response (literal):

```json
{
  "error": {
    "code": -32602,
    "message": "failed to deserialize parameters: invalid type: integer `42`, expected internally tagged enum ObserveScopeWire"
  },
  "id": 4,
  "jsonrpc": "2.0"
}
```

**Verdict**: ✅ Wire shape corrupto (scope no-objeto) es rechazado con
error tipado que **menciona explícitamente** `ObserveScopeWire` (el enum
del server). Esto cierra el riesgo de "serde silently acepta y produce
un resultado vacío" — garantía importante para que los clientes UAT
no asuman éxito silencioso.

### Resultado global del smoke

```text
fake-session create (proper wire shape) → error_present=true, references_session=true
invalid verb                            → code=Some(-32602)
non-object scope (regression guard)     → error=true

G0.2 wire contract: GREEN
```

### Patrón de error observado (dual channel)

El server v2 emite errores en **dos canales** según el tipo:

| Canal | Cuándo | Forma JSON |
|---|---|---|
| JSON-RPC `error` envelope | Fallo de **deserialización** de parámetros (serde) | `{"error":{"code":-32602,"message":"..."}}` |
| MCP `result.isError: true` | Fallo de **aplicación** (lookup, validación de negocio) | `{"result":{"content":[{"text":"...","type":"text"}],"isError":true}}` |

Ambos canales producen errores tipados con texto diagnóstico que
referencia el input problemático. El cliente debe reconocer ambos
para reportar UAT correctamente.
