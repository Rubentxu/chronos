#!/usr/bin/env bash
# Pre-push gate: ejecuta los comandos que la CI remota ejecuta.
#
# Por que existe
# --------------
# `.pipeline.kts` ya declaraba el stage `lint-workspace` con el comando
# literal de `ci.yml`, y con un comentario que dice "paridad con ci.yml".
# Ese stage es correcto y no se ejecutaba nunca: nada en el repositorio lo
# invoca (ni justfile, ni Makefile, ni workflow), y no hay hooks de git
# instalados. La especificacion verificable por maquina existia, pero
# quien la ejecutaba era la memoria del operador.
#
# Ese hueco es la causa raiz de cinco incidencias seguidas, todas la misma:
#
#   1. feature que nunca compacto      -> check verde por no ejecutarse
#   2. compilado con otro compilador   -> check verde por no ejecutarse
#   3. compilado solo bajo harness     -> check verde por no ejecutarse
#   4. gate que no puede dispararse    -> check verde por no dispararse
#   5. clippy con un subconjunto de crates, no el workspace entero
#
# En la quinta el guard existia y fue saltado: se verifico con
# `cargo clippy -p chronos-log -p chronos-services` cuando el codigo
# cambiado vivia en `chronos-sandbox`. Un subconjunto que no cubre el
# cambio es un check que no comprueba nada.
#
# Politica: lo que la CI remota corre, este gate lo corre. Si anades un
# comando a `.github/workflows/`, anadelo tambien aqui.
#
# Uso
# ---
#   cp scripts/pre_push_gate.sh .git/hooks/pre-push
#   chmod +x .git/hooks/pre-push
#
# Bypass: `git push --no-verify` lo salta, igual que cualquier hook. Es
# deliberado: un hook que no se puede saltar se acaba deshabilitando en
# bloque y deja de valer para todos. Lo que este script hace es volver
# visible el salto en vez de ocultarlo.

set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

# Allow an explicit opt-out for a single invocation, e.g.
#   SKIP_PRE_PUSH_GATE=1 git push origin main
# Skipping is always printed, so it shows up in the terminal history rather
# than happening invisibly.
if [ "${SKIP_PRE_PUSH_GATE:-0}" = "1" ]; then
    echo "pre-push: SKIP_PRE_PUSH_GATE=1, puerta saltada declaradamente" >&2
    exit 0
fi

fail=0
run_gate() {
    local name="$1"; shift
    echo "pre-push: ${name}"
    if "$@"; then
        echo "pre-push: ${name} OK"
    else
        echo "pre-push: ${name} FALLO" >&2
        fail=1
    fi
}

# Paridad literal con ci.yml. El workspace COMPLETO y --all-targets: un
# subconjunto de crates es exactamente el fallo que produjo la quinta
# incidencia, asi que aqui no se acota.
run_gate "fmt" cargo fmt --all -- --check
run_gate "clippy (workspace, all-targets)" \
    cargo clippy --workspace --all-targets -- -D warnings

# CC#4: los SHA-256 de los manifests del vault. Se ejecuta solo con arbol
# limpio y commiteado, porque hashea el working tree. Si hay cambios sin
# commitear lo dice en vez de inventarse un resultado.
if [ -z "$(git status --porcelain)" ]; then
    if python3 scripts/regen_manifest_index_shas.py --check; then
        echo "pre-push: CC#4 clean"
    else
        echo "pre-push: CC#4 FALLO: hay SHA obsoletos en los manifests." >&2
        echo "pre-push: con el arbol limpio ejecuta scripts/regen_manifest_index_shas.py" >&2
        fail=1
    fi
else
    echo "pre-push: CC#4 omitido, el arbol tiene cambios sin commitear" >&2
fi

if [ "$fail" -ne 0 ]; then
    echo >&2
    echo "pre-push: puerta cerrada. Los fallos de arriba son los mismos que" >&2
    echo "pre-push: ejecutara la CI remota. No los ignores con --no-verify" >&2
    echo "pre-push: salvo que sepas que el gate remoto no corre ese comando." >&2
    exit 1
fi

echo "pre-push: todas las puertas locales en verde"
