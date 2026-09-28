// .pipeline.kts — chronos (Rust workspace, 13 crates)
// Ejecuta verificación real del workspace: cargo check + cargo build.
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
