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

        stage("evidence") {
            sh("ls -la $REPO/.pipelinek/db.sqlite")
            sh("test -d $REPO/.pipelinek/control/last-run && echo 'last-run present'")
            sh("test -d $REPO/.pipelinek/control/workspace && echo 'workspace tracking present'")
        }
    }
}
