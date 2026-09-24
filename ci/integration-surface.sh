#!/usr/bin/env bash
# integration-surface.sh — Superficie de integración obligatoria de Chronos.
#
# Replica exactamente el step "Test (integration, mandatory surface)" de
# .github/workflows/ci.yml. La skip list se deriva del ledger
# reconstruction-contracts.toml vía scripts/derive_test_buckets.py, de modo
# que añadir/quitar deuda diferida actualiza CI remoto y CI local a la vez.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

python3 scripts/derive_test_buckets.py --check-inventory --out-dir .sddk-state/test-buckets
SKIP_ARGS=$(awk '{printf " --skip %s", $1}' .sddk-state/test-buckets/cargo-skip.txt)
echo "Mandatory surface: cargo test --workspace --tests (skip list: ${SKIP_ARGS:-none})"
# shellcheck disable=SC2086
exec cargo test --workspace --tests --exclude chronos-e2e -- --test-threads=1 $SKIP_ARGS
