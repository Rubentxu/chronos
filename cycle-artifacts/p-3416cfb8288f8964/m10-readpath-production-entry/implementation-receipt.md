# implementation-receipt — M10 read path production entry

- cycle: `p-3416cfb8288f8964/m10-readpath-production-entry`
- path: B-direct
- gate: `implementation-complete`
- gate receipt: `gate-implementation-complete-cf05779dc1e234c5-1`
- subject: `896c2f17` → `c45488c4`
- diff digest: `ec722d5b2e6e27e3e2ac0c52ac7167eb4e37253516c3224805534c751688c3ad`

## Blocker 2 (production entry point): premisa falsada

El blocker afirmaba que faltaba una decisión de arquitectura para que el read
path alcanzara un log en producción. **La premisa era falsa.**

`SessionExecutionLogRegistry::get()` ya devuelve un `SessionExecutionLog` owned,
barato de clonar y respaldado por `Arc`. `subscribe_with_log` solo necesita
`&SessionExecutionLog`. No había decisión ADR pendiente: era un malentendido
owned-vs-borrowed.

## Commits

| commit | contenido |
|---|---|
| `896c2f17` | `ReadPathService` en `chronos-services` (poll_batch, summarize, rollup, causality_status) + 12 tests |
| `c45488c4` | cableado MCP: tool `execution_log_read` + `read_path` en `ChronosServer` + 3 tests de contrato |

## Archivos

- `crates/chronos-services/src/read_path.rs`
- `crates/chronos-services/src/lib.rs`
- `crates/chronos-mcp/src/server.rs`

## Invariantes preservados

1. **Una sola autoridad.** `events_read` y `read_path` comparten el mismo
   `Arc<SessionExecutionLogRegistry>` en *ambos* constructores de
   `ChronosServer` (`new` y `with_toolset`). Son dos vistas del mismo log, no
   dos fuentes.
2. **Fail-closed.** Un log inexistente o no disponible produce error. Nunca un
   envelope vacío de éxito: un agente que ve `events: []` sin error concluiría
   que la sesión está callada, que es un falso negativo sobre evidencia.
3. **Aggregates are not evidence.** `execution_log_read` es un discriminador
   distinto de `events_read` por diseño; la descripción del tool lo dice
   explícitamente.
4. **Lock acotado.** El mutex `engines` se toma solo en modo `causality`. Los
   otros tres modos son lecturas puras del log y no serializan detrás de él.
   `QueryEngine` no es `Clone`, así que el engine se pide prestado bajo el guard.

## Verificación ejecutada (OBSERVED, sobre `c45488c4`)

| check | comando | exit | digest |
|---|---|---|---|
| fmt | `cargo fmt --all -- --check` | 0 | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` (cadena vacía) |
| clippy | `cargo clippy -p chronos-mcp --all-targets` | 0 | `b4915759ad15ba3de1a5e08aaf9566db904647abaf265daf44d061943dac6350` |
| mcp tests | `cargo test -p chronos-mcp --lib` | 0 | `da17660e6f61a9e7d459d16ad7448b6c4b4945ca8dc372994bd420dd710e04fe` |
| svc tests | `cargo test -p chronos-services --lib` | 0 | `0a207f4debebba693b62d5208d2083060e505dad360c5b2ad9df32bc7b7e031c` |
| pipelinek | `pipelinek run --db .pipelinek/db.sqlite --control-root .pipelinek/control .pipeline.kts` | 0 | journal: `RunFinished` outcome=success, `StepFailed`=0 |

Resultados: chronos-mcp **87 passed, 0 failed**; chronos-services **539 passed, 0 failed**;
clippy **0 warnings**; pipelinek **4/4 stages, `Pipeline finished with SUCCESS`**.

SHA-256 de `.pipeline.kts`: `ec1d4f77db5025addcf9b349112f0de560379898448724137753d02c33cc0d62`

## Mutation test (OBSERVED)

Sustituir la rama fail-closed de `execution_log_read` por un éxito vacío
(`event_count: 0`) hace fallar `execution_log_read_fails_closed_for_unknown_session`
con `unknown session must fail closed, not return an empty envelope`. El test
no es verde decorativo. El fichero fue restaurado después (16 ocurrencias del
patrón `Err(e) => ...error...` verificadas).

## Cambio de test justifiable

`tool_availability_map_has_42_entries` (conteo mágico) → 
`tool_availability_map_covers_every_registered_tool`.

El test viejo tenía un comentario que advertía explícitamente de no subir el
número a mano. Sustituirlo por una derivación desde el router vivo más una
comprobación de cobertura nombre por nombre hace imposible esa clase de
deriva: un tool registrado pero no descubrible es invisible para el cliente.

## Deuda y decisiones fuera del WorkItem

- **DERIVED:** `execution_log_read` queda en el toolset `auto`, igual que
  `capture_session` y `debug_get_variables`. No se añadió a `native`/`minimal`
  porque no hay requisito que lo pida y esas listas son un allowlist
  deliberado. Decisión registrada, no descuidada.
- **Pendiente, fuera de este WorkItem:** los commits verificados siguen sin push
  y el blocker 1 de `sddk lint` sigue sin resolverse. No bloquea este gate
  pero impide cerrar la iniciativa.
- Archivos untracked preexistentes (`.atl/`, `cycle-artifacts/_suspended-*`,
  `session-handoff/CIH_H_HANDOFF_*`) no tocados ni incluidos.

## Clases de evidencia

- OBSERVED: fmt, clippy, tests, pipelinek, mutation test, Head SHA, diff digest.
- DERIVED: justificación de la colocación en el toolset `auto`; lectura de que
  la premisa del blocker 2 era falsa (contrastada contra el código, no contra
  documentación).
- ASSUMED: ninguno.
- DESCONOCIDO: nada que afecte a la aceptación de este gate.
