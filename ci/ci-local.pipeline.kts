// ci-local.pipeline.kts — CI local de Chronos con PipelineK 0.39.0.
//
// Replica el job `test` de .github/workflows/ci.yml (fmt → clippy → build →
// unit/lib tests → integration tests con skip list del ledger). Es la
// verificación incremental (T0..T3) que precede al push; el gate de
// integración (T4/T5) sigue siendo GitHub Actions.
//
// Ejecutar: ci/run-pipelinek run ci-local.pipeline.kts
//
// NOTA DSL: los steps sh() corren en un workspace aislado del control-root,
// no en el repo, y Kotlin interpola $ dentro de las strings: cualquier $ de
// shell debe vivir en los scripts .sh de ci/, no aquí inline. REPO_ROOT lo
// exporta ci/run-pipelinek.

pipeline {
    stages {
        stage("t0-fmt") {
            sh("cd ${System.getenv("REPO_ROOT")} && cargo fmt --all -- --check")
        }
        stage("t0-clippy") {
            sh("cd ${System.getenv("REPO_ROOT")} && cargo clippy --workspace --all-targets -- -D warnings")
        }
        stage("t1-build") {
            sh("cd ${System.getenv("REPO_ROOT")} && cargo build --workspace")
        }
        stage("t1-test-lib") {
            sh("bash ${System.getenv("REPO_ROOT")}/ci/test-lib.sh < /dev/null")
        }
        stage("t2-test-integration") {
            // Misma superficie obligatoria que CI: ledger-driven skip list.
            sh("bash ${System.getenv("REPO_ROOT")}/ci/integration-surface.sh")
        }
    }
}
