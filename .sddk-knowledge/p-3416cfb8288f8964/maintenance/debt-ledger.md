# Pre-existing debt ledger (from REC-C4 verify, 2026-09-20)

Debt items found during REC-C4 verification that reproduce on `origin/main`
(75d04447) and are therefore NOT regressions of this cycle. Recorded per rule
B5 (no waivers — pre-existing failures get debt entries).

| ID | Source | Description | Repro |
|---|---|---|---|
| DEBT-C4-01 | vault drift CC#11 | `rec-c3.3-train-b` suspended cycle stuck in `apply_complete_pending_capture_session`, expected `CLOSED`. Suspended artifact dirs (`cycle-artifacts/_suspended-rec-c3.3-train-b/`) are untracked leftovers. | `bash scripts/check_vault_drift.sh` | **RETRACTED 2026-10-03 (R3.4): NOT real debt.** Verified, not assumed. The cycle is no longer stuck: `apply-checkpoint.json` for `rec-c3.3-train-b` reads `status: CLOSED`, which is the field CC#11 checks. A `cycle-artifacts/_suspended-rec-c3.3-train-b` directory still exists as filesystem residue, but it is not what the check reads, and `bash scripts/check_vault_drift.sh` reports CC#11 clean.
| DEBT-C4-02 | vault drift CC#18 | Missing `verify-findings.json` for cycles rec-c3-hexagonal-closure, rec-c3.3-train-b, rec-c3.3.4-native, rec-c3.5-residual-inversion, rec-c4-solid-connascence | same | **RETRACTED 2026-10-03 (R3.4): NOT real debt.** Verified, not assumed. **All five cycles now HAVE `verify-findings.json`** — `rec-c3-hexagonal-closure`, `rec-c3.3-train-b`, `rec-c3.3.4-native`, `rec-c3.5-residual-inversion`, `rec-c4-solid-connascence`: every one resolves to `yes` on disk and every one is git-tracked. On top of that, CC#18 itself was amended by the R0 audit to skip non-`CLOSED` cycles, so it no longer reports these at all. The entry describes a state from before R0's filter and before the artifacts were synthesized.
| DEBT-C4-03 | vault drift CC#22 | `rec-c3.3-train-b/release-receipt.md` missing Head SHA / Remote tag / tag_peel / Peel match fields | same | **RETRACTED 2026-10-03 (R3.4): NOT real debt.** Verified, not assumed. **All four fields are present** in `rec-c3.3-train-b/release-receipt.md`, and the peel genuinely matches: `Head SHA a1c628e6c5f31ba3d224d93461781ce828ba0b91`, `Remote tag rec-c3.3-train-b.0`, `Remote tag_peel a1c628e6c5f31ba3d224d93461781ce828ba0b91`, `Peel match true`, plus a `tag_peel_match: true` field. CC#22 is green.
| DEBT-C4-04 | vault drift CC#56 | `chronos-sandbox/src/client/identity.rs:151-164` mutates process env (`std::env::set_var`/`remove_var` for `CHRONOS_MCP_EXPECTED_SHA`). Test-only helper; pre-exists on main. **Closed in G0.3** (commit pending): refactored to `verify_expected_sha_value(Option<&str>)` explicit parameter; tests call it directly. | same | **RETRACTED 2026-10-03 (R3.4): NOT real debt.** Verified, not assumed. **There is no environment mutation left to fix.** `grep -nE '^[^/]*std::env::(set_var|remove_var)\s*\(' chronos-sandbox/src/client/identity.rs` returns **nothing**: zero executable calls. What survives is one PROSE mention at `identity.rs:104` — a doc comment on `verify_expected_sha` that says the old test helper used `std::env::set_var` and that vault drift forbids it. That is the comment explaining WHY `verify_expected_sha` delegates to `verify_expected_sha_value(expected: Option<&str>)`, which IS the m9-73 fix, and CC#56's regex requires a `(` after the name so prose does not match. The debt is satisfied; what remains is the sentence that documents the satisfaction.
| DEBT-G0.4-01 | C5.2 sandbox-client migration gap | Sandbox client `query_events` wrapper was reading `events` at inner JSON root, but server v2 publishes `events` nested in `result` envelope (refactor C5.2). **Closed in G0.4** (commit `b55efb8f`): wrapper now mirrors V2 envelope and reads `v2.result.events`. | n/a (closed) |
| DEBT-G0.4-02 | G0.2 regression in `tests/observe_uprobe.rs` | 2 tests added in G0.2 assumed `Ok(response_with_error_body)` but `RpcClient::call_tool` (`rpc.rs:153-165`) converts both JSON-RPC error envelopes AND MCP `result.isError=true` to `Err(RpcError)`. **Closed in G0.4** (commit `b55efb8f`): tests now `match Err(McpSandboxError::RpcError)` and assert error TYPE plus discriminator fragments. | n/a (closed) |
| DEBT-G0.5-01 | Legacy `offset_*` pagination tests | 3 tests in `chronos-sandbox/tests/query_filters.rs` (`test_query_events_offset_pagination`, `test_query_events_offset_beyond_total`, `test_query_events_limit_exact_pagination`) use `QueryFilter.offset > 0` which the wrapper at `tools.rs:670-674` explicitly rejects (pre-C5.2 pagination contract was replaced by opaque cursors in C5.2). **Ignored in G0.4** (commit `b55efb8f`, bodies preserved verbatim §0.4): `#[ignore = "G0.4: legacy pre-C5.2 offset pagination; migrate to cursor next_cursor (M1+)"]`. | `cargo test -p chronos-sandbox --test query_filters` (3 tests marked ignored) | **RETRACTED 2026-10-03 (R3.3): this debt was NOT real.** Its remediation was already in the tree and the entry was never updated. Verified, not assumed: (1) **The migration the entry asks for already landed**: commit `283a1445 `fix(cursor): migrate sandbox client from offset to cursor pagination (R0.2)`, and `test_query_events_limit_exact_pagination` carries its own note — "R0.2 (2026-09-22): migrated from `offset` to `next_cursor`. The observable property under test is preserved." (2) **They are NOT `#[ignore]`d**: `grep -c '#\[ignore' chronos-sandbox/tests/query_filters.rs` == 0. (3) **They PASS**: `cargo test -p chronos-sandbox --test query_filters` -> `9 passed; 0 failed; 0 ignored`, including `test_query_events_offset_pagination` and `test_query_events_offset_beyond_total ... ok`. (4) The entry also misplaces one of the three: `test_query_events_offset_beyond_total` lives in `query_edge_cases.rs:17`, not in `query_filters.rs`. So the two candidate follow-up cycles (`m1-offset-cursor-migration`, `m1-probe-inject-error-prefix`) are chasing work that is done.
| DEBT-M7-02-01 | `probe_inject` legacy error prefix | 4 tests in `chronos-sandbox/tests/probe_inject.rs` expect pre-C5.2 error prefix `probe_inject: capability: ebpf-uprobe`, but the m7-02 wrapper migration emits v2 `observe: probe still starting up`. **Pre-existing failure** (verified on `main @ c81ca08c` before G0.4 merge, NOT a regression). m7-02 debt. **not_run per directiva** (M1+ follow-up). | `cargo test -p chronos-sandbox --test probe_inject` (4 failed pre-G0.4, persists post-G0.4) | **RETRACTED 2026-10-03 (R3.3): this debt was NOT real.** Its stated cause does not hold and the tests are green where the gate runs. Verified, not assumed: (1) **The stated cause is wrong.** The local failure is not an error prefix at all — the tests never reach the capability assertion. They die in SETUP: `Failed to start MCP server: SpawnFailed("BinaryIdentity::from_path(chronos-mcp-not-built): read failed: No such file or directory (os error 2)")`. A missing build artefact, not a wrapper migration. (2) **With the binary built they pass**: `cargo build --bin chronos-mcp && cargo test -p chronos-sandbox --test probe_inject` -> `4 passed; 0 failed` (`..._without_root_... ok`, `..._invalid_symbol_... ok`, `..._nonexistent_session ... ok`, `..._before_pid_known_... ok`). (3) **And they already pass in CI**: run `37142399528` (`main @ 6dca303c`) logs all four as `... ok`. So "pre-existing failure that persists" was never true of the mandatory surface; it was true only of a checkout whose `chronos-mcp` had not been built. Worth noting the failure mode is the `54b6e2bc` fix WORKING: an unbuilt binary now names its own build step instead of masquerading as a missing system dependency.
| DEBT-G0-04 | UAT-G0-04 (privileged uprobe in real host) | UAT-G0-04 requires root + ptrace kernel in the running environment. Cannot be executed in this sandbox. **not_run per directiva** (M1+ follow-up or external CI). | n/a (env-locked) |
| DEBT-VAULT-CC-PERMANENT | CC#11/CC#18/CC#22 in `rec-c3.3-train-b` | 3 vault drift CCs (CC#11 status check, CC#18 verify-findings, CC#22 release-receipt fields) report drift on `rec-c3.3-train-b` only. Suspended per directive del operador (m9-89 cycle, 2026-09-19). **not_run per directiva** — `rec-c3.3-train-b` is in `apply_complete_pending_capture_session` and requires a política decision distinta (NOT a code fix). | `bash scripts/check_vault_drift.sh` (1 line CC#11, 4 lines CC#22, 1 line CC#18 from this cycle only) |
| DEBT-VAULT-CC-REC-C3 | CC#18 in 4 `rec-c3.*` cycles | CC#18 reports 4 missing `verify-findings.json` in `rec-c3-hexagonal-closure`, `rec-c3.3.4-native`, `rec-c3.5-residual-inversion`, and `rec-c3.3-train-b` (already counted in DEBT-VAULT-CC-PERMANENT). The 3 non-train-b are out-of-scope for vault drift (WIP/local). **not_run per directiva**. | same |

