#!/usr/bin/env bash
#
# check_vault_drift.sh — execute the vault-drift-sweep meta-checks.
#
# Runs CC#48 (python meta-check, executes every python CC) and
# CC#54 (bash meta-check, executes every bash CC). Exits 1 if any
# CC reports drift; 0 if all clean.
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

# ==============================================================================
# CC#48: python meta-check (executes every python CC)
# ==============================================================================
VAULT_FILE="$VAULT_FILE" python3 - <<'PYEOF'
import os, re, subprocess, sys

text = open(os.environ["VAULT_FILE"]).read()
m = re.search(r'### 48\..*?\n```python\n(.*?)\n```', text, re.DOTALL)
if not m:
    print("ERROR: CC#48 block not found in vault-drift-sweep.md", file=sys.stderr)
    sys.exit(2)

# Count total CCs and python-executable CCs for the PASS message.
sections = re.split(r'### (\d+)\.', text)
total_ccs = sum(1 for i in range(1, len(sections), 2) if int(sections[i]) != 48)
python_ccs = 0
bash_ccs = 0
for i in range(1, len(sections), 2):
    num = int(sections[i])
    if num == 48:
        continue
    body = sections[i+1]
    # CCs that CC#48 actually executes (have a python block).
    # Note: CC#54 itself has a bash block and is NOT executed by CC#48;
    # CC#54 is executed by the bash part of this script (below).
    if '```python' in body:
        python_ccs += 1
    # CC#54 is itself a bash CC (sibling meta-check, not executed by CC#48).
    if num == 54 or ('```bash' in body and num != 48):
        bash_ccs += 1

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
    print("DRIFT detected (CC#48):")
    print(out)
    sys.exit(1)

# Stash CC counts for the final message.
import json
with open('/tmp/check_vault_drift_counts.json', 'w') as f:
    json.dump({'total_ccs': total_ccs, 'python_ccs': python_ccs, 'bash_ccs': bash_ccs}, f)
PYEOF

# ==============================================================================
# CC#54: bash meta-check (executes every bash CC)
# ==============================================================================
bash_block=$(python3 - <<'PYEOF'
import re, sys
text = open(".sddk-knowledge/p-3416cfb8288f8964/maintenance/vault-drift-sweep.md").read()
m = re.search(r'### 54\..*?\n```bash\n(.*?)\n```', text, re.DOTALL)
if not m:
    print("ERROR: CC#54 block not found", file=sys.stderr)
    sys.exit(1)
print(m.group(1))
PYEOF
)

set +e
bash_out=$(bash -c "$bash_block" 2>&1)
bash_exit=$?
set -e
if [ $bash_exit -ne 0 ]; then
    echo "DRIFT detected (CC#54, exit $bash_exit):"
    echo "$bash_out"
    exit 1
fi

# All clean.
counts=$(cat /tmp/check_vault_drift_counts.json 2>/dev/null || echo '{"python_ccs": 0, "bash_ccs": 0}')
rm -f /tmp/check_vault_drift_counts.json
python_ccs=$(echo "$counts" | python3 -c "import json,sys; print(json.load(sys.stdin)['python_ccs'])")
bash_ccs=$(echo "$counts" | python3 -c "import json,sys; print(json.load(sys.stdin)['bash_ccs'])")
echo "vault-drift-sweep: PASS ($python_ccs python CCs all clean, $bash_ccs bash CCs all clean)"
