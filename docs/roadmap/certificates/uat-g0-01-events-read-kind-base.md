# Certificate — UAT-G0-01-events-read-kind-base

**Capacidad:** UAT-G0-01 — `events_read` JSON-RPC discriminator contract.
**Perfil:** `base` (no privileged).
**SHA validado:** `afa14fd20f191d5884a8b030d4f05a915024f794` (HEAD de `main` en G0.6).
**Tag remoto:** `v0.7.112` peel `0be2ec2d53d9698956ae705938b32b80d7365ad7` (intacto).
**Fecha:** 2026-09-21T16:18Z (re-run G0.6).
**Propietario:** AGENT (modo AUTO). **Revisor:** mismo agente (mismo SHA, mismo host, run reproducible).

## Aserción observable (del UAT_CATALOG.md)

> Invocar `events_read` por JSON-RPC con `mode=query` y `mode=by_id`: schema publicado, Serde, herramienta y respuesta coinciden. Discriminador inválido produce error tipado, nunca fallback.

## Niveles CERT

| Nivel | Estado | Razón |
|---|---|---|
| CERT-0 | `passed` | Requisito: tres contratos (JSON Schema publicado, Serde derive, JSON-RPC client wrapper) deben aceptar `"query"` y `"by_id"`, y rechazar variantes inválidas con error tipado. ADR: ver STATE.md `Estado de G0.1`. |
| CERT-1 | `passed` | Implementación aislada: `crates/chronos-services/src/output.rs:1587` deriva `Serialize` + `#[serde(rename_all = "snake_case")]` en `EventsReadKind`. 9/9 contract tests en `crates/chronos-services/tests/events_read_kind.rs` GREEN sobre `afa14fd2`. |
| CERT-2 | `passed` | Integración canónica: wire smoke `/tmp/g0.1-wire-smoke/` contra binario real `chronos-mcp` rebuilt post-G0.5 (HEAD `afa14fd2`). Output verbatim (2026-09-21T16:18Z): `query → code=None msg=""`, `by_id → code=None msg=""`, `Pascal → code=Some(-32602) msg="failed to deserialize parameters: unknown variant 'Query', expected 'query' or 'by_id'"`. |

## Mapa UAT

| Test | Fixture | Comando | Resultado |
|---|---|---|---|
| `events_read_kind.rs` × 9 | crate-internal unit tests | `cargo test -p chronos-services --test events_read_kind` | 9/9 GREEN |
| Wire smoke G0.1 | `/tmp/g0.1-wire-smoke/` (manual JSON-RPC client, no rmcp) | `cargo run --release` con `CHRONOS_MCP_PATH=…/chronos-mcp` | 3/3 GREEN |

## Pruebas negativas (cubiertas)

- `mode="Query"` (capital) → -32602 "unknown variant 'Query', expected 'query' or 'by_id'".
- `mode="unknown"` → idem.
- Tests unitarios cubren 9 variantes negativas adicionales en `events_read_kind.rs`.

## Recibo (verbatim del output)

```
[g0.1-wire] store_path = /home/rubentxu/.jcode/scratch/g0.1-wire-smoke-2912975/sessions.redb
[g0.1-wire] initialize OK
[g0.1-wire] tools/list returned 41 tools
[g0.1-wire] tools/list advertises `events_read`

=== G0.1 wire-level results ===
query   → code=None msg=""
by_id   → code=None msg=""
Pascal  → code=Some(-32602) msg="failed to deserialize parameters: unknown variant `Query`, expected `query` or `by_id`"

G0.1 wire contract: GREEN
```

Log completo: `/tmp/g0.1-wire-smoke-output.log`.

## Deuda residual aceptada

Ninguna.

## Incompatibilidades / exclusiones

Ninguna para este UAT.

## Fecha / condición de recertificación

Recertificar cuando:

1. Cambie `crates/chronos-services/src/output.rs:1587` (`EventsReadKind` enum o sus derives).
2. Cambie `crates/chronos-services/src/output.rs` (handler que lee el discriminador).
3. Cambie el wire shape de `events_read` en el binario MCP (cualquier bump de `rmcp` o `serde`).
4. Recertificación programada al menos cada release candidate.

## Historial

- 2026-09-21T13:30Z (G0.1 merge `0f773810`): emitido, primer wire smoke GREEN.
- 2026-09-21T16:18Z (G0.6 recertificación): re-ejecutado contra HEAD actual `afa14fd2`. **GREEN**.
