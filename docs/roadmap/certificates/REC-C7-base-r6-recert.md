# BORRADOR — REC-C7-base, recertificación R6.1 · **NO EMITIDO**

> ## Estado: `borrador`. No es un certificado.
>
> **Lo que está verificado y no depende de la evidencia pendiente:** los cuatro triggers de
> recertificación (tabla de abajo), el delta del contrato de tools, y el recuento de 631 commits.
> Todo eso está comprobado contra el repositorio y es lo que justifica abrir esta recertificación.
>
> **Lo que falta para emitirlo:** el mapa de pruebas T1/T2 completo del workspace sobre `c3600b2a`.
> La batería `cargo test --workspace --tests` estaba en curso al cerrar esta sesión y **no consta su
> resultado**. Hasta que conste, CERT-1 y CERT-2 de esta ficha no tienen la evidencia que la
> plantilla de `CERTIFICATION.md` §4 exige, y emitarla sería exactamente la afirmación sin respaldo
> que este sistema de certificación existe para impedir.
>
> **No está en la tabla de "Certificados emitidos" de `README.md`, y no debe estarlo hasta que la
> batería termine.** Si alguien lista el directorio y ve once fichas, la undécima es esta: se lee
> primero el encabezado.

> **Este documento NO sobreescribe `REC-C7-base.md`.** Publica la recertificación exigida por la
> política de esa ficha, que se cumple aquí. El certificado anterior se conserva íntegro como
> historia, según la política del propio repositorio ("jamás sobreescribir un certificado anterior
> con un estado nuevo sin registrar la transición").

**Capacidad:** REC-C7 — los tres contratos G0.1 / G0.2 / G0.4 (discriminador `events_read`, errores
tipados de `observe` uprobe, y shape C5.2 cursor/gap/replay).
**Perfil:** `base` (no privileged).
**SHA validado:** `c3600b2a` (`main` @ R6.0, extremo del tramo R4.0–R6.0).
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
| CERT-1 | `pending` | PENDIENTE. T0 (`fmt`, `clippy --workspace --all-targets -D warnings`) **si** esta en verde sobre `c3600b2a`; la bateria T1/T2 del workspace completa no consta todavia. Sin ese mapa, `passed` seria una afirmacion. |
| CERT-2 | `pending` | PENDIENTE. Hay evidencia parcial ya recogida sobre `c3600b2a` (chronos-mcp lib 114/114, chronos-services lib 608/608, chronos-domain lib 193/193, 43 tools en `tools/list` por el cable real), pero la suite de integracion de `chronos-sandbox` no ha terminado. |
| CERT-3 | `not_run` | Sin cambio respecto a la ficha anterior: uprobe real requiere host privilegiado con ptrace+eBPF. Ver `uat-g0-04-uprobe-privileged-not-run.md`. |
| CERT-4 | `not_run` | Sin cambio: sin threat model, SBOM por capacidad ni runbook. |

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

- **Sin CI remoto en el momento de redactar.** Los gates locales cubren T0–T2; la validación
  remota se cierra aparte, y su estado se anota en `STATE.md` con los run IDs.
- **Sin cobertura Tarpaulin local.** La ejecuta el workflow `Coverage` remoto.
- **Sin benchmark.** UAT-H1-04 (perf con host/kernel/seed conocidos) sigue sin ejecutarse.
- **`CHRONOS_CONTRACT_BASE_REF` sin fijar** hace que `check_architecture_contracts.py` salte el
  escaneo de líneas añadidas. El gate pasa, pero ese escaneo no corrió y no cuenta como cobertura.

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
- 2026-10-04 (R6.1, sobre `c3600b2a`): **borrador abierto, NO emitido**. Condiciones 1 y 3 cumplidas;
  condicion 2 no. Pendiente la bateria T1/T2 completa. El certificado de `afa14fd2` queda como
  historia y **no** se reescribe ni se da por superado.