These do not gate REC-C4 (verified pre-existing on main). DEBT-C4-04 closed
in G0.3 (commit pending). DEBT-G0.4-01 + DEBT-G0.4-02 closed in G0.4
(merged `b44504ed`). DEBT-G0.5-01 ignored in G0.4 (bodies preserved §0.4). **2026-10-03: ambas filas retracted — ver R3.3.**

## Auditoria R3.4 (2026-10-04): la lista de deudas ABIERTAS estaba casi entera fantasma

Se verificaron las siete filas abiertas antes de trabajar ninguna. **Seis no son deuda real:**
sus criterios habian dejado de regir, que es la condicion que la regla pone para no contarlas.

| ID | Por que no es deuda |
|---|---|
| `DEBT-C4-01` | el ciclo ya esta `CLOSED`; lo que queda es residuo de directorio, y CC#11 esta verde |
| `DEBT-C4-02` | los 5 ciclos YA TIENEN `verify-findings.json`, y CC#18 lleva filtro de no-`CLOSED` desde R0 |
| `DEBT-C4-03` | los 4 campos del `release-receipt.md` estan presentes y el peel coincide de verdad |
| `DEBT-C4-04` | `identity.rs` tiene **cero** llamadas ejecutables a `set_var`; solo queda la frase del doc que explica el por que |
| `DEBT-G0.5-01` | migrado por R0.2 (`283a1445`), 0 `#[ignore]`, 9/9 verdes |
| `DEBT-M7-02-01` | nunca llego a la asercion: era un binario sin construir; 4/4 verdes y ya verdes en CI |

**Queda de verdad, y no es accionable desde el codigo:** `DEBT-G0-04` (UAT privilegiado, env-locked,
`not_run` por directiva) y `DEBT-VAULT-CC-PERMANENT` / `DEBT-VAULT-CC-REC-C3` (politica de
ciclos suspendidos, que ya no producen drift porque los checks estan filtrados).

**Por que esto importa mas que las seis filas.** Cada una era un ciclo completo de trabajo
programado sobre un problema ya resuelto, y cuatro de ellas describian causas que nunca fueron
las causas. La regla de no contar deuda cuyos criterios dejaron de regir no es burocracia: es lo
que impide que el backlog se llene de trabajo fantasma que ademas PARECE urgente.
DEBT-M7-02-01 + DEBT-G0-04 + DEBT-VAULT-CC-PERMANENT + DEBT-VAULT-CC-REC-C3
stay `not_run` per directiva — visible in STATE, not deleted.

