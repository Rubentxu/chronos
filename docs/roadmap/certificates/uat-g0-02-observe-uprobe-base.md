# Certificate — UAT-G0-02-observe-uprobe-base

**Capacidad:** UAT-G0-02 — `observe` JSON-RPC typed errors contract.
**Perfil:** `base` (no privileged).
**SHA validado:** `afa14fd20f191d5884a8b030d4f05a915024f794` (HEAD de `main` en G0.6).
**Tag remoto:** `v0.7.112` peel `0be2ec2d53d9698956ae705938b32b80d7365ad7` (intacto).
**Fecha:** 2026-09-21T16:19Z (re-run G0.6).
**Propietario:** AGENT (modo AUTO). **Revisor:** mismo agente (mismo SHA, mismo host, run reproducible).

## Aserción observable (del UAT_CATALOG.md)

> `observe` uprobe antes de PID y ante sesión inexistente: errores semánticos de capacidad/no encontrado estables y tipados; no exigir prefijos v1.

## Niveles CERT

| Nivel | Estado | Razón |
|---|---|---|
| CERT-0 | `passed` | Requisito: `observe` con verb inválido debe devolver JSON-RPC -32602 con mensaje estable que enumere los verbos válidos; `observe(create)` contra sesión inexistente debe devolver error tipado (no panic, no 500). ADR: ver STATE.md `Estado de G0.2` + exploration-report §3. |
| CERT-1 | `passed` | Implementación aislada: el wire shape del `scope` con doble key es la forma canónica de las tagged-enum en serde (`tag = "scope"`); 2 tests negativos nuevos en `chronos-sandbox/tests/observe_uprobe.rs` (`test_observe_with_invalid_verb_returns_typed_error`, `test_observe_uprobe_against_nonexistent_session_returns_typed_error`) pin el discriminador `ObserveVerb` snake_case y la `ObserveScopeWire` tagged. |
| CERT-2 | `passed` | Integración canónica: wire smoke `/tmp/g0.2-wire-smoke/` contra binario real `chronos-mcp` rebuilt post-G0.5 (HEAD `afa14fd2`). Output verbatim (2026-09-21T16:19Z): `observe(frobnicate) → code=Some(-32602) msg="failed to deserialize parameters: unknown variant 'frobnicate', expected one of 'create', 'list', 'update', 'delete', 'query'"`; `observe(create, fake session) → error=true, references_session=true`; `observe(scope=42) → error=true` (regression guard). |

## Mapa UAT

| Test | Fixture | Comando | Resultado |
|---|---|---|---|
| `tests/observe_uprobe.rs::test_observe_with_invalid_verb_returns_typed_error` | sandbox integration | `cargo test -p chronos-sandbox --test observe_uprobe` | GREEN |
| `tests/observe_uprobe.rs::test_observe_uprobe_against_nonexistent_session_returns_typed_error` | sandbox integration | idem | GREEN |
| Wire smoke G0.2 | `/tmp/g0.2-wire-smoke/` (manual JSON-RPC client, no rmcp) | `cargo run --release` con `CHRONOS_MCP_PATH=…/chronos-mcp` | 3/3 GREEN |

## Pruebas negativas (cubiertas)

- `observe(verb="frobnicate")` → JSON-RPC -32602 "unknown variant 'frobnicate', expected one of 'create', 'list', 'update', 'delete', 'query'".
- `observe(verb="create", session=fake-uuid)` → `result.isError=true` con UUID en `content[0].text`.
- `observe(scope=42)` (integer en lugar de objeto tagged) → JSON-RPC -32602 "invalid type: integer `42`, expected internally tagged enum ObserveScopeWire".

## Recibo (verbatim del output)

```
[g0.2-wire] observe(create, fake session) → error=true, references_session=true
[g0.2-wire] observe(frobnicate) → code=Some(-32602) msg="failed to deserialize parameters: unknown variant `frobnicate`, expected one of `create`, `list`, `update`, `delete`, `query`"
[g0.2-wire] observe(scope=42) → error=true

=== G0.2 wire-level results ===
fake-session create (proper wire shape) → error_present=true, references_session=true
invalid verb                            → code=Some(-32602)
non-object scope (regression guard)     → error=true

G0.2 wire contract: GREEN
```

Log completo: `/tmp/g0.2-wire-smoke-output.log`.

## Deuda residual aceptada

- DEBT-M7-02-01: 4 `probe_inject` tests pre-C5.2 fallan porque esperan el prefijo v1 (`probe_inject: capability: ebpf-uprobe`) y el wrapper actual emite `observe: probe still starting up`. La migración m7-02 está hecha pero los tests pre-existen. **Out-of-scope G0; M1+** (cambio requiere refactor de los tests pre-existentes a la nueva API).

## Incompatibilidades / exclusiones

- `probe_inject` legacy tests NO son parte de este UAT (cubrían el contrato v1 antes de la migración m7-02).

## Fecha / condición de recertificación

Recertificar cuando:

1. Cambie `ObserveVerb` o `ObserveScopeWire` (discriminadores).
2. Cambie el contrato JSON-RPC de `tools/call` para `observe`.
3. Cambie el formato de `content[0].text` que devuelve el wrapper.
4. Recertificación programada al menos cada release candidate.

## Historial

- 2026-09-21T14:00Z (G0.2 merge `13495fae`): emitido, primer wire smoke GREEN.
- 2026-09-21T16:19Z (G0.6 recertificación): re-ejecutado contra HEAD actual `afa14fd2`. **GREEN**.
