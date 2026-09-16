#!/bin/bash
set -e
cd "$(dirname "$0")/.."

# Pin tarpaulin to the same version used in CI for reproducibility.
# `--locked` keeps Cargo.lock authoritative.
TARPAULIN_VERSION="${TARPAULIN_VERSION:-0.37.2}"

# Install tarpaulin if not present (or wrong version).
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "Installing cargo-tarpaulin ${TARPAULIN_VERSION}..."
    cargo install cargo-tarpaulin --version "${TARPAULIN_VERSION}" --locked
fi

# Validate that the installed tarpaulin matches the pinned version. This
# guards against silent CLI drift when cargo-tarpaulin changes flag
# semantics between releases (the previous `-o` invocation was a tarpaulin
# 0.36 vs 0.37 semantics mismatch).
INSTALLED="$(cargo tarpaulin --version 2>/dev/null | awk '{print $NF}')"
if [[ -n "${INSTALLED}" && "${INSTALLED}" != "${TARPAULIN_VERSION}" ]]; then
    echo "Warning: installed cargo-tarpaulin ${INSTALLED} != pinned ${TARPAULIN_VERSION}" >&2
    echo "Reinstalling the pinned version..." >&2
    cargo install --force cargo-tarpaulin --version "${TARPAULIN_VERSION}" --locked
fi

# Run coverage.
# `cargo-tarpaulin --out` takes a format token (Html | Json | Xml | Lcov |
# Stdout) — NOT a path. The path goes to `--output-dir`. Mixing them up
# produces "error: invalid value '<PATH>' for '--out [<FMT>...]'", which
# is what previously broke the Coverage workflow.
mkdir -p ./metrics/coverage
# Deferred test filters are derived and inventory-validated from the ledger.
python3 scripts/derive_test_buckets.py --check-inventory --out-dir .sddk-state/test-buckets
mapfile -t DEFERRED_SKIPS < .sddk-state/test-buckets/cargo-skip.txt
TARPAULIN_TEST_ARGS=()
for skip in "${DEFERRED_SKIPS[@]}"; do TARPAULIN_TEST_ARGS+=(--skip "$skip"); done
cargo tarpaulin \
    --engine llvm \
    --workspace \
    --out Html \
    --out Json \
    --out Xml \
    --output-dir ./metrics/coverage/ \
    -- "${TARPAULIN_TEST_ARGS[@]}"

# Upload to codecov if token present.
if [ -n "$CODECOV_TOKEN" ]; then
    codecovupload -t "$CODECOV_TOKEN" -f ./metrics/coverage/tarpaulin.xml
fi

echo "Coverage report generated at ./metrics/coverage/"
