// .pipeline.kts — chronos (Rust workspace, 13 crates)
// Ejecuta verificación real del workspace: cargo check + cargo build +
// lint + la suite de tests del workspace + contratos de arquitectura.
//
// PARIDAD CON LA CI REMOTA (2026-10-01)
//   Este gate es, por contrato de AGENTS.md §CI Local, la ÚNICA autoridad
//   para declarar el repo verificado. Antes de esta revisión ejecutaba un
//   único binario de test (`execution_log_read_e2e`, 2 tests E2E del
//   read-path) y ningún lint. Medido sobre b5edae57 el 2026-10-01, la
//   superficie no cubierta era:
//     cargo test --workspace --lib      1421 tests  → 0 ejecutados
//     binarios tests/ del workspace     65         → 1 ejecutado (2 tests)
//     clippy -D warnings                ausente    → 0 lint
//     contratos de arquitectura         ausente    → 0 comprobaciones
//   O sea: el gate certificaba 2 tests y no detectaba ni un fallo en los
//   1419 restantes, ni una regresión de lint, ni un hueco arquitectónico.
//   Un "SUCCESS" probaba compilación, no comportamiento.
//
//   Los comandos de los stages nuevos son copia literal de
//   .github/workflows/ci.yml y .github/workflows/architecture-contracts.yml.
//   No se inventa política: si la CI remota corre algo, este gate lo corre.
//
//   Coste medido (2026-10-01, run 863f0be3, con el host corriendo además
//   sddk-framework, assuranceGraph y agent-secretless sobre el mismo
//   cargo-targets):
//     TIER 1 (este script)  5.4 min (323s), 9 stages, 18 steps, 0 fallos
//     TIER 2 (opt-in)       >47 min sin terminar la matriz de 65 binarios
//   Incluir TIER 2 de forma incondicional convertía el gate en algo que la
//   gente se salta, y un gate que se salta es peor que un gate rápido y
//   honesto. Por eso TIER 2 es opt-in con CHRONOS_FULL_GATE=1; el run por
//   defecto es TIER 1.
//
//   TIER 1 (por defecto, rápido, todo lo que antes no se ejecutaba):
//     lint-workspace            clippy --workspace --all-targets -D warnings
//     test-workspace-lib        1421 tests, la superficie unitaria completa
//     architecture-contracts    --strict-no-gaps con base=HEAD~1
//     test-sandbox-read-path    E2E sobre el wire real (pre-existente)
//     feature-matrix-truth      --all-targets --all-features (añadido 2026-10-03)
//     ci-toolchain-parity       lint con la toolchain de la CI (añadido 2026-10-03)
//     production-bin-build      cargo build --bin chronos-mcp (añadido 2026-10-03)
//
//   NOTA 2026-10-03 (feature-matrix-truth). Los recuentos de arriba
//   ("9 stages, 18 steps", 5.4 min / 323s) son la medición del run
//   863f0be3 del 2026-10-01 y NO se reescriben: son un hecho. Este stage
//   eleva el gate a 10 stages y 19 steps. El gate completo con este stage
//   no se ha vuelto a medir todavía; el coste del propio check sí está medido
//   en 35.2s incremental sobre host compartido.
//
//   NOTA 2026-10-03 (ci-toolchain-parity). Segundo guard añadido el mismo día,
//   por el mismo modo de fallo: un gate local en verde que la CI remota
//   rechaza. El de feature-matrix era "código que nunca se compiló"; este es
//   "código que se compiló con otro compilador". La CI usa el canal `stable`
//   flotante y el host estaba clavado en 1.98.1 mientras la CI ya iba en
//   1.99.0, así que el lint `clippy::double_must_use` —nuevo en 1.99— no
//   existed para ningún gate local. Con este stage el gate local son 11
//   stages y 20 steps. Coste SIN MEDIR: depende de si el host tiene la
//   toolchain de la CI (en cuyo caso re-copia el lint) o no (en cuyo caso
//   para y lo dice).
//
//   NOTA 2026-10-03 (production-bin-build). TERCER guard, tercer incidente del
//   mismo tipo: el commit b8ba75ae pasó fmt, clippy --all-targets y 107/107
//   tests, y el Sandbox Debt Sentinel lo rechazó con E0599 porque el BINARIO DE
//   PRODUCCIÓN no compilaba. `mod tests` en
//   `crates/chronos-mcp/src/server.rs` no está gated con `#[cfg(test)]` por
//   decisión pre-existente (server.rs:4318), y rustc elimina las fns con
//   `#[test]`/`#[tokio::test]` cuando no hay harness pero SÍ compila una `fn`
//   desnuda. Tres helpers nuevos llamaban a una API gated por dev-dependency
//   (`create_for_tests`), que en `cargo build` no existe. Con este stage el
//   gate local son 12 stages y 21 steps y corre el comando literal de
//   `sandbox-debt-sentinel.yml:45`.
//
//   TIER 2 (CHRONOS_FULL_GATE=1, replica integral de ci.yml):
//     test-workspace-integration  --workspace --tests --exclude chronos-e2e
//
//   Antes de este cambio: 5 stages, 9 steps, 2 tests, ningún lint.
//   Después:              9 stages, 18 steps, 1421 tests + lint + contratos.
//
//   TIER 2 nunca se salta en silencio: si no se pide, la stage lo dice con
//   `echo` explícito, en el mismo estilo con el que el sentinel declara sus
//   skips en vez de omitirlos sin avisar.
//
// IMPORTANTE — restricciones del motor pipelinek v0.39.0:
//   1. NO resuelve el cwd del script: las rutas dentro de sh(...) son LITERALES.
//      Hay que hardcodear la ruta absoluta del checkout en REPO.
//   2. sh("cmd 2>&1 | tail -N") retorna exit code del pipe (= 0 de tail).
//      Workaround correcto (R6.2): redirigir a fichero y usar el codigo de
//      salida del comando, no el del pipe. Ver el bloque repetido mas abajo.
//      Lo que este fichero hacia antes, `${PIPESTATUS[0]}`, NO funciona aqui:
//      pipelinek escribe script.sh SIN shebang, asi que se ejecuta con /bin/sh
//      (dash en Fedora) y PIPESTATUS es una variable de bash. El shell recibia
//      la cadena literal y `test` fallaba con exit 1. Medido el 2026-10-04:
//      el gate llevaba tiempo sin compilar y por eso nadie llego a verlo.
//   3. La primera stage debe ser discover-repo (contrato AGENTS.md §CI Local).
//   4. OJO con los raw strings (sh("""...""")). Ahi `\$` NO escapa el dollar:
//      Kotlin los procesa igual, asi que `$CI_VER` se resuelve contra Kotlin
//      ("Unresolved reference") y hay que escribir `${'$'}CI_VER`. El mismo
//      descuido en un string normal NO ocurre: ahi `\$` si escapa.
//   5. En shell, la ASIGNACION no lleva dollar (`GATE_LOG=...`, `GATE_RC=$?`);
//      solo las LECTURAS lo llevan (`> "$GATE_LOG"`, `test $GATE_RC -eq 0`).
//      Escribir `$GATE_LOG=...` hace que sh intente ejecutar `$GATE_LOG`.

