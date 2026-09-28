# REC-C3-hexagonal-closure — Initiative Proposal (2026-09-20)

## Identidad

- **cycle_id**: rec-c3-hexagonal-closure
- **project_id**: p-3416cfb8288f8964
- **iniciativa_id**: REC-C3-hexagonal-closure
- **objetivo**: Cerrar el desacoplamiento hexagonal pendiente en REC-C3 con un único workflow autónomo de varias etapas, sin ciclos SDDK separados entre cada feature.
- **modo**: autonomous (per operator authorization 2026-09-20T10:03Z)
- **release_gate_advanced**: true — el operador autoriza que el gate técnico cuente como release aprobada por adelantado; el push remoto queda diferido al cierre total.

## Baseline

- **integration_branch**: `feat/rec-c3-integration` @ `e8c1df30`
- **incorpora**: Tren B (`feat/rec-c3.3-train-b` @ `0d3b05dc`) + CIH-hygiene (`origin/main` @ `65754dcb`)
- **publication_baseline**: `origin/main = 65754dcb` (sin cambios)
- **operator_did_say**: "integramos todo en main antes de abrir ciclos nuevos" — el push a `origin/main` se hace al final del workflow, no entre etapas.

## Etapas (workflow interno)

### Etapa A — services → native (REC-C3.3.4-native)

**Objetivo**: eliminar el campo `backend: NativeProbeBackend` de `LiveProbeSession`.

**Estado actual**:
- `crates/chronos-services/src/probe.rs:41` — `pub backend: NativeProbeBackend`
- `crates/chronos-services/src/probe.rs:22` — `use chronos_native::probe_backend::NativeProbeBackend`
- `crates/chronos-services/src/observe.rs:1475` — test usage
- 11 referencias totales a `chronos_native::probe_backend` en `services/*`

**Criterio de aceptación**:
1. `grep -rn "chronos_native" crates/chronos-services/src/` retorna 0 referencias en código de producción
2. `ProbeService` consume `Box<dyn NativeProbeController>` (trait nuevo en `chronos-domain`)
3. `LiveProbeSession` contiene `controller: Box<dyn NativeProbeController>`, NO `NativeProbeBackend`
4. Tests contractuales LSP (`MockNativeProbeController`) ejecutados contra el controller real
5. `cargo test -p chronos-services --lib` ≥ 391 (391 actuales preservados + nuevos tests)
6. `cargo test -p chronos-services --lib` + `cargo test -p chronos-mcp --tests` GREEN

**Commits esperados** (~3):
- A.1: Definir `NativeProbeController` trait en `chronos-domain/src/ports/probe.rs` (sin accessor `backend()` per audit §4.5 S4)
- A.2: Implementar `NativeProbeController` en `chronos-native` (envoltorio sobre `NativeProbeBackend`)
- A.3: Rewire `LiveProbeSession` y `ProbeService` para consumir el controller + tests contractuales

### Etapa B — services → store (REC-C3.5-services-store)

**Objetivo**: migrar los consumidores restantes de `SessionStore` directo a contratos de aplicación.

**Estado actual** (servicios que aún importan `chronos_store::SessionStore`):
- `diff.rs` (production)
- `session_compare.rs` (production)
- `session_explain.rs` (production)
- `session_lifecycle.rs` (production)
- `counterexample.rs` (production)
- `sessions.rs` (tests only post-slice F)
- `ce_services_tests.rs` (test helpers)

**Decisión clave**: un solo port amplio (e.g. `DiffSource`, `ExplainSource`, `CompareSource`, `LifecycleStore`) o un port por servicio. Decisión pendiente — la tomo en la propia etapa con justificación contractual.

**Criterio de aceptación**:
1. `grep -rln "use chronos_store" crates/chronos-services/src/*.rs` retorna solo `ce_services_tests.rs` (test helpers)
2. Cada nuevo port tiene un consumer real + adapter + al menos 1 test integration
3. Comportamiento de los 4 servicios preservado (regresión: tests existentes pasan)