Candidate follow-up cycles:
- `m-rec-c4.5-vault-debt`: close DEBT-C4-01..03 (now DEBT-VAULT-CC-PERMANENT + DEBT-VAULT-CC-REC-C3).
- ~~`m1-offset-cursor-migration`~~ **NO HACER FALTA — retracted 2026-10-03 (R3.3)**: DEBT-G0.5-01 ya estaba migrado por R0.2 (`283a1445`) y sus 3 tests corren en verde. (texto original: close DEBT-G0.5-01 (3 ignored `offset_*` tests).
- ~~`m1-probe-inject-error-prefix`~~ **NO HACER FALTA — retracted 2026-10-03 (R3.3)**: DEBT-M7-02-01 no tenía la causa que declaraba; los 4 tests pasan con el binario construido y ya pasaban en CI. (texto original: close DEBT-M7-02-01 (4 probe_inject tests with stale prefix).
- `m1-uat-g0-04-privileged`: close DEBT-G0-04 (requires privileged environment).

## DEBT-SCALE-MEM-01 (2026-10-04): la sesion se materializa al ARROLLAR, no al leer

> **SUPERADA POR LA SECCION DE CIERRE DE MAS ABAJO (R4.0).** Esta entrada y la siguiente se
> conservan como historia: las tres pasaron por la misma entrada y cada una corrijo a la anterior.
> Lo vigente es la seccion `DEBT-SCALE-MEM-01 — CERRADA (R4.0)` con la medicion antes/despues.
> Se conserva sobre todo la **primera** version, que atribuyo los 949 MB al read path y era falsa:
> es el ejemplo de por que la regla de verificar antes de actuar esta paid.

**Esta si es deuda real, y es la unica que R3.1/R3.2 anaden. Se corrigio a si misma el mismo
dia:** la primera version decia que "el camino de lectura carga la sesion en memoria — 949 MB
para responder un agregado". **Eso era falso**, y lo desmentio una medicion por fases hecha horas
despues en el mismo bloque. Se conserva el error original porque es justo el tipo de conclusion
que la regla de verificar antes de actuar existe para cazar, y porque tapaba el dato que si
importa.

| ID | Source | Description | Repro |
|---|---|---|---|
| DEBT-SCALE-MEM-01 | R3.2, al sondear por fases lo que R3.1 atribuyo | **El servidor materializa la sesion completa al arrancar: 944.608 KB residentes ANTES de responder un solo agregado.** Aislado: con la sesion de 1M valida el arranque esta en ~944 MB; con un fixture que el lector rechaza, en ~600 MB. Los **~344 MB de diferencia** son el coste de tener la sesion resident. | `TMPDIR=/var/home/rubentxu/chronos-scratch cargo run -p chronos-sandbox --example rss_probe` |

**LO QUE LA MEDICION REFUTO, que es la parte valuable del bloque.** El agregado de 1M cuesta
**+2.952 KB**, no 949 MB. Y cuatro agregados seguidos anaden **5.288 KB en total**, con la serie
aplanandose: **no hay fuga y el coste no escala con el numero de peticiones.** El diseno de R2.2 y
R2.4 —leer por paginas en vez de clonar la pagina entera— **funciona en memoria igual que en
tiempo**, y eso no estaba medido en ningun sitio: C3 (§7.3) acoto el tiempo y nadie habia acotado
la memoria del recorrido. El cociente de 2,14x que R3.1 midio entre harness y servidor es
**correcto y sigue en pie**; lo que estaba mal era leerlo como "el read path es el que pesa".

**CAUSA RAIZ LOCALIZADA (2026-10-04, sin abrir frente).** El arranque comprueba
`this.config.replay_on_open` (`crates/chronos-log/src/segmented.rs:492`) y, si esta activo, llama
a `replay_into_inner()` (`:983`), que construye un plan de replay y lo aplica con `apply_plan`
(`:988-996`) sobre un `InMemoryExecutionLog` **nuevo**, swapping el backend entero. Esa es la
materializacion: **la sesion completa queda resident antes de la primera pregunta.** No es un
desbordamiento del read path, es el diseño del arranque.

**Por que NO se arregla aqui, y es lo importante.** `apply_plan` construye un backend **nuevo** y
solo entonces lo publica, precisamente para que un fallo no deje un log meio reconstruido; los
comentarios de `:978-982` explican que las dos rutas lenient anteriores se eliminaron para que no
hubiera una puerta lateral que reconstruyera otra verdad. Un arranque lazy o paginado es un cambio
de **arquitectura de la recuperacion**, con su propio contrato de integridad, no un arreglo de
memoria. Ademas hay una razon de por que puede ser necesaria: `open()` deduce el `tail_state` de lo
que el replay reconstruyo (`:497-499`), o sea que la materializacion no es evidentemente un descuido sino la
fuente de la que se deriva el estado de cola. Characterizarla exige decidir si el estado de cola
puede derivarse de los metadatos del manifiesto sin cargar los registros — y esa es la pregunta
correcta, no "ponerlo lazy".

**CORRECCION DE ESA PREGUNTA, misma sesion (2026-10-04).** La pregunta anterior era la equivocada,
y el codigo lo desmenti en cinco lineas. `ReplayPlan` (`replay.rs:126-136`) lleva un campo
`entries: Vec<SegmentEntry>` con **todos** los registros decodificados, no solo su rango. Ese vector
sirve para dos cosas que no son la misma: validar la contiguedad y alimentar despues
`apply_replay_plan` (`replay.rs:322-336`), que los reinserta uno a uno en el backend. **Durante la
aplicacion coexisten el plan entero y el backend recien llenado**, luego el pico de arranque es la
sesion resident **mas** su copia — dos veces, no una. Eso explica por que ~1M de registros ocupan
~344 MB y no ~172 MB.

**La pregunta util, entonces, no es si el `tail_state` puede derivarse del manifiesto.** Ese dato ya
esta disponible en el propio plan (`replay.rs:135`, `reconstructed_tail`) y el manifiesto ya lo
persiste (`segmented.rs:526-527`); ademas `open()` solo lo usa para **verificar un sello**
(`segmented.rs:513-523`), y ninguna de esas tres cosas justifica retener los registros. **La pregunta
util es si la validacion puede dejar de retener entradas**: validar por rangos y aplicar con un cursor
o por tandas, de modo que el plan no tenga que vivir entero en memoria. Es un cambio local a
`replay.rs`, con su propia no-vacuidad que comprobar (contiguedad, huecos, solapes y el
`PayloadRangeMismatch` del control de gap retroactivo) — y es un bloque con valor propio.

## DEBT-SCALE-MEM-01 — CERRADA (R4.0, 2026-10-04)

**Medicion antes y despues, mismo host, misma sonda (`chronos-sandbox/examples/open_probe.rs`,
200.000 eventos, apertura sobre un directorio ya poblado):**

| | Coste de abrir | Por evento |
|---|---|---|
| Antes | 95.416 KB | **489 B** |
| Despues | 32.824 KB | **168 B** |

**Reduccion del 65,6%**, y replicada. A 1M de eventos son ~164 MB menos en el arranque, lo que
situa al servidor por debajo de la cifra de ~344 MB que R3.2 atribuyo a la materializacion.

**Lo que se hizo, en dos commits.** `6cd757d7` (fix): `build_and_apply_replay_listing` valida,
aplica y descarta un segmento cada vez, y devuelve el listado aplicado para que el llamante no
necesite un segundo recorrido — un `build_replay_plan` extra ahi habria reconstruido el plan que
la funcion existe para no construir, y habria pagado la memoria justo al intentar ahorrarla.
`replay_into_inner` y `populate_with_replay` usan esa forma. `9f46b06f` (test): el guard que
obliga a las dos rutas de validacion a coincidir.

**Lo que NO cambio, y es la condicion de aceptacion.** La disciplina de backend nuevo e
intercambio solo en exito se conserva intacta: un fallo sigue sin poder publicar un log medio
reconstruido, que es lo que las dos rutas lenient anteriores rompian. `build_replay_plan` y
`apply_replay_plan` siguen siendo API publica para "validar una vez, aplicar despues"; solo
salen de la ruta de arranque, que es la que paga el pico.

**EL GUARD ENCONTRO UN BUG EL DIA QUE SE ESCRIBIO, y era mio.** El primer
`PayloadRangeMismatch` de la ruta nueva reportaba `header_start: header.start_seq` donde la
canonica usa `cursor` — el punto exacto donde la secuencia se desvia. El mensaje decia que el
segmento "cubre 3..=5 pero su cabecera declara 0..=9" cuando el desacuerdo real estaba en el 10.
**Las dos rutas rechazaban el log, luego ninguna suite existente lo noto:** solo el guard, que
compara las dos respuestas, vio que no decian lo mismo. Es el segundo guard de este bloque que
paga su coste el dia que se escribe.

**Lo que queda, y es menor que la deuda original.** El arranque sigue materializando la sesion
(~168 B por evento), porque el backend en memoria es el modelo de `InMemoryExecutionLog`. Bajar
de ahi es un cambio de arquitectura de la recuperacion, no de memoria, y no se abre aqui. **No
se fija ningun threshold:** `SCALE_BUDGETS` §0 prohibe una cifra que nadie pueda re-medir, y
168 B/evento en este host es tan poco un contrato como lo eran 489.


**Por que la deuda sigue abierta siendo que el read path esta bien.** El coste de arranque escala
con el tamano de la sesion y ningun contrato lo acota: abrir un servidor contra un log grande
consume su memoria antes de que nadie pregunte nada, y un operador que dimensiona una caja
mirando solo el tiempo de un agregado (11,3 s) no ve ese numero. Falta caracterizar si esa
resident es **necesaria** o una consecuencia de la replay, y si hay un techo natural. **No se
inventa un threshold:** `SCALE_BUDGETS` §0 prohibe fijar una cifra que nadie pueda re-medir.

**Lo que R3.2 deja para quien la retome:** la medicion por proceso era **inexpresable** antes
(`McpProcess.child` era privado), y la sonda por fases quedo en
`chronos-sandbox/examples/rss_probe.rs`. Ademas se registran **cuatro intentos fallidos** de esa
sonda, porque son la clase de resultado que se repite si no se anota: un nombre de herramienta
pasado como metodo JSON-RPC (-32601); `params` como objeto de argumentos en vez de sobre
`{name, arguments}` —que responde `total_events: 0` en 0,0 s **sin ejecutar nada** y se lee como
un agregado rapido sobre un log vacio—; y un `TraceEvent` inventado campo a campo que el lector
rechazo con `payload tag "trace_event" could not be decoded`. Los tres se veian como fallos de
producto. El probe ahora **aserta `total_events == 1.000.000`**, que es lo que convierte "no
ocurrio" de un resultado silencioso en un fallo ruidoso.

**Ademas, y esto es lo que mas importa para leer cualquier medicion futura: las cuatro filas de la
tabla de `SCALE_BUDGETS` §9.5 describen el RIG, no el producto.** El sandbox liquida al servidor
con `kill -9` externo, asi que el nieto nunca entra en la contabilidad de `/usr/bin/time -v`.
Cualquier dimensionado hecho con esas cifras estaba sobrestimando o subestimando sin saber cual.

---

## DEBT-CI-TIMING-01 — `uat_c2_01_probe_drain_is_not_an_authority` falla por carga del runner, no por el producto

**Estado:** `OPEN` · **Abierta en:** R6.2 (2026-10-04) · **Owner:** agente principal · **Origen:** job `Test`
del run `37183456326` (`8b1e2e5a`), en `chronos-sandbox/tests/rec_c2_2_uat_c2.rs:282`.

**Lo que se ve.** El test drena el probe, obtiene `first_event_after_ms=4` y `count=1`, y despues
espera a que el `ExecutionLog` del fixture produzca registros nuevos dentro de
`deadline_ms=300000`. No llega ninguno y el test falla con *"the fixture's ExecutionLog did not
produce any new records within 300000ms after the first drain"*. Los otros cinco tests del mismo
binario pasan, incluido `uat_c2_03_durable_evidence_exceeds_the_ring`.

**Evidencia de que es flake y no regresion, y no una suposicion.** El **mismo binario, en el mismo
runner y con el mismo codigo**, pasa a las `07:14:17Z` (run `37179456792`, sobre `c7796d35`) y falla
a las `07:26:54Z` (run `37183456326`, sobre `8b1e2e5a`): doce minutos de diferencia. Entre los dos
SHAs no hay cambio de codigo en esa ruta.

**Por que ocurre.** La propiedad que el test verifica —*el drain no es una autoridad: lo que llega
despues viene del log, no del probe*— se comprueba esperando a que el probe uprobe genere syscalls
suficientes. Eso convierte la carga de la maquina en parte del criterio. En un runner compartido, si
el ritmo de syscalls cae por debajo del umbral durante la ventana, el fixture no produce registros
nuevos y el test falla aunque el producto este bien. El propio mensaje de diagnostico del test lo
reconoce: *"Either the probe stopped, the syscall rate is too low, or ..."*.

**Por que es deuda y no solo una molestia.** Un test cuyo veredicto depende de la carga del runner
no puede ser un gate: hace que un CI rojo signifique dos cosas a la vez, y quien llegue despues
gastara el tiempo mirando el producto cuando el defecto esta en el criterio. Un gate que miente
cuando no debe es peor que no tener gate.

**Lo que NO se hace todavia, y por que.** No se marca `#[ignore]`: el test cubre una propiedad real,
y un `ignore` sin mas razon que "a veces falla" solo reduce la senal. La reparacion que toca es
**separar la propiedad del ruido**: que el fixture produzca registros nuevos de forma determinista
—por ejemplo, disparando la senal que el probe observa en vez de esperar a que el proceso genere
syscalls solo— y dejar el drain como la variable que se mide. Eso exige leer el fixture entero, y
se deja para quien lo retome con el contexto delante.

**Se relanzo el job fallido** (`gh run rerun 37183456326 --failed`) para confirmar que el veredicto
es no determinista. Si vuelve a pasar, la evidencia queda en los dos runs; si vuelve a fallar, esta
entrada pasa de flake a defecto reproducible y hay que tratarlo como tal.

**Diagnostico de raiz, medido leyendo el fixture (no deducido).** La cadena es esta:

1. `start_probe_with_ring` lanza el fixture `test_busyloop` y duerme 2 s. El fixture **corre ~3 s
   y luego muere** (`rec_c2_2_uat_c2.rs:35-42`).
2. El primer `probe_drain` con `limit: 1` devuelve el primer evento (en CI, a los 4 ms).
3. `wait_for_log_advance` hace polling **cada 100 ms** con `limit: 1000` y compara el
   `evidence_cursor` con el del primer drain. Devuelve `Some` en cuanto el cursor **cambia**, y
   `None` al agotar el deadline.
4. El cursor solo cambia si el `ExecutionLog` ha producido **algun** registro nuevo. Un solo evento
   nuevo basta, porque `limit: 1000` devuelve hasta el ultimo examinado.

O sea que el test exige que **el busyloop siga vivo durante la ventana de espera**. En CI fallo
porque, con la maquina compartida, el arranque mas el primer drain mas el parseo se comieron la
ventana de 3 s y el fixture ya no generaba nada. Los 300 s de espera no pueden arreglar un fixture
que ha terminado: el polling no crea evidencia, solo la observa.

**Y hay un antecedente en el propio fichero que desaconseja subir el deadline.** El bloque de
comentario de `uat_c2_01_probe_drain_is_not_an_authority` registra **cuatro** intentos anteriores de
ampliar el plazo, todos con la misma firma: el primer evento llega *exactamente* en el deadline
(R8 10s -> 10041ms, R8.1 30s -> 30006ms, R9.11 60s -> 60003ms, R9.12 300s -> 300028ms con
`total_buffered=0`). El propio autor concluyo que *"the wait is not latency"* y que *"el test mide
una propiedad que este entorno no puede observar"*, y por eso el test ya se salta bajo tarpaulin.
**El patron "subrir el numero" esta agotado y hay cuatro casos que lo demuestran**, asi que un
quinto intento del mismo tipo no es una solucion sino una repeticion.

**La reparacion que si toca** es de fixture, no de plazo: que el proceso que alimenta el log este
vivo y generando por demanda mientras el polling corre. Las dos vias que se ven son (a) relanzar el
busyloop —o un segundo proceso generador— desde dentro de `wait_for_log_advance` cuando el cursor
no avanza, de modo que la senal la produce el test y no la suerte; o (b) separar la propiedad que se
quiere medir —*el drain no crea evidencia: el cursor solo avanza si el log crecio*— del ruido
temporal, comprobandola con un log que crece de forma controlada. La (a) es la menor y conserva el
test tal cual; la (b) es la mas limpia y toca mas superficie.

**Por que no se ejecuta aqui.** Es un cambio de fixture en un test de captura uprobe, en plena
preparacion de un release, y su verificacion depende de un runner saturado que es justo la condicion
en la que se manifesta el defecto. Hacerlo sin ese entorno seria cambiar un test sin poder observar
el fallo ni la correccion, que es como se llega a un test que pasa por casualidad. Se deja con el
diagnostico completo para que quien lo retome sepa exactamente que mirar.

**Fecha / condicion de cierre:** cerrar cuando el fixture deje de depender del ritmo de syscalls del
runner, o cuando exista un modo determinista de producir los registros nuevos. Revisar si en tres
runs seguidos sobre el mismo codigo aparece mas de un fallo.

---

## R6.3 — la hipotesis del fixture queda refutada por medicion

**La premisa de esta entrada era la vida util del fixture, y es falsa.** Se comprobo, no se supuso.

Para poder desacoplar la vida del fixture del deadline de observacion, `test_busyloop` recibio su
duracion por `argv[1]` (con el default 3 intacto) y el cliente sandbox recibio una forma de
enviarla. Con eso se pudo correr el mismo test con vidas de fixture de 25, 50, 100, 200, 220, 240,
260, 280, 300, 301, 302, 303, 304 y 305 segundos, manteniendo la ventana de observacion en 20 s:

| vida del fixture | resultado | primer evento |
|---|---|---|
| 25, 50, 100, 200 | pasan | 14-28 ms |
| 220 | **falla** | — (`count=0`) |
| 240, 260, 280, 300 | pasan | 18-413 ms |
| 301, 302, 303, 304 | pasan | 14-56 ms |
| 305 | pasa (2 veces) | 16-17 ms |
| 305 | **falla (4 veces)** | — (`count=0`) |

No hay umbral monótono: 220 falla y 240 pasa. Y 305 fallo cuatro veces y paso dos, en la misma
maquina y con el mismo binario. **La duracion del fixture no es el disparador**, y el cambio se
revierte en lugar de publicarse: no arregla nada y solo ensancha la ventana en la que un hijo
filtrado quema CPU.

**Lo que se escribio despues, y resulto ser un error propio.** Se afirmo que `uat_c2_01` abandona
sus sesiones al hacer skip y que esa fuga era auto-amplificante y explicaba la carga. No se
sostiene. Se forzo la ruta de skip de forma determinista (condicionando el predicado, con la
sesion real y valida) y se midieron procesos `test_busyloop` vivos despues de la corrida:

| rama de skip | huerfanos |
|---|---|
| con `stop_all` | 0 |
| con el `client.shutdown()` de antes (defecto presente) | 0 |

El defecto presente no produce huerfano alguno: la medicion no distingue los dos casos. El motivo
es el fixture: vive 3 s y el skip ocurre a los 300 s, asi que cuando se abandona la sesion el hijo
ya termino por su cuenta. No hay nada que filtrar.

Los huerfanos de 305 s que se "encontraron" con `ps` (PID 3380252, 413 s de vida, 27% de CPU) eran
de `test_busyloop 305`: una duracion que la propia R6.3 introdujo para el experimento. El efecto
auto-amplificante se ve, pero lo produce el cambio que se iba a probar, no el codigo de origen. Por
eso se revierte: no solo no arregla nada, ademas fabrica el problema que despues se diagnosticaba
como preexistente.

Queda `stop_all(client, sessions)` como mejora de higiene, y solo como eso: abandonar una sesion
con un proceso todavia trazado es incorrecto aunque hoy no se note, y tomar el cliente por valor
hace que el teardown no pueda olvidarse. No se presenta como reparacion de nada.

**La fuga de las rutas de `panic!` si es real y si esta medida.** Al forzar un error de RPC el test
cayo por `panic!` en lugar de por skip, y quedaron **2 procesos `test_busyloop` vivos** despues de
la corrida: ahi el hijo si estaba en pie cuando el test termino. Corregirlo exige que `Drop` pueda
hacer `await` sobre `probe_stop`, que no puede, asi que requiere reestructurar el test o un guard
de propiedad de sesion. No se mezcla en este commit.

**Corroboracion independiente de que el veredicto es no determinista, sin tocar el fixture:**
`Coverage` fue **rojo** en `f8fe1416` y **verde** en `7babbde3`, que difieren solo en un commit de
documentacion. Mismo codigo de producto, mismo runner, veredicto distinto. Ademas el job `Test` de
`7babbde3` quedo verde con `uat_c2_01` tardando ~315 s: es decir, **agotando los 300 s y saliendo
por SKIP**, no verificando el contrato. Un verde por esta via no es un verde.

**Lo que queda abierto, y es lo que de verdad importa.** Cuando falla, `probe_start` devuelve una
sesion `running` que en 300 s no captura **ni un evento**: `total_buffered: 0`, `completeness` de
seq 0 a 0. El skip lo etiqueta *"environment verdict, not a contract verdict"*, y ese juicio no
esta ganado: el test sabe distinguir "el host va lento" de "el producto no capturo nada", pero no
ha establecido cual de los dos esta mirando. Queda como pregunta de producto:
**¿por que `probe_start` devuelve a veces una sesion viva que no captura?**

No se ha podido reproducir el fallo de forma deliberada: con 32 spinners y un huerfano vivo de
164 s, el test paso en 14 ms. Y no hay muestreo del estado de traza del hijo *durante* un fallo,
porque no se ha logrado uno. Que un huerfano aparezca en estado `R` no prueba que el attach
fallara: cuando su tracer muere, el hijo deja de estar trazado y sigue corriendo. Esa inferencia
se hizo y se retiro.

**Fecha / condicion de cierre:** esta entrada ya no se cierra por "que el fixture no dependa del
ritmo de syscalls", porque el fixture no es la causa. Se cierra cuando se responda por que
`probe_start` devuelve a veces una sesion viva sin captura, o cuando esa ruta pase a fallar en
lugar de a hacer skip.

---

## DEBT-PROBE-LIVENESS-01 — `probe_drain` reporta `running` como literal fijo, y un worker de captura muerto es indistinguible de uno sano

**Estado:** `OPEN` · **Abierta en:** R6.3 (2026-10-04) · **Owner:** agente principal · **Origen:** investigacion
encargada mientras se diagnosticaba `DEBT-CI-TIMING-01` · **Severidad:** alta, es la causa probablemente
de raiz del intermittencia de `uat_c2_01`.

**Lo que se ve desde fuera.** `probe_start` devuelve un session id, `probe_drain` responde
`"status": "running"` y un `hint` que dice *"Probe is still running"*, y el `ExecutionLog` no recibe
**ni un registro** durante 300 s. Con cero registros, `completeness` sale
`{from_seq: 0, to_seq_exclusive: 0, status: "complete", scope: "examined_range"}` y el cursor no se
mueve. Nadie, desde la API, puede distinguir ese estado de uno sano.

**El `status` no es un estado: es un literal.** En `crates/chronos-mcp/src/server.rs:2794` el
`"running"` se escribe como constante, sin consultar al worker. El `AtomicBool` `running` que si
existe en el backend del probe no lo lee nadie fuera de `start_probe` y `stop_probe`. Consecuencia
directa: **la comprobacion de vivacidad del propio test es circular** —
`rec_c2_2_uat_c2.rs:219` lee ese mismo literal para decidir si la sesion esta viva, asi que
`verdict_is_unobservable(session_live, ...)` recibe un `true` que el servidor garantiza. Por eso su
skip califica de *"environment verdict"* algo que no ha midido.

**El estado vacio es ambiguo, no distinguible.** En `canonical_drain.rs:288-296` el par
`from_seq`/`to_seq_exclusive` es "lo examined", y la rama `Complete` se elige **tambien con cero
registros examinados** (`:269-271` fija `exhausted` y `:282` deja el cursor igual). El resultado es
byte a byte identico para los cuatro casos que importan: *el productor nunca arranco*, *murio*,
*se aparco* y *de verdad aun no hay actividad*. `scope` es un `&'static str` constante
(`events_log_read.rs:109-118`) y no aporta informacion de vivacidad.

**Caminos verificados en codigo que producen una sesion viva y vacia:**

1. **El tracee queda detenido y el bucle se aparca en un `waitpid` sin limite.**
   `ptrace_tracer.rs:482` hace `waitpid(-1, __WALL)` bloqueante y, con `follow_children: true`
   (`probe_backend.rs:509`), es el **unico** camino de despertar: la rama de sondeo de
   `ptrace_tracer.rs:498-544` es inalcanzable. Si el resume de `probe_backend.rs:985-992` falla, el
   error se emite solo con `debug!` (`:991`, apagado en release) y el bucle vuelve a `wait_event` sin
   volver nunca. Sesion `running`, hilo vivo, log congelado. **Es el unico camino encontrado que da
   exactamente 1 evento (el primer `SyscallEnter` post-exec) y despues silencio permanente**, que es
   la firma observada en el run `37186894696`.
2. **El hilo muere y la sesion sigue diciendo `running`.** Si `launch()` falla,
   `probe_backend.rs:807-810` hace `tracing::error!` + `return`, y —a diferencia del camino de
   attach (`probe_backend.rs:1038`)— **no limpia `running`**. Igual con `Ok(None)`/`Err` de
   `wait_event` (`:903-912`): se mata el tracee, el hilo termina con un `info!` y el registro de
   sesion no se actualiza nunca. `ptrace_tracer.rs:694-697` devuelve `Ok(None)` para wait status sin
   manejar.
3. **Apendices rechazados no dejan rastro.** `probe_backend.rs:956-968` y `:964`: un fallo de
   `accept_and_publish` no consume seq, no registra hueco y solo es un `warn!`. `probe_drain` no lo
   ve. Es candidato a contribuyente, pero no explica 300 s de silencio.
4. **Division de identidad del log — descartada.** `probe.rs:266` registra el mismo handle que
   escribe el productor; `probe.rs:676` lee ese clon.

**Lo que NO se ha verificado, y no sepresenta como establecida:** que `PTRACE_SYSCALL`
(`ptrace_tracer.rs:722`) falle efectivamente bajo carga, ni que el punto de rotura sea el SIGTRAP
del exec. No se ha seguido el comportamiento del kernel de syscalls/ptrace. La subinvestigacion no
verifico comportamiento de kernel, solo rutas de codigo.

**Por que es deuda y no una molestia.** Sin vivacidad observable, *ningun* consumidor de la API
puede decir si una captura funciono. `uat_c2_01` no puede distinguir "el host va lento" de "el
producto no capturo nada" porque el unico dato que podria hacerlo es constante. Eso convierte su
skip en un **verde que no verifica nada**, que es peor que un rojo: un rojo al menos obliga a mirar.

**La reparacion que toca (no se aplica aqui).** Un handshake acotado y observable en `start_probe`, o
un hecho de vivacidad del worker expuesto a `probe_drain`, de modo que el caller pueda distinguir
productor-muerto de log-aun-vacio. Mientras `status` sea un literal, cualquier consumidor —test
incluido— seguira decidiendo con un dato que el servidor fabricate. Y en `probe_backend.rs:991` el
`debug!` de un fallo de resume deberia ser al menos un `warn!`: un fallo que deja el bucle aparcado
para siempre no puede ser invisible en release.

**Fecha / condicion de cierre:** cerrar cuando `probe_drain` exponga la vivacidad real del worker, o
cuando el bucle de `wait_event` tenga un limite y un error observable en vez de un
`waitpid` infinito. Revisar si los caminos 1 y 2 se pueden confirmar con una sesion viva y log vacio
reproducible.

---

## R6.5 - causa raiz encontrada: dos hilos tracer se robaban el tracee del otro

**Estado:** la causa de producto esta **encontrada y corregida**; la honestidad de la API sigue abierta.

**Como se cerro la busqueda.** R6.4 dejo de disimular el fallo, y la CI lo mostro al instante sobre
`684b3d16`: `uat_c2_01` con `first_event_after_ms=300034`, `count=0`, `total_buffered: 0`,
`completeness {from_seq 0, to_seq_exclusive 0}`, mientras su captura de control, en el mismo host,
mismo fixture y mismo binario, veia un evento a los **5 ms**. El host era capaz. Esa sesion estaba
muda. Es la segunda reproduccion independiente de la misma firma (la primera, la del verificador).

**La causa raiz.** `PtraceTracer::wait_event` hacia `waitpid(-1, __WALL)`, que reap el estado de
cualquier hijo **del proceso**, no del hilo que llama. Este servidor lanza un hilo tracer por sesion
de captura, y las sesiones concurrentes son una **funcionalidad del producto**:
`chronos-sandbox/tests/multi_session.rs` arranca dos probes y ejercita operaciones entre sesiones.
Dos hilos tracer del mismo proceso compiten por los mismos hijos; el `waitpid` que despierte primero
se consume el estado, y el perdedor no vuelve a observar a su tracee. Su sesion sigue viva, se
declara `running` y su ExecutionLog queda vacio para siempre.

Eso explica de una sola vez las tres cosas que el sintoma hacia inexplicables: la sensibilidad a la
carga (mas planificacion, mas carreras), la firma de 1 registro frente a 0 segun quien gane el
drain, y que el hermano diagnostico vea un evento a los 47 ms mientras la sesion del test no ve nada.

**La premisa que era falsa.** El comentario de `crates/chronos-native/src/test_support.rs` justificaba
`waitpid(-1)` diciendo que en produccion `ChronosServer` lleva una sola `active_session`, luego
"exactamente un tracer sigue exactamente un arbol de procesos". Eso no se sostiene: las sesiones
concurrentes son una funcionalidad probada del producto. El fichero describe una restriccion real de
los tests que corren en paralelo dentro de un binario de test, y su justificacion de produccion era
erronea. Esa confusion es probablemente lo que mantuvo el defecto vivo tanto tiempo: el archivo que
documentaba el peligro se contradecía con su propia premisa.

**El arreglo.** `__WNOTHREAD` junto a `__WALL` en el `waitpid` del tracer: un hilo solo reap los
hijos que el mismo empezo, que es la respuesta del kernel y la que usan strace y gdb. Cada hilo de
captura hace su propio fork con `TRACEME`, asi que cada tracee suyo es hijo suyo y no se pierde nada;
`__WALL` se conserva porque es lo que entrega las paradas por syscall y las de clon/fork. Ademas el
fallo de resume en `probe_backend.rs` pasa de `debug!` a `warn!`: un resume fallido deja el tracee
detenido y el bucle aparcado en un `waitpid` bloqueante que nadie despierta, y un defecto que wedgea
una captura en silencio no debe compilarse fuera de release.

**Verificado.** `chronos-native --lib` 114/114. `multi_session` 7/7 (140,77 s) y el test de probes
concurrentes aislado 18,61 s: son exactamente las rutas de sesion concurrente que el cambio tocaba.
`program_scenarios` 11/11 (207,18 s), que es la ruta de clon/`follow_children`.
`m1_04_live_probe_execution_log` 2/2. `rec_c2_2_uat_c2` 6/6 en 76,32 s, con paso real y no un skip.

**El limite de esa verificacion, dicho sin adornos.** Se puede probar que la carrera es real, que su
premisa de justificacion era falsa y que el flag no rompe ninguna ruta cubierta. **No** se ha podido
reproducir la carrera bajo demanda, porque depende del planificador, asi que no hay un antes/despues
medido sobre ella. Que la carrera ocurra en la practica esta respaldado por dos reproducciones
independientes con la misma firma, no por una demostracion controlada. La CI de `684b3d16` es el
arbitro final de si el arreglo la cierra.

**Lo que sigue abierto.** El `status` de `probe_drain` sigue siendo el literal `"running"` de
`server.rs:2794`. R6.5 elimina una causa de sesion muda, pero si otra apareciera, la API seguiria
sin poder distinguirla de una sesion sana, y el gate tendria que volver a recurrir a una captura de
control para enterarse. Exponer la vivacidad real del worker sigue siendo el cierre de
`DEBT-PROBE-LIVENESS-01`.

---

## R6.4 - el lado de test deja de mentir; el lado de producto sigue abierto

**Lo cerrado en R6.4 (mitigacion, no causa).** `uat_c2_01` ya no decide su veredicto con el literal
que el producto fabrica. Ahora corre `control_capture_sees_an_event()`: una segunda captura
independiente, misma fixture, mismo host, sesion propia y deadline corto de 30 s, que responde a la
unica pregunta relevante, *puede este host observar una captura ahora mismo*. La excusa solo se gana
si el control **tambien** vio nada. Si el control ve un evento, el host es capaz, el log vacio es un
fallo de producto y el test entra en panic nombrando `server.rs:2794` como origen del literal.

El diagnostico `cih_g_uat_c2_01_diagnostic_first_event_timing` deja de adjudicar: medir es su trabajo,
y un test que entra en panic sobre la respuesta que existe para recolectar destruye la senal. Su
llamada a `verdict_is_unobservable` se **elimina** en lugar de adaptarse, porque del cable no sale
ningun veredicto honesto.

De paso se eliminaron dos guards que no podian fallar. El assert `elapsed <
UAT_C2_01_FIRST_EVENT_DEADLINE` del diagnostico era cierto por construccion (el poll vuelve en cuanto ve
un registro; si vence el deadline devuelve `count == 0`, que es la rama anterior). Se sustituyo por un
presupuesto de latencia de 60 s, que es lo que la medicion mide y si tiene dientes. Los guards de
exhaustividad se renombraron a `unobservable_verdict_requires_a_control_that_also_saw_nothing` y se
verifico su no-vacuidad por mutacion: cambiando el helper a la semantica vieja, los dos pasan a
FAILED con `verdict_is_unobservable(false, true, false) must be true`.

**El verificador independiente que dio FAIL a v0.2.0.** Fue lo que hizo que esto se detectara. Su
verificacion dio 7 hallazgos, y el que abrio el resto fue que `probe_start` devuelve una sesion
`running` que en 300 s no captura nada, y el test lo reporta `ok`. Lo reprodujo con
`first_event_after_ms=300047, count=0`, mientras su propio diagnostico hermano veia un evento a los
47 ms en el mismo host. Ademas confirmo que el `status` es un literal, que el check de vivacidad del
test es circular, y que en CI el test tardo 338 s frente a 15,6 s de sus hermanos, que es la firma de
agotar el deadline.

**El hallazgo 5 del verificador, tambien abierto.** El `pipelinek` 12/12 es **solo TIER 1**: su
propio log dice `TIER 2 NO EJECUTADO: CHRONOS_FULL_GATE != 1` y que la matriz de integracion (que es
la que posee `rec_c2_2_uat_c2`) esta omitida por diseno y no es un PASS de integracion. El "12/12" que
se cerro antes sobrevaloraba el gate.

**Lo que sigue abierto, sin cambios.** La causa de producto: por que `probe_start` devuelve a veces una
sesion viva sin captura. R6.4 no la arregla, solo hace que el gate deje de ocultarla. Siguen
pendientes (a) exponer la vivacidad real del worker en `probe_drain`, y (b) acotar el bucle de
`wait_event` para que un tracee detenido no se aparque en un `waitpid` infinito, con el error de
resume en `warn!` y no en `debug!`.
