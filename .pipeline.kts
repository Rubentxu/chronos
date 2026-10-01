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
//      Workaround: usar `${PIPESTATUS[0]}` y `test` para preservar el exit code.
//   3. La primera stage debe ser discover-repo (contrato AGENTS.md §CI Local).

val REPO = "/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos"

pipeline {
    stages {
        stage("discover-repo") {
            sh("ls -la $REPO/Cargo.toml $REPO/AGENTS.md | head -3")
            sh("head -3 $REPO/Cargo.toml")
        }

        stage("workspace-check") {
            // cargo check --workspace: acota tiempo evitando compilar deps pesadas.
            sh("cd $REPO && cargo check --workspace --message-format=short 2>&1 | tail -20; test \${PIPESTATUS[0]} -eq 0")
        }

        stage("build-domain") {
            // Compilar el crate domain (más pequeño, no requiere eBPF ni Go).
            sh("cd $REPO && cargo build -p chronos-domain 2>&1 | tail -10; test \${PIPESTATUS[0]} -eq 0")
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
            sh("cd $REPO && cargo clippy --workspace --all-targets -- -D warnings 2>&1 | tail -30; test \${PIPESTATUS[0]} -eq 0")
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
            sh("cd $REPO && cargo test --workspace --lib -- --test-threads=1 2>&1 | tail -30; test \${PIPESTATUS[0]} -eq 0")
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
            sh("cd $REPO && if [ \"\${CHRONOS_FULL_GATE:-0}\" = \"1\" ]; then python3 scripts/derive_test_buckets.py --check-inventory --out-dir .sddk-state/test-buckets 2>&1 | tail -5; test \${PIPESTATUS[0]} -eq 0; else echo 'skip de sentinel: no requerido en TIER 1'; fi")
            sh("cd $REPO && if [ \"\${CHRONOS_FULL_GATE:-0}\" = \"1\" ]; then SKIP_ARGS=\$(awk '{printf \" --skip %s\", \$1}' .sddk-state/test-buckets/cargo-skip.txt) && echo \"Skip args: \$SKIP_ARGS\" && cargo test --workspace --tests --exclude chronos-e2e -- --test-threads=1 \$SKIP_ARGS 2>&1 | tail -30; test \${PIPESTATUS[0]} -eq 0; else echo 'matriz de integración omitida en TIER 1 por diseño; no es un PASS de integración'; fi")
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
            sh("cd $REPO && if git rev-parse --verify -q HEAD~1 >/dev/null; then CHRONOS_CONTRACT_BASE_REF=HEAD~1 python3 scripts/check_architecture_contracts.py --strict-no-gaps 2>&1 | tail -20; test \${PIPESTATUS[0]} -eq 0; else echo 'HEAD~1 no resuelve: se omite el escaneo de legacy y el script lo declara'; python3 scripts/check_architecture_contracts.py --strict-no-gaps 2>&1 | tail -20; test \${PIPESTATUS[0]} -eq 0; fi")
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
            sh("cd $REPO && cargo build --bin chronos-mcp 2>&1 | tail -10; test \${PIPESTATUS[0]} -eq 0")
            sh("cd $REPO && cargo test -p chronos-sandbox --test execution_log_read_e2e -- --test-threads=1 2>&1 | tail -25; test \${PIPESTATUS[0]} -eq 0")
        }

        stage("evidence") {
            sh("ls -la $REPO/.pipelinek/db.sqlite")
            sh("test -d $REPO/.pipelinek/control/last-run && echo 'last-run present'")
            sh("test -d $REPO/.pipelinek/control/workspace && echo 'workspace tracking present'")
        }
    }
}