val REPO = "/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos"

pipeline {
    stages {
        stage("discover-repo") {
            sh("ls -la $REPO/Cargo.toml $REPO/AGENTS.md | head -3")
            sh("head -3 $REPO/Cargo.toml")
        }

        stage("workspace-check") {
            // cargo check --workspace: acota tiempo evitando compilar deps pesadas.
            sh("cd $REPO && GATE_LOG=\"/tmp/chronos-gate-\$\$.log\" && cargo check --workspace --message-format=short > \"\$GATE_LOG\" 2>&1; GATE_RC=$?; tail -20 \"\$GATE_LOG\"; test \$GATE_RC -eq 0")
        }

        // ---------------------------------------------------------------
        // feature-matrix-truth  (REC-C0.2 — Feature-matrix truth)
        //
        // Por qué este stage existe: `cargo check --workspace` compila la
        // combinación HABITUAL de features (default). Toda feature que no
        // sea default queda sin verificar: es código que el gate certifica
        // como verde sin haberlo compilado nunca.
        //
        // Eso no era teórico. `chronos-native` tiene la feature
        // `perf_counters` (no default), y sus diez referencias usaban
        // `super::perf::*` desde dentro de `mod imp` — donde `super` es el
        // módulo `ptrace_tracer`, no la raíz del crate, así que la ruta no
        // existía. Seis gates T4 consecutivos de este repo pasaron en
        // verde sin detectarlo, porque ninguno compilaba esa feature.
        //
        // El comando es copia literal de
        // .github/workflows/architecture-contracts.yml:34, que es donde la CI
        // remota lo detecta. La cabecera de este archivo ya fija la política:
        // "si la CI remota corre algo, este gate lo corre". Este stage es esa
        // política cumplida, no política inventada.
        //
        // Coste medido 2026-10-03: 35.2s (incremental, host compartido). Es
        // un `check`, no un `build`: no compila deps de release ni enlaza.
        // -----------------------------------------------------------------
        stage("feature-matrix-truth") {
            sh("cd $REPO && GATE_LOG=\"/tmp/chronos-gate-\$\$.log\" && cargo check --workspace --all-targets --all-features > \"\$GATE_LOG\" 2>&1; GATE_RC=$?; tail -20 \"\$GATE_LOG\"; test \$GATE_RC -eq 0")
        }

        // ---------------------------------------------------------------
        // ci-toolchain-parity
        //
        // Por qué este stage existe: `rust-toolchain.toml` fija el canal
        // `stable`, que FLOAT. La CI remota resuelve ese mismo canal en cada
        // run (`dtolnay/rust-toolchain@stable`), así que la CI siempre compila
        // con el stable MÁS RECIENTE, mientras el host de desarrollo puede
        // llevar semanas clavado en un stable anterior.
        //
        // La consecuencia es la misma clase de defecto que corrigió
        // feature-matrix-truth, pero por otro lado: un gate local puede
        // certificar en verde EXACTAMENTE lo que la CI remota rechaza.
        //
        // No era teórico. El 2026-10-03, el push 5654b9dd pasó fmt, clippy
        // y all-features en verde sobre clippy 1.98.1 local, y la CI lo
        // rechazó: `clippy::double_must_use` es un lint NUEVO de clippy
        // 1.99.0 sobre el `#[must_use]` que genera `async_trait` en
        // crates/chronos-domain/src/ports/browser_probe.rs:56. Con 1.98.1
        // ese lint no existe, así que ningún gate local podía verlo.
        //
        // Qué hace, y por qué no es un stage rojo permanente:
        //   - Compara el clippy local contra la versión que la CI usó por
        //     última vez (CHRONOS_CI_CLIPPY_VERSION, abajo).
        //   - Si coinciden: imprime "paridad OK" y sale. Coste ~0.
        //   - Si difieren: re-ejecuta el lint con EXACTAMENTE la toolchain de
        //     la CI y exige exit 0. Ahí es donde se detecta el fallo.
        // Es aut-curativo: en el host que hoy tiene 1.98.1 la stage paga un
        // clippy extra, y en cuanto el host suba a la versión de la CI vuelve
        // a costar nada. Un gate que se pone rojo para siempre y nadie arregla
        // es peor que no tenerlo.
        //
        // NO SE HACIA ANTES UNA MANIPULACIÓN DE TOOLCHAIN: si la toolchain de
        // la CI no está instalada, la stage falla diciendo qué instalar. No
        // declara PASS sin haber verificado, que es el principio de ADR-0004
        // aplicado al propio gate.
        //
        // Coste: NO MEDIDO todavía. El lint con 1.99.0 se midió en 42.3s con
        // CARGO_TARGET_DIR aparte; sobre el target compartido del host será
        // más. Se declara sin medir en vez de inventar una cifra.
        // -----------------------------------------------------------------
        stage("ci-toolchain-parity") {
            sh("""cd $REPO && GATE_LOG="/tmp/chronos-gate-${'$'}${'$'}.log" && CI_VER="1.99.0" && LOCAL_VER=${'$'}(rustc --version | cut -d' ' -f2) && echo "clippy local: ${'$'}LOCAL_VER | CI (último run observado): ${'$'}CI_VER" && if [ "${'$'}LOCAL_VER" = "${'$'}CI_VER" ]; then echo 'PARIDAD OK: el gate local lint corre con la misma toolchain que la CI remota'; else echo "DRIFT DETECTADO: local=${'$'}LOCAL_VER != CI=${'$'}CI_VER. Un lint nuevo puede pasar aqui y rechazar en remoto. Re-ejecutando el lint con la toolchain exacta de la CI."; if rustup toolchain list | grep -q "^${'$'}CI_VER-"; then CARGO_TARGET_DIR=/tmp/chronos-parity-target cargo +${'$'}CI_VER clippy --workspace --all-targets -- -D warnings > "${'$'}GATE_LOG" 2>&1; GATE_RC=${'$'}?; tail -30 "${'$'}GATE_LOG"; test ${'$'}GATE_RC -eq 0; else echo "PARIDAD NO VERIFICADA: la toolchain ${'$'}CI_VER no está instalada en este host. Instálala con 'rustup toolchain install ${'$'}CI_VER --component clippy' y repite el gate. Esta stage no declara PASS sin verificar."; exit 1; fi; fi""")
        }

        stage("production-bin-build") {
            // -----------------------------------------------------------------
            // ¿Por qué este stage existe: TERCER caso del mismo modo de fallo.
            //
            // 1. feature-matrix-truth: el gate certificaba código que NUNCA se
            //    había compilado (una feature no-default).
            // 2. ci-toolchain-parity: el gate certificaba código compilado con
            //    OTRO compilador (lint nuevo de clippy 1.99).
            // 3. este: el gate certificaba código que sólo compilaba bajo el
            //    harness de test, y el binario de PRODUCCIÓN no compilaba.
            //
            // El 2026-10-03, el commit b8ba75ae pasó fmt, clippy
            // --all-targets y 107/107 tests, y el Sandbox Debt Sentinel lo
            // rechazó con E0599. Motivo: `mod tests` en
            // crates/chronos-mcp/src/server.rs NO está gated con
            // `#[cfg(test)]` (decisión pre-existente explícita, server.rs:4318),
            // así que se compila en el build normal; rustc elimina las fns
            // anotadas con `#[test]`/`#[tokio::test]` cuando no hay harness, pero
            // una `fn` desnuda SÍ se compila. Tres helpers nuevos llamaban a
            // `SessionExecutionLog::create_for_tests`, que está gated
            // `#[cfg(any(test, feature = "test-utils"))]`, y en
            // `cargo build` las dev-dependencies no existen. E0599.
            //
            // El comando es LITERALMENTE el de sandbox-debt-sentinel.yml:45. La
            // política del gate ya decía "si la CI remota corre algo, este gate
            // lo corre"; el gate incumplia su propia regla por tercera vez, y
            // esta vez por una línea de diferencia.
            //
            // Coste: SIN MEDIR. No se inventa una cifra.
            // -----------------------------------------------------------------
            sh("""cd $REPO && GATE_LOG="/tmp/chronos-gate-${'$'}${'$'}.log" && echo "== build de produccion (sin harness de test) ==" && cargo build --bin chronos-mcp > "${'$'}GATE_LOG" 2>&1; GATE_RC=${'$'}?; tail -20 "${'$'}GATE_LOG"; test ${'$'}GATE_RC -eq 0""")
        }

        stage("build-domain") {
            // Compilar el crate domain (más pequeño, no requiere eBPF ni Go).
            sh("cd $REPO && GATE_LOG=\"/tmp/chronos-gate-\$\$.log\" && cargo build -p chronos-domain > \"\$GATE_LOG\" 2>&1; GATE_RC=$?; tail -10 \"\$GATE_LOG\"; test \$GATE_RC -eq 0")
        }

        // ---------------------------------------------------------------
        // lint-workspace
        //
        // Copia literal de .github/workflows/ci.yml (job `test`):
        //   cargo clippy --workspace --all-targets -- -D warnings
        //
        // Por qué este stage existe: el gate local no tenía lint. Una
        // regresión de clippy sólo la detectaba la CI remota, es decir que
        // "Pipeline finished with SUCCESS" podía certificar un árbol que la
        // CI remota.rejectaría. -D warnings convierte el lint en gate, no
        // en aviso.
        // ---------------------------------------------------------------
        stage("lint-workspace") {
            echo("Lint del workspace: cargo clippy --workspace --all-targets -- -D warnings (paridad con ci.yml)")
            sh("cd $REPO && GATE_LOG=\"/tmp/chronos-gate-\$\$.log\" && cargo clippy --workspace --all-targets -- -D warnings > \"\$GATE_LOG\" 2>&1; GATE_RC=$?; tail -30 \"\$GATE_LOG\"; test \$GATE_RC -eq 0")
        }

        // ---------------------------------------------------------------
        // test-workspace-lib
        //
        // Copia literal de .github/workflows/ci.yml:
        //   cargo test --workspace --lib -- --test-threads=1
        //
        // Por qué este stage existe: esta es la superficie que el gate
        // local nunca ejecutó. Medido el 2026-10-01 sobre b5edae57:
        // 18 binarios, 1421 tests passed, 0 failed, 5 ignored. El gate
        // anterior cubría 2 de esos 1421 (sólo el E2E read-path), o sea
        // el 0.14% de la superficie unitaria del workspace.
        // ---------------------------------------------------------------
        stage("test-workspace-lib") {
            echo("Tests unitarios del workspace: cargo test --workspace --lib (1421 tests medidos el 2026-10-01)")
            sh("cd $REPO && GATE_LOG=\"/tmp/chronos-gate-\$\$.log\" && cargo test --workspace --lib -- --test-threads=1 > \"\$GATE_LOG\" 2>&1; GATE_RC=$?; tail -30 \"\$GATE_LOG\"; test \$GATE_RC -eq 0")
        }

        // ---------------------------------------------------------------
        // test-workspace-integration  (TIER 2 — opt-in)
        //
        // Copia literal de .github/workflows/ci.yml:
        //   python3 scripts/derive_test_buckets.py --check-inventory \
        //     --out-dir .sddk-state/test-buckets
        //   SKIP_ARGS=$(awk '{printf " --skip %s", $1}' \
        //     .sddk-state/test-buckets/cargo-skip.txt)
        //   cargo test --workspace --tests --exclude chronos-e2e \
        //     -- --test-threads=1 $SKIP_ARGS
        //
        // POR QUÉ ES OPT-IN Y NO OBLIGATORIA
        //   Se midió el 2026-10-01 sobre b5edae57: la matriz completa
        //   (65 binarios de tests/) pasó de 47 min sin terminar, con el host
        //   corriendo además sddk-framework, assuranceGraph y
        //   agent-secretless contra el mismo cargo-targets. Sin integración
        //   obligatoriao el gate es utilizable; con ella, la tentación de
        //   saltárselo es mayor que el riesgo que cubre, porque el TIER 1 ya
        //   cubre lint y las 1421 pruebas unitarias, que es donde caen las
        //   regresiones de producto. La matriz completa sigue ejecutándose
        //   íntegra en ci.yml, en remoto y con runners dedicados.
        //
        //   No se degrada en silencio: sin CHRONOS_FULL_GATE=1 la stage
        //   imprime que TIER 2 no se ejecutó y por qué. Es el mismo
        //   principio "No Silent Lies" que exige que el sentinel declare sus
        //   skips en vez de omitirlos sin avisar.
        //
        // El skip list NO se inventa: lo deriva el sentinel del propio
        // proyecto, que hoy produce 0 filtros (0 tests diferidos). Si
        // aparece deuda diferida, el sentinel la refleja sola y esta stage
        // la salta sin que nadie edite el pipeline.
        //
        // Cada sh(...) corre en su propio shell, así que SKIP_ARGS se
        // recalcula dentro del mismo step que la usa; escribirlo en un
        // step anterior no lo propagaría.
        //
        // --exclude chronos-e2e replica la exclusión de la CI remota:
        // la superficie e2e la cubre el stage test-sandbox-read-path de
        // este mismo pipeline y sandbox-smoke.yml en remoto.
        // ---------------------------------------------------------------
        stage("test-workspace-integration") {
            sh("cd $REPO && if [ \"\${CHRONOS_FULL_GATE:-0}\" = \"1\" ]; then echo 'TIER 2 ACTIVADO (CHRONOS_FULL_GATE=1): ejecutando la matriz completa de integración'; else echo 'TIER 2 NO EJECUTADO: CHRONOS_FULL_GATE != 1. Este run cubre TIER 1 (lint + 1421 tests unitarios + contratos de arquitectura + E2E read-path). La matriz completa --workspace --tests corre íntegra en ci.yml; para replicarla en local usa CHRONOS_FULL_GATE=1'; fi")
            sh("cd $REPO && GATE_LOG=\"/tmp/chronos-gate-\$\$.log\" && if [ \"\${CHRONOS_FULL_GATE:-0}\" = \"1\" ]; then python3 scripts/derive_test_buckets.py --check-inventory --out-dir .sddk-state/test-buckets > \"\$GATE_LOG\" 2>&1; GATE_RC=$?; tail -5 \"\$GATE_LOG\"; test \$GATE_RC -eq 0; else echo 'skip de sentinel: no requerido en TIER 1'; fi")
            sh("cd $REPO && GATE_LOG=\"/tmp/chronos-gate-\$\$.log\" && if [ \"\${CHRONOS_FULL_GATE:-0}\" = \"1\" ]; then SKIP_ARGS=\$(awk '{printf \" --skip %s\", \$1}' .sddk-state/test-buckets/cargo-skip.txt) && echo \"Skip args: \$SKIP_ARGS\" && cargo test --workspace --tests --exclude chronos-e2e -- --test-threads=1 \$SKIP_ARGS > \"\$GATE_LOG\" 2>&1; GATE_RC=$?; tail -30 \"\$GATE_LOG\"; test \$GATE_RC -eq 0; else echo 'matriz de integración omitida en TIER 1 por diseño; no es un PASS de integración'; fi")
        }

        // ---------------------------------------------------------------
        // architecture-contracts
        //
        // Copia de .github/workflows/architecture-contracts.yml:
        //   python3 scripts/check_architecture_contracts.py
        //
        // Se usa --strict-no-gaps porque es la invocación que el propio
        // proyecto registra como gate en STATE.md (R5/R6) y es la que
        // convierte un hueco arquitectónico en fallo en vez de nota.
        //
        // CHRONOS_CONTRACT_BASE_REF: architecture-contracts.yml lo define
        // desde github.event.pull_request.base.sha (o event.before en push).
        // Sin él el script imprime "WARN: ... skipping added-line legacy
        // scan" y se salta media comprobación en silencio. Localmente el
        // equivalente es el padre del commit verificado, así que se fija
        // explícitamente: si no se hace, este stage reportaría PASS
        // habiendo comprobado la mitad de lo que comprueba la remota.
        // Si HEAD~1 no resuelve (clon superficial, primer commit), se deja
        // sin definir y el script vuelve a avisar en vez de mentir.
        //
        // Es el gate que más directamente protege el invariante de este
        // proyecto: el dominio no debe depender de infraestructura, y los
        // servicios consumen puertos con la raíz de composición conectar
        // adaptadores. Sin este stage, esa regla no la verificaba nada
        // localmente.
        // ---------------------------------------------------------------
        stage("architecture-contracts") {
            echo("Contratos de arquitectura: check_architecture_contracts.py --strict-no-gaps (base=HEAD~1)")
            sh("cd $REPO && GATE_LOG=\"/tmp/chronos-gate-\$\$.log\" && if git rev-parse --verify -q HEAD~1 >/dev/null; then CHRONOS_CONTRACT_BASE_REF=HEAD~1 python3 scripts/check_architecture_contracts.py --strict-no-gaps > \"\$GATE_LOG\" 2>&1; GATE_RC=$?; tail -20 \"\$GATE_LOG\"; test \$GATE_RC -eq 0; else echo 'HEAD~1 no resuelve: se omite el escaneo de legacy y el script lo declara'; python3 scripts/check_architecture_contracts.py --strict-no-gaps > \"\$GATE_LOG\" 2>&1; GATE_RC=$?; tail -20 \"\$GATE_LOG\"; test \$GATE_RC -eq 0; fi")
        }

        // ---------------------------------------------------------------
        // test-sandbox-read-path
        //
        // Por qué este stage existe: hasta ahora el pipeline sólo hacía
        // `cargo check` + `cargo build`, así que un "Pipeline finished with
        // SUCCESS" certificaba COMPILACIÓN, no comportamiento. Los tests
        // del read path (chronos-sandbox/tests/execution_log_read_e2e.rs)
        // hablan JSON-RPC real contra el binario chronos-mcp y nunca se
        // ejecutaban en el gate local.
        //
        // El orden importa: `cargo build --bin chronos-mcp` va ANTES del test
        // porque `cargo test -p chronos-sandbox` no construye ese binario
        // (vive en crates/chronos-mcp, y CARGO_BIN_EXE_* sólo se define
        // para binarios del mismo crate). Sin este paso, el resolver puede
        // arrancar un binario viejo y dar verde falso. Es el mismo defecto
        // que se corrigió en .github/workflows/sandbox-smoke.yml (da93f6cf).
        // ---------------------------------------------------------------
        stage("test-sandbox-read-path") {
            sh("cd $REPO && GATE_LOG=\"/tmp/chronos-gate-\$\$.log\" && cargo build --bin chronos-mcp > \"\$GATE_LOG\" 2>&1; GATE_RC=$?; tail -10 \"\$GATE_LOG\"; test \$GATE_RC -eq 0")
            sh("cd $REPO && GATE_LOG=\"/tmp/chronos-gate-\$\$.log\" && cargo test -p chronos-sandbox --test execution_log_read_e2e -- --test-threads=1 > \"\$GATE_LOG\" 2>&1; GATE_RC=$?; tail -25 \"\$GATE_LOG\"; test \$GATE_RC -eq 0")
        }

        stage("evidence") {
            sh("ls -la $REPO/.pipelinek/db.sqlite")
            sh("test -d $REPO/.pipelinek/control/last-run && echo 'last-run present'")
            sh("test -d $REPO/.pipelinek/control/workspace && echo 'workspace tracking present'")
        }
    }
}
