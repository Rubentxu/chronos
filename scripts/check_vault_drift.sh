#!/usr/bin/env bash
#
# check_vault_drift.sh — execute the vault-drift-sweep CC#48 meta-check.
#
# Runs every CC in .sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md
# (except CC#48 itself to avoid recursion) and exits 1 if any CC reports drift.
#
# Usage:
#   ./scripts/check_vault_drift.sh
#
# Exit codes:
#   0 — all CCs clean
#   1 — at least one CC reported drift
#   2 — python3 not available
#
# This script is the entry point used by CI (.github/workflows/vault-drift.yml).

set -euo pipefail

cd "$(dirname "$0")/.."

if ! command -v python3 >/dev/null 2>&1; then
    echo "ERROR: python3 not found in PATH" >&2
    exit 2
fi

VAULT_FILE=".sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md"

if [[ ! -f "$VAULT_FILE" ]]; then
    echo "ERROR: $VAULT_FILE not found" >&2
    exit 2
fi

# Extract the CC#48 python block and execute it.
python3 - <<PYEOF
import re, subprocess, sys

text = open("$VAULT_FILE").read()
m = re.search(r'### 48\..*?\n\`\`\`python\n(.*?)\n\`\`\`', text, re.DOTALL)
if not m:
    print("ERROR: CC#48 block not found in vault-drift-sweep.md", file=sys.stderr)
    sys.exit(2)

# Capture the output of exec so we can report failures clearly.
import io
buf = io.StringIO()
old = sys.stdout
sys.stdout = buf
try:
    exec(m.group(1))
finally:
    sys.stdout = old

out = buf.getvalue().strip()
if out:
    print("DRIFT detected:")
    print(out)
    sys.exit(1)
else:
    print("vault-drift-sweep: PASS (52 CCs all clean)")
PYEOF
