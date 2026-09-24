#!/usr/bin/env bash
# test-lib.sh — batería unitaria/lib del workspace (replica step de ci.yml).
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
exec cargo test --workspace --lib -- --test-threads=1
