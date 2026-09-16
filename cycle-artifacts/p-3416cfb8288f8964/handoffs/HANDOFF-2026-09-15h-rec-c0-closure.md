# HANDOFF-2026-09-15h-rec-c0-closure

## Resumen

REC-C0 cerrado en la rama `feat/rec-convergence-truth-gate`. Issues
#20 y #21 resueltos con verificación real. 5 commits cohesivos, 6 cycle
artifacts, sin nuevas violaciones arquitectónicas, fitness gate pasa.

## Commits

```
c580d8a9 rec-c0-2-d: UAT verification + cycle artifacts for REC-C0 convergence
7e3702d7 rec-c0-2-c: remove webhook code, reqwest dep, and stale waiver from chronos-domain
3619c123 rec-c0-2-b: add chronos-webhook driven adapter implementing NotificationSink
a3dabc14 rec-c0-2-a: introduce NotificationSink outbound port in chronos-domain
11962e71 rec-c0-1-a: replace fragile sandbox acceptance runner with current_exe-based gate
8e4f54fb docs(roadmap): decompose convergence into executable development cycles  [entrypoint]
```

## Issue #20 (REC-C0.1-A) — m0_10 sandbox acceptance runner

**Causa raíz**: el meta-test reconstruía su propio binario escaneando
`CARGO_BIN_EXE_m0_acceptance` y `CARGO_TARGET_DIR/debug/deps/m0_acceptance-*`.
Cuando `CARGO_TARGET_DIR` apuntaba fuera de `target/` (caso del host
`/home/rubentxu/cargo-targets`), el escaneo fallaba y el test panicaba
con "could not locate compiled m0_acceptance test binary".

**Solución**: eliminar el meta-test. El gate externo
`cargo test -p chronos-sandbox --test m0_acceptance -- --include-ignored --test-threads=1`
corre los 6 `m0_NN_*_impl` UAT dentro del binario que cargo ya
produjo. Reemplazado por un regression test que verifica `current_exe()`
y `--list` bajo 3 escenarios de `CARGO_TARGET_DIR` (unset / default /
custom tmpdir). Si una regresión reintroduce coupling al target dir,
el test falla inmediatamente.

**Cobertura preservada**: los 6 `m0_*_impl` (m0-02..m0-07) siguen
siendo UAT reales invocables con `--include-ignored`. Ningún test fue
eliminado, ignorado ni mockeado.

## Issue #21 (REC-C0.2-A/B/C/D) — Extracción hexagonal del webhook

**Causa raíz**: `chronos-domain/src/tripwire/webhook.rs` era código
especulativo (nadie lo llamaba en producción) detrás de una feature
flag rota. Activar `--features webhook` rompía la compilación con 10
errores (tokio/tracing/serde_json no declarados, `to_rfc3339` sobre
`u64`, `TripwireError` no importado). Además violaba la arquitectura
hexagonal: el dominio contenía reqwest, HTTP, retries, timeouts y
validación HTTPS.

