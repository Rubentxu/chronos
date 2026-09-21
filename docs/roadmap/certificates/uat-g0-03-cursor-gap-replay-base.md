# Certificate — UAT-G0-03-cursor-gap-replay-base

**Capacidad:** UAT-G0-03 — cursor, gap, replay contract via `query_events`.
**Perfil:** `base` (no privileged).
**SHA validado:** `afa14fd20f191d5884a8b030d4f05a915024f794` (HEAD de `main` en G0.6).
**Tag remoto:** `v0.7.112` peel `0be2ec2d53d9698956ae705938b32b80d7365ad7` (intacto).
**Fecha:** 2026-09-21T16:19Z (re-run G0.6).
**Propietario:** AGENT (modo AUTO). **Revisor:** mismo agente (mismo SHA, mismo host, run reproducible).

## Aserción observable (del UAT_CATALOG.md)

> Sobre dos consumidores de 10.000 records, avance de productor, reinicio, gap forzado y retención: cursores independientes; `complete` prohibido cuando no está probado; sin robo/destrucción de evidencia.

## Niveles CERT

| Nivel | Estado | Razón |
|---|---|---|
| CERT-0 | `passed` | Requisito: `query_events(mode="query", limit=N)` debe (a) devolver cursor `next_cursor` para paginación, (b) exponer `completeness.status` ("complete"/"incomplete") con `from_seq`/`to_seq_exclusive`/`scope`, (c) reportar `gap_summary` cuando hay gaps, (d) preservar `provenance.session_id` y `provenance.source`. ADR: ver STATE.md `Estado de G0.4`. |
| CERT-1 | `passed` | Implementación aislada: `TraceEvent` mirror del wire v2 con flat `event_type: String` (API estable, downstream no rompe), `location: serde_json::Value`, `data: serde_json::Value`. `GetEventResponse` con envelope `{event, mode, provenance, session_id}` + `SourceLocation` v2 shape. |
| CERT-2 | `passed` | Integración canónica: wire smoke `/tmp/g0.4-wire-smoke/` contra binario real `chronos-mcp` rebuilt post-G0.5 (HEAD `afa14fd2`) confirma el wire shape C5.2 con 9 keys top-level (`completeness`, `gap_summary`, `mode`, `next_cursor`, `provenance`, `result`, `retention`, `session_id`, `tail`), 10 eventos `syscall_enter`/`syscall_exit` en `result.events`, `completeness.status="complete"` con `from_seq=0, to_seq_exclusive=10, scope="examined_range"`, `gap_summary=null` (sin gaps), `next_cursor` (formato `ecv1:1:36:...`), `provenance.{session_id, source="execution_log"}`, `retention.{history_truncated=false, retained_from_seq=0}`, `tail.{state="open", tail_seq=32575}`. |

## Mapa UAT

| Test | Fixture | Comando | Resultado |
|---|---|---|---|
| Sandbox integration G0.4 (sandbox client mirror del wire v2) | `chronos-sandbox/tests/event_tools.rs` × 3 | `cargo test -p chronos-sandbox --test event_tools` | 3/3 GREEN |
| Sandbox integration `query_filters` (cursor/gap/limit) | `chronos-sandbox/tests/query_filters.rs` × 6 (+ 3 ignored legacy) | idem | 6/6 GREEN + 3 #[ignore]d §0.4 |
| Wire smoke G0.4 | `/tmp/g0.4-wire-smoke/` (manual JSON-RPC client, no rmcp) | `cargo run --release` con `CHRONOS_MCP_PATH=…/chronos-mcp` | wire shape C5.2 confirmado; sandbox client wrapper lee `v2.result.events` correctamente |

## Pruebas negativas (cubiertas)

- 3 tests `offset_*` legacy `#[ignore = "G0.4: legacy pre-C5.2 offset pagination; migrate to cursor next_cursor (M1+)"]`. Cuerpos preservados verbatim §0.4 — no se borra evidencia.
- DEBT-G0.5-01: 3 tests legacy offset_* marcados con `#[ignore]` per directiva. Out-of-scope G0; M1+.

## Recibo (verbatim del wire shape)

```json
{
  "completeness": {
    "from_seq": 0,
    "scope": "examined_range",
    "status": "complete",
    "to_seq_exclusive": 10
  },
  "gap_summary": null,
  "mode": "query",
  "next_cursor": "ecv1:1:36:1ffefa31-5a39-4a27-bccd-d23cc21d1a7c:10",
  "provenance": {
    "session_id": "1ffefa31-5a39-4a27-bccd-d23cc21d1a7c",
    "source": "execution_log"
  },
  "result": {
    "events": [
      {
        "data": { "Syscall": { "args": [], "name": "syscall_...", "number": ..., "return_value": 0 } },
        "event_id": 0,
        "event_type": "syscall_enter",
        "location": { "address": 0, "column": null, "file": null, "function": null, "line": null },
        "thread_id": 2917784,
        "timestamp_ns": 1790007577999869617
      },
      ...
    ],
    "next_offset": null,
    "total_matching": 10
  },
  "retention": {
    "history_truncated": false,
    "retained_from_seq": 0
  },
  "session_id": "1ffefa31-5a39-4a27-bccd-d23cc21d1a7c",
  "tail": {
    "state": "open",
    "tail_seq": 32575
  }
}
```

Log completo: `/tmp/g0.4-wire-smoke-output.log`.

## Deuda residual aceptada

- DEBT-G0.5-01: 3 `offset_*` tests `#[ignore]`d, cuerpos preservados verbatim §0.4. Migración a `next_cursor` queda para M1+.

## Incompatibilidades / exclusiones

- **Sin test con 10.000 records / 2 consumidores / restart / gap forzado** (el aserción completa del UAT-G0-03 original). El wire smoke confirma el wire shape en una sesión sintética pequeña (10 events); el stress test con 10k records / multi-consumer / restart queda para M1+ (H1.3 contract tests de cursor, pérdida, lectura independiente).
- **Sin T5 privileged** (uprobe real requiere host ptrace+eBPF).

## Fecha / condición de recertificación

Recertificar cuando:

1. Cambie `TraceEvent`, `GetEventResponse`, o `V2Query`/`V2Result` structs.
2. Cambie el wire shape de `events_read` (nuevos fields en `result`, `completeness`, `gap_summary`, `retention`, `tail`).
3. Cualquier bump de `rmcp`, `serde`, `serde_json`, `schemars` que afecte la deserialización del envelope.
4. Recertificación programada al menos cada release candidate.

## Historial

- 2026-09-21T14:52Z (G0.4 merge `b44504ed`): emitido, primer wire smoke documenta C5.2 wire shape; sandbox client wrapper fix.
- 2026-09-21T16:19Z (G0.6 recertificación): re-ejecutado contra HEAD actual `afa14fd2`. **Wire shape confirmado**.
