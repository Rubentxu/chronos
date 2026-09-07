#!/bin/bash
set -e
cd "$(dirname "$0")/.."

# Install tarpaulin if not present
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "Installing cargo-tarpaulin..."
    cargo install cargo-tarpaulin
fi

# Run coverage
mkdir -p ./metrics/coverage
cargo tarpaulin --workspace --out html --out json --out xml -o ./metrics/coverage/

# Upload to codecov if token present
if [ -n "$CODECOV_TOKEN" ]; then
    codecovupload -t "$CODECOV_TOKEN" -f ./metrics/coverage/tarpaulin.xml
fi

echo "Coverage report generated at ./metrics/coverage/"