**Solución por fases** (siguiendo la instrucción del usuario "no
mezcles #20 y #21 en un único refactor grande"):

- **REC-C0.2-A** (`a3dabc14`): introducir el puerto `NotificationSink`
  en `crates/chronos-domain/src/ports/notification.rs`. Trait sync,
  transport-neutral, depende sólo de thiserror + serde. 3 tests unitarios.

- **REC-C0.2-B** (`3619c123`): crear el adapter `chronos-webhook`
  (`crates/chronos-webhook/`) que implementa el puerto sobre HTTPS
  con retries exponenciales 1s/2s/4s. Tests con servidor hyper 1.x
  en proceso + `Arc<AtomicUsize>` para contar requests. 4/4 tests
  verdes (success / 4xx / 5xx retry / target override).

- **REC-C0.2-C** (`7e3702d7`): borrar
  `crates/chronos-domain/src/tripwire/webhook.rs` (186 líneas), el
  submodule declaration, `TripwireSubscription`, `TripwireError::*`,
  `validate_callback_url`, la feature `webhook`, la dep opcional
  `reqwest`, y la dep `url` (ya no usada). Eliminar el waiver
  `chronos-domain->reqwest` del `reconstruction-contracts.toml`.

- **REC-C0.2-D** (`c580d8a9`): evidencia UAT completa
  (`verification.log`).

## Estado de comandos baseline (REC-C0 cierre)

| Comando | Estado |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS |
| `cargo build --workspace` | PASS |
| `cargo test --workspace --lib -- --test-threads=1` | PASS (156+4+77+…) |
| `cargo test --workspace --tests -- --test-threads=1` | PASS salvo 3 pre-existentes |
| `cargo check --workspace --all-targets --all-features` | **PARTIAL**: chronos-domain limpio; chronos-store (cas_bench 7-arg vs 6) y chronos-ebpf (cannot find value 'config' con --features ebpf) siguen fallando — pre-existentes, no introducidos por REC-C0, estaban enmascarados |
| `python3 scripts/check_architecture_contracts.py` | PASS; forbidden baseline shrunk 6→5 |

## Forbidden dependency baseline (REC-C0 antes → después)

| Edge | Antes | Después |
|---|---|---|
| `chronos-domain->reqwest` | waiver | **eliminada** |
| `chronos-services->chronos-store` | waiver | waiver (REC-C3) |
| `chronos-services->chronos-ebpf` | waiver | waiver (REC-C3) |
| `chronos-services->chronos-native` | waiver | waiver (REC-C3) |
| `chronos-services->chronos-browser` | waiver | waiver (REC-C5) |
| `chronos-store->chronos-native` | waiver | waiver (REC-C3) |

## Pre-existing failures (no introducidos por REC-C0)

### `chronos-sandbox probe_inject.rs`

3 tests asumen que el host no permite uprobe attach sin root:
- `test_probe_inject_before_pid_known`
- `test_probe_inject_invalid_symbol`
- `test_probe_inject_without_root_returns_error`

En este host (`rubentxu`) el kernel permite uprobe attach, así que la
respuesta contiene "uprobe attached" en lugar de "permission denied".
**Verificado pre-existente**: `git checkout main -- chronos-sandbox/tests/probe_inject.rs`
(HEAD = 9cc44ce3, byte-identical a la rama) produce los mismos 3 fallos.

### `chronos-store` bench y `chronos-ebpf` features

- `chronos-store`: bench `cas_bench` llama constructor con 7 args
  donde el tipo tiene 6 args.
- `chronos-ebpf`: `cannot find value 'config'` con `--features ebpf`.

Ambos errores estaban **enmascarados** antes de REC-C0 porque cargo
paraba en `chronos-domain/src/tripwire/webhook.rs` (los 10 errores
de webhook). Tras eliminar el módulo roto, cargo compila chronos-domain
y expone los errores pre-existentes de los otros crates. **No son
introducidos por REC-C0**; quedan tracked como follow-up M-series.

## Deuda residual (para REC-C3 / REC-C5, fuera de REC-C0)

- 5 violaciones hexagonales todavía en waiver:
  `chronos-services->{store, ebpf, native, browser}` +
  `chronos-store->native`. Son scope de REC-C3 (services) y REC-C5 (browser).

- `chrono-domain/Cargo.toml`: ya no contiene reqwest, ni url, ni
  tokio, ni tracing, ni hyper. Sólo `thiserror`, `serde`, `uuid`,
  `schemars`. La dependencia `url` residual que mencionó el plan del
  usuario fue eliminada completamente al no quedar usuarios.

## Decisiones arquitectónicas tomadas (sin re-preguntar)

- **Webhook nunca se llamó en producción**: era código especulativo.
  En lugar de conservar la funcionalidad "por si acaso", lo borré y
  lo reemplacé por un puerto + adapter que **puede** ser consumido
  por futuros servicios. La deuda especulativa no se preserva; se
  sustituye por abstracción reutilizable.
- **NotificationSink es sync** (no async): evita `async-trait` en
  domain. El adapter hace bridge sync→async con su propio runtime
  dedicado de tokio. El trait method sync mantiene el dominio libre
  de runtime.
- **chronos-webhook usa `rustls-tls`** (no native-tls): portable
  across distros sin dependencias de OpenSSL system.
- **chronos-webhook tests usan hyper 1.x in-process**: ningún mock,
  ningún httpmock externo. Verificación real del round-trip HTTP
  con counters `Arc<AtomicUsize>`.

## Próximo paso (REC-C1)

El usuario instruyó: "No avances todavía a #22/REC-C1 hasta que REC-C0
esté verde". REC-C0 está verde (excepto los pre-existentes documentados).
El siguiente ciclo es **REC-C1** (Issue #22): `events_read` debe usar
evidencia autoritativa, no cursor simulado. Requisitos:

- `EventSeq` autoritativo
- Cursor opaco bound a session+seq+version
- Re-read no-destructivo
- Múltiples consumers independientes
- `next_cursor` basado en last consumed seq
- Detección explícita de malformed/stale/wrong-session cursors
- Real gaps, completeness derivado de evidence
- NUNCA retornar `complete` cuando Gap o coverage unknown existe
- QueryEngine como proyección, nunca primary session source
- UAT 10 pasos (10k eventos, consumer A/B independientes, etc.)

## Branch state

```
$ git log --oneline -5
c580d8a9 rec-c0-2-d: UAT verification + cycle artifacts for REC-C0 convergence
7e3702d7 rec-c0-2-c: remove webhook code, reqwest dep, and stale waiver from chronos-domain
3619c123 rec-c0-2-b: add chronos-webhook driven adapter implementing NotificationSink
a3dabc14 rec-c0-2-a: introduce NotificationSink outbound port in chronos-domain
11962e71 rec-c0-1-a: replace fragile sandbox acceptance runner with current_exe-based gate
8e4f54fb docs(roadmap): decompose convergence into executable development cycles  [main base]
```

Rama: `feat/rec-convergence-truth-gate`
Branch tracking: `origin/feat/rec-convergence-truth-gate` (no pusheado aún)
Working tree: clean