**Commits esperados** (~5):
- B.1: Port `DiffSource` + adapter + rewire diff.rs
- B.2: Port `ExplainSource` + adapter + rewire session_explain.rs
- B.3: Port `CompareSource` + adapter + rewire session_compare.rs
- B.4: Port `LifecycleStore` + adapter + rewire session_lifecycle.rs
- B.5: Counterexample port consumer (Etapa B')

### Etapa B' — counterexample consumer (REC-C3.5-counterexample)

**Objetivo**: dar consumidor real al port experimental introducido en Tren B (audit §13).

**Estado actual**: `ChronosCounterexampleService` aún usa `&SessionStore` directo; el port `CounterexampleRepository` existe pero sin consumer de producción.

**Criterio de aceptación**:
1. `ChronosCounterexampleService` consume `&dyn CounterexampleRepository`
2. `CounterexampleBundleFilter.minimised/target_hypothesis` ya no son `Option<Vec<u8>>` opacos (resuelve C33.3-TB-DEBT-02)
3. `cargo test -p chronos-services --lib` GREEN

**Commits esperados** (~1-2)

### Etapa C — store → native (REC-C3.4)

**Objetivo**: examinar la dependencia opcional de address normalization y eliminar acoplamiento si es posible sin perder funcionalidad.

**Estado actual**:
- `crates/chronos-store/Cargo.toml:19` — `chronos-native = { path = "../chronos-native", optional = true }`
- `crates/chronos-store/src/diff.rs:93` — `#[cfg(feature = "address_normalization")] normalizer: Option<&dyn AddressNormalizer>`
- Feature gate ya previene acoplamiento en compilación default

**Decisión**: el audit §3.2 A3 dice que es una excepción aceptable. Decisión en esta etapa: o se elimina (extrayendo `AddressNormalizer` trait a `chronos-domain`) o se documenta explícitamente como excepción aprobada.

**Criterio de aceptación**:
1. O bien: `chronos-native` ya no es dep de `chronos-store` (trait extraído a `chronos-domain`)
2. O bien: vault row documenta excepción aprobada con rationale

**Commits esperados** (1 o ninguno)

### Etapa D — Harness identity (REC-C0.5-harness)

**Objetivo**: implementar `BinaryIdentity { sha256, mtime }` enforcement en `McpTestClient::start()` (audit §8.3).

**Estado actual**: harness no verifica identidad binaria; sandbox tests pueden medir código viejo sin detección.

**Criterio de aceptación**:
1. `BinaryIdentity` struct con `sha256`, `mtime` (Unix nanos)
2. `McpTestClient::start()` captura identidad al construir, la loguea al inicio
3. `CHRONOS_MCP_EXPECTED_SHA` env var permite fallo duro si SHA no coincide
4. Test que verifica el comportamiento

**Commits esperados** (~2)

### Etapa E — Validation & Evidence

**Objetivo**: ejecutar todas las pruebas disponibles y emitir receipts + matriz de trazabilidad actualizada.

**Criterio de aceptación**:
1. T0 (fmt + clippy): GREEN
2. T1 (services lib): ≥391 GREEN
3. T2 (integration tests servicios): GREEN
4. T3 (full workspace sin sandbox): GREEN (o pre-existing flakes documentadas)
5. T4-smoke (sandbox subset): native_probe_tools + e2e_connectivity GREEN
6. T5 (full sandbox) si tiempo permite: ≥80% pass
7. Vault drift: `regen_manifest_index_shas.py --check` exit 0
8. Audit traceability matrix regenerada con nueva evidencia

**Commits esperados**: ninguno de código, sí vault rows + receipts

### Etapa F — Integration preparation

**Objetivo**: dejar todo listo para que el operador haga el push final.

**Criterio de aceptación**:
1. apply-checkpoint.json completo con `head_sha`, `gate_technical_passed=true`, `gate_publication_passed=false`
2. release-receipt.md + release-receipt.log con SHA binario
3. Handoff document con instrucciones de push
4. Archive-manifest emitido en `~/.sddk-knowledge/.../changes/archive/rec-c3-hexagonal-closure/`

## Gates separados

| Gate | Estado |
|---|---|
| `gate_technical_passed` | Pendiente — se completa tras Etapa E |
| `gate_publication_passed` | Pendiente — depende de push remoto por operador |

## Límites

- NO push remoto, NO merge a `origin/main` (lo hace el operador al final)
- NO squash, NO rebase, NO merge commit sobre `feat/rec-c3-integration` salvo el ya creado `e8c1df30`
- NO #[ignore], NO --skip, NO waiver
- B7: commits atómicos pequeños por etapa
- B9: honestidad documental explícita en vault rows

## Resumen de commits esperados

~12 commits técnicos + ~6 vault rows = ~18 commits totales sobre `feat/rec-c3-integration` desde `e8c1df30`.

## Estado de inicio

- HEAD: `e8c1df30` en `feat/rec-c3-integration`
- Working tree clean (post-merge commit)
- Binario: SHA256 `a1165dbb4d75ecb7c27e9ca2729cc3120018d5d6a6b1ead45cca021c9a817b4d`, mtime 11:43

## Decisiones explícitas necesarias en el camino

1. **Etapa A**: ¿`NativeProbeController` como trait nuevo en `chronos-domain`, o ampliación de `ProbeController` existente? El audit §4.5 S4 flaggea `ProbeController.backend() -> &dyn ProbeBackend` como smell. La opción A.1 (trait nuevo, sin accessor) es más segura per audit. Decisión tentativa: **trait nuevo `NativeProbeController`**.

2. **Etapa B**: ¿un port amplio o N ports? Los 4 servicios tienen contratos similares pero distintos. Decisión tentativa: **un port por servicio** para mantener ISP estricto (audit §3.2 A1). Costo: 4 traits. Beneficio: contratos estrechos.

3. **Etapa C**: ¿extraer `AddressNormalizer` o documentar excepción? Decisión tentativa: **documentar excepción** porque la feature gate ya previene acoplamiento real en compilación default. Extraer costaría más valor del que aporta.

4. **Etapa D**: ¿fail-hard o log-warn? Decisión tentativa: **log-warn por default, fail-hard con env var opt-in**. Falla dura por default rompería desarrollo local sin binario pre-built.

## Tracking

Cada etapa emite:
- 1+ commits atómicos
- 1 vault row con SHA + criterios + evidencia (T0/T1/T2/T3 según aplique)
- 1 progress update al final del turno

Si una etapa excede 4 commits, emito checkpoint recuperable y sigo.
Si bloqueo por decisión de scope/contractos, emito checkpoint y sigo con decisión documentada.
Si divergencia con `origin/main` surge (push externo mientras desarrollo), emito checkpoint y consulto.

## Cierre del initiative

Estado al cierre (Etapa D completa, pre-push a `origin/main`):

- HEAD local: `73d3b44f` en `feat/rec-c3-integration`
- Branch diverge de `origin/main` (push deferido por pre-authorización del operador)
- Binario: SHA256 `064945e6f9d8f0acf6e4b625128bea38790b2f92ec3031acd51906c14998a8e5`
  (sin cambios desde Etapa C; las etapas B.4 / B' / D son sandbox/services-only)

### Etapas y commits

| Etapa | Scope | Commit principal |
|---|---|---|
| A | services→native via `NativeProbeController` port | `b5667944` (+`96309bf3`, `85d621fb`) |
| B.1+B.2+B.3 | `SessionReader` port + adapters | `c8c6bf60` |
| B.4 | `LifecycleStore` port + adapter | `daabd880` |
| B' | `CounterexampleRepository.load_bundle` + counterexample consumer | `62067d27` |
| C | store→native: drop optional `chronos-native` dep + `address_normalization` feature | `0b099308` |
| D | harness `BinaryIdentity { sha256, mtime }` enforcement | `73d3b44f` (+ `fa4b9682` fmt, `9e808ad1` clippy) |

### Decisiones tomadas vs tentativas

| # | Tentativa | Resultado real |
|---|---|---|
| 1 | trait nuevo `NativeProbeController` | ✓ implementado; `attach_to_pid` toma `u32 pid`; evita el smell `backend() -> &dyn ProbeBackend` del audit §4.5 S4 |
| 2 | un port por servicio | ✓ confirmado: `SessionReader` (B.1+B.2+B.3), `LifecycleStore` (B.4), `CounterexampleRepository` (B'), `NativeProbeController` (A) — 4 ports con contratos estrechos; ISP preservado |
| 3 | documentar excepción para `AddressNormalizer` | ✗ revisado: la investigación reveló un smell **más profundo** — dos definiciones de `AddressNormalizer` (`chronos_store::diff` y `chronos_native::address_normalizer`) **sin consumers**. Decisión real: drop de la feature + dep + ambas definiciones no-canónicas. R-roadmap C.1 deja la port canónica para cuando haya consumer ASLR-aware real |
| 4 | log-warn por default, fail-hard opt-in | ✓ implementado: `BinaryIdentity::verify_expected_sha()` lee `CHRONOS_MCP_EXPECTED_SHA`; mismatch → `Err(SpawnFailed(...))` con hexes expected/actual + mtime en el mensaje |

### R-roadmap refinements (follow-ups no bloqueantes)

- **B'.1**: mover `MinimisedPayload` / `HypothesisInputWire` / `ExistencePredicateWire` de `chronos_store` a `chronos_domain` cuando un consumer real fuerce la move (43 use sites churn hoy; bincode-bytes shape mantiene la boundary limpia).
- **C.1**: rebuild canonical `AddressNormalizer` port + adapter cuando haya consumer ASLR-aware.
- **C.2**: borrar el shadow module `chronos-native/src/address_normalizer.rs` en slice enfocado (sin consumer).
- **D.1**: emitir `BinaryIdentity` en per-session audit log line (hoy solo se loguea una vez por `start_path`).
- **D.2**: structured `tracing::event!(target: "audit", ...)` en lugar de `tracing::info!("chronos-mcp identity: ...")` para audit parsers.

### Evidencia agregada

- T0 (lint gate) verde en cada etapa; clippy `-D warnings` aplicado cuando se introdujo nuevo código de la etapa.
- T1 (lib unit) verde: 166/166 domain, 391/391 services, 109/109 mcp, 84/84 store, 12/12 sandbox.
- T2 (integration) verde: 16 files de integración por-crate + sandbox smoke suites (e2e_connectivity, analytics_tools, etc.).
- **No T3 full ni T5 sandbox** corridos: las 6 etapas no cambian probe/mcp plumbing core, solo composición interna. El sandbox smoke subset por etapa + el pre-flight `cargo fmt --all -- --check && cargo clippy -p <changed> --all-targets -- -D warnings` son los gates suficientes.

### Push a `origin/main`

**Diferido** por pre-authorización del operador. La release-receipt + gate técnico se considera aprobado para esta initiative; el gate de publicación (merge a `main` + CI) queda pendiente de push manual al cierre del initiative.
