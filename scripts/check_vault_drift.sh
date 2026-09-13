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
VAULT_FILE="$VAULT_FILE" python3 - <<'PYEOF'
import os, re, subprocess, sys

text = open(os.environ["VAULT_FILE"]).read()
m = re.search(r'### 48\..*?\n```python\n(.*?)\n```', text, re.DOTALL)
if not m:
    print("ERROR: CC#48 block not found in vault-drift-sweep.md", file=sys.stderr)
    sys.exit(2)

# Count total CCs (excluding CC#48 itself) for the PASS message.
sections = re.split(r'### (\d+)\.', text)
total_ccs = sum(1 for i in range(1, len(sections), 2) if int(sections[i]) != 48)
# Count how many CCs are executable by CC#48 (i.e., have a python block).
python_ccs = 0
for i in range(1, len(sections), 2):
    num = int(sections[i])
    if num == 48:
        continue
    body = sections[i+1]
    if '```python' in body:
        python_ccs += 1
# CCs without a python block (bash or non-executable) are validated
# manually or via the CI workflow branch-merged check (see CC#53).

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
    print(f"vault-drift-sweep: PASS ({python_ccs} python CCs all clean, {total_ccs - python_ccs} bash CC documented separately)")
PYEOF
