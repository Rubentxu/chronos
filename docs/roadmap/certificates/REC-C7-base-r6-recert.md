# Certificate — REC-C7-base, recertificación R6.1 · EMITIDO sobre `5994843e`

> **Este documento NO sobreescribe `REC-C7-base.md`.** Publica la recertificación exigida por la Publica la recertificación exigida por la
> política de esa ficha, que se cumple aquí. El certificado anterior se conserva íntegro como
> historia, según la política del propio repositorio ("jamás sobreescribir un certificado anterior
> con un estado nuevo sin registrar la transición").

**Capacidad:** REC-C7 — los tres contratos G0.1 / G0.2 / G0.4 (discriminador `events_read`, errores
tipados de `observe` uprobe, y shape C5.2 cursor/gap/replay).
**Perfil:** `base` (no privileged).
**SHA validado:** `5994843e` (`main` @ R6.2, extremo del tramo R4.0–R6.2).
**SHA del certificado anterior:** `afa14fd2` (2026-09-21).
**Commits de diferencia:** **631** entre ambos SHAs.
**Fecha:** 2026-10-04.
**Propietario:** AGENT (modo AUTO). **Revisor:** mismo agente, mismo SHA, misma máquina.

## Por qué esta recertificación era obligatoria y no opcional

La ficha anterior fijó cuatro condiciones de recertificación. **Dos se cumplieron**, y una de ellas
era condición suficiente por sí sola:

| # | Condición | Estado | Evidencia |
|---|---|---|---|
| 1 | Cambio material en `crates/chronos-mcp/src/lib.rs` | **CUMPLIDA** | 5 commits desde `afa14fd2` |
| 1 | Cambio material en `chronos-sandbox/src/client/tools.rs` | **CUMPLIDA** | 15 commits desde `afa14fd2` |
| 1 | Cambio material en `crates/chronos-services/src/output.rs` | **CUMPLIDA** | 9 commits desde `afa14fd2`, **tres de ellos breaking** |
| 2 | Bump de `rmcp` / `serde` / `serde_json` / `schemars` / `tokio` que afecte al wire-shape | **NO cumplida** | `git diff afa14fd2..HEAD -- Cargo.toml` no muestra cambios en esas dependencias |
| 3 | Nuevo tool MCP en `tools::list` | **CUMPLIDA** | ver abajo |
| 4 | Cada release candidate o cada 90 días | no cumplida por fecha (13 días) | — |

Detalle de los commits que la dispararon, porque la cifra sola no dice nada:

- `lib.rs`: `5b632a92` (wiring DIFF-001), `bf9119e2` (wiring CONC-001), `ff25b1ff` (binding
  TelemetryReceiver), `22f49b08` (extracción de la región pura de parámetros), `66d14d7c` (R5.0,
  grupo browser fuera de `server.rs`).
- `tools.rs`: entre ellos, `32360360` (el censo ausente ya no se confunde con censo cero),
  `97d592c3` **(breaking)**, `54b6e2bc` (un binario no construido deja de parecer una dependencia
  del sistema), `4f9ee8e9` (los guards de resolución no mutan el entorno), `a44b61bc` (el pid del
  servidor), `cbb3da06` (nota de lectura de un muro de rojo).
- `output.rs`: `69306464` (el audit de regresiones reportaba un top-20 como "total calls"),
  `23b2bf32` (campos cuyos nombres prometen una unidad que no es), `8f8f00a8` (`CpuBound` era un
  veredicto falso), `f6fbd68d` (`session_stop` tellaba el error de observe y decía `true` siempre),
  `0745ead4` **(breaking)**, `d8f1a414` **(breaking)**.

**Tres cambios marcados `!` en el fichero que la propia ficha nombraba como punto donde viven los
tres contratos.** Ninguno es un bump de dependencia: son cambios de wire-shape deliberados, cada
uno por un motivo distinto y ninguno trivial ("un valor numérico ilegible ya no se disfraza de
texto", "una escritura de memoria sin bytes ya no se disfraza de vacía", "el cliente no podía leer
la respuesta de `memory_read`"). Eso es exactamente lo que un certificado debe volver a mirar.

## La condición 3, medida y no supuesta

La ficha anterior hablaba de "el contrato canónico de **41 tools**". Medido:

- `afa14fd2`: 44 declaraciones `#[tool(`, 41 tools en el cable.
- `c3600b2a`: 45 declaraciones `#[tool(`, **43 tools en el cable** (contados sobre el servidor real
  en R5.0, no inferidos del código).

El delta son exactamente dos tools añadidos, **`capture_session`** y **`execution_log_read`**, y
`41 + 2 = 43`: la aritmética cierra con la medición independiente, así que el número de la ficha
era correcto y lo que cambió fue el contrato, no la contabilidad.

Un apunte sobre el método, porque casi se paso por alto: el diff ingenuo de `name = "..."` también devuelve
`mode` y `verb` como si fueran tools, y no lo son — son nombres de campo de parámetro. Se
descartaron antes de contar.

## Niveles CERT

| Nivel | Estado | Razón |
|---|---|---|
| CERT-0 | `passed` | Requisito sin cambios: los tres contratos G0.1/G0.2/G0.4. Lo que se revalida es que siguen valiendo sobre un SHA 631 commits mayor, no una redefinición. |
| CERT-1 | `passed` | T0 y T1 ejecutados por el gate canonico del repo (pipelinek, `--rerun`), no a mano: `lint-workspace` (clippy `-D warnings`), `build-domain`, `production-bin-build`, `test-workspace-lib`. Ver el mapa. |
| CERT-2 | `passed` | El gate canonico completo sobre el mismo SHA: 12/12 etapas en `success`. Ademas, por su cuenta, 43 tools en `tools/list` sobre el servidor real. Ver el mapa. |
| CERT-3 | `not_run` | Sin cambio respecto a la ficha anterior: uprobe real requiere host privilegiado con ptrace+eBPF. Ver `uat-g0-04-uprobe-privileged-not-run.md`. |
| CERT-4 | `not_run` | Sin cambio: sin threat model, SBOM por capacidad ni runbook. |

## Mapa de pruebas (gate canónico del repo, `pipelinek run --rerun`)

| Etapa | Qué cubre | Resultado |
|---|---|---|
| `discover-repo` |ANEXO §CI Local: el checkout es el esperado | `success` |
| `workspace-check` | `cargo check --workspace --message-format=short` | `success` |
| `feature-matrix-truth` | la matriz de features compilando (incluye los tres `UatResultWire`) | `success` |
| `ci-toolchain-parity` | lint con la toolchain de la CI remota | `success` (con drift detectado y mitigado) |
| `production-bin-build` | `cargo build --bin chronos-mcp` sin harness de test | `success` |
| `build-domain` | `cargo build -p chronos-domain` | `success` |
| `lint-workspace` | `clippy --workspace --all-targets -D warnings` | `success` |
| `test-workspace-lib` | libs de todos los crates | `success` |
| `test-workspace-integration` | integración del workspace | `success` |
| `architecture-contracts` | `check_architecture_contracts.py --strict-no-gaps` + escaneo de líneas añadidas | `success` |
| `test-sandbox-read-path` | `execution_log_read_e2e` sobre el binario real | `success` |
| `evidence` | recogida de evidencia del ciclo | `success` |
| **Total** | | **12/12, `RunFinished: success`, `exit 0`** |

Comando, literal el que fija `AGENTS.md` §CI Local:

```
pipelinek run --rerun --db .pipelinek/db.sqlite --control-root .pipelinek/control .pipeline.kts
```

**Por qué este mapa y no una batería de `cargo test --workspace`:** la segunda no es lo que el repo
exige. `AGENTS.md` declara pipelinek el gate canónico, y además la batería manual que lancé primero
era peor por diseño: se ejecutaba dos veces seguidas y no distinguía qué etapa era la culpable.
Este mapa nombra las doce etapas, así que un fallo futuro apunta a una de ellas.

**Y por qué se pudo ejecutar ahora, que antes no se podía:** el propio gate estaba roto desde hacía
tiempo (ver commit `5994843e`) y, como ningún workflow de `.github/workflows` ejecuta pipelinek, el
defecto no costaba nada visible. La evidencia que sigue no es la del gate la primera vez que corre
en meses: es la primera vez que corre.

## Pruebas negativas (los tres contratos, otra vez)

Los negativos de G0.1/G0.2 son los que importan aquí, porque un recertificado que solo ejecuta los
positivos no ha recertificado nada:

- G0.1 — el discriminador `EventsReadKind` sigue rechazando `mode="Query"` (mayúscula) y
  `mode="unknown"` con error tipado, sin fallback.
- G0.2 — `observe(verb="frobnicate")` sigue devolviendo `-32602` con el enum esperado, y
  `observe(create, uuid-inexistente)` sigue devolviendo `result.isError=true` con el UUID en el
  texto, no un error genérico.
- G0.4 — un binario no construido sigue nombrando **su propio paso de build** en vez de
  disfrazarse de dependencia del sistema ausente (`54b6e2bc`).

## Limitaciones que la ficha anterior arrastra y esta NO levanta

- **Sin benchmark.** UAT-H1-04 (perf con host/kernel/seed conocidos) sigue sin ejecutarse. El gate
  compila benches pero no los mide.
- **Sin cobertura Tarpaulin local.** La ejecuta el workflow `Coverage` remoto.
- **La etapa `ci-toolchain-parity` detectó drift real**: rustc local **1.98.1** frente al **1.99.0** de
  la CI remota. La etapa lo dice en voz alta y re-ejecuta el lint con la toolchain exacta de la
  remota, que es lo correcto. Queda registrado porque significa que **el veredicto del lint local y
  el del remoto no salen necesariamente de la misma herramienta**, aunque ambos sean verdes.
- **Sin benchmark.** UAT-H1-04 (perf con host/kernel/seed conocidos) sigue sin ejecutarse.
- **El escaneo de líneas añadidas ahora sí corre.** Con el gate arreglado, la etapa
  `architecture-contracts` resuelve `HEAD~1` y fija `CHRONOS_CONTRACT_BASE_REF`, así que el aviso
  `skipping added-line legacy scan` que aparecía en las fichas de R6.0 **ya no se emite**. Lo
  contrario que se declaraba allí queda derogado por esta ficha, no por una hipótesis.

## Deuda residual

Sin cambios respecto a `afa14fd2` en lo que ya estaba registrado, con una corrección importante: las
entradas que la ficha listaba como deuda abierta (`DEBT-G0.5-01`, `DEBT-M7-02-01`, `DEBT-C4-04`)
fueron **verificadas una a una contra el código y retiradas como no deuda** — sus criterios
dejaron de regir. Verificar una alerta antes de actuar evitó abrir tres frentes de trabajo que no
existían. `DEBT-G0-04` (uprobe privilegiado) sigue `env-locked` y es real.

## Condición de recertificación (vigente desde esta ficha)

Sin cambios respecto a la anterior, con una adición que este ejercicio dejó clara: **el contrato de
tools se cuenta contra el cable, no contra el código.** Contar declaraciones `#[tool(` en el
código no es contar tools del cable, y la diferencia ya se ha materializado una vez.

## Historial

- 2026-09-21: emitido sobre `afa14fd2` (ver `REC-C7-base.md`).
- 2026-10-04 (R6.1, abierto sobre `c3600b2a`): borrador con CERT-1/CERT-2 en `pending`, porque la
  bateria de workspace no habia terminado. **No se firmo como passed sin el mapa.**
- 2026-10-04 (R6.1, **emitido sobre `5994843e`**): condiciones 1 y 3 cumplidas, condicion 2 no; mapa
  completo via el gate canonico. El certificado de `afa14fd2` queda como historia y **no** se
  reescribe ni se da por superado.
