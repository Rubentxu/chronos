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
