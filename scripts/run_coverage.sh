#!/bin/bash
set -e
cd "$(dirname "$0")/.."

# Install tarpaulin if not present
if ! command -v cargo-tarpaulin &> /dev/null; then
    echo "Installing cargo-tarpaulin..."
    cargo install cargo-tarpaulin
fi

# Run coverage.
# Note: `cargo tarpaulin` changed its argument shape; `--out` is now a format
# selector (Html/Json/Xml/Lcov), and the output directory is `--output-dir`.
# The legacy `-o <path>` is rejected as an invalid `--out` value on
# tarpaulin >= 0.27.
mkdir -p ./metrics/coverage
cargo tarpaulin --workspace --out Html --out Json --out Xml --output-dir ./metrics/coverage/

# Upload to codecov if token present.
# tarpaulin emits files without a `tarpaulin.` prefix in --output-dir mode;
# discover them rather than hard-coding the name.
if [ -n "$CODECOV_TOKEN" ]; then
    COVERAGE_XML=$(ls ./metrics/coverage/cobertura.xml ./metrics/coverage/tarpaulin.xml 2>/dev/null | head -1)
    if [ -n "$COVERAGE_XML" ] && [ -f "$COVERAGE_XML" ]; then
        codecovupload -t "$CODECOV_TOKEN" -f "$COVERAGE_XML" || true
    else
        echo "No XML coverage report found under ./metrics/coverage/, skipping upload"
    fi
fi

echo "Coverage report generated at ./metrics/coverage/"
