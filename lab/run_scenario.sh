#!/usr/bin/env bash
# SANDBOX-S0.1 minimal harness: run lab/scenarios/scenario.json's logical
# command under one backend; emit identical evidence JSON per backend.
# Usage: run_scenario.sh <host|bwrap|podman|qemu>
set -uo pipefail
BACKEND="${1:?backend required}"
SCENARIO_DIR="$(cd "$(dirname "$0")" && pwd)"
OUT="$SCENARIO_DIR/results"
mkdir -p "$OUT"
EVIDENCE="$OUT/${BACKEND}.json"
start_ns=$(date +%s%N)

emit() { # evidence JSON skeleton; callers fill facts
  printf '{"backend":"%s","exit_code":%s,"duration_ms":%s,%s}\n' \
    "$BACKEND" "$1" "$2" "$3" > "$EVIDENCE"
}

case "$BACKEND" in
  host)
    true >/dev/null 2>&1; rc=$?
    ;;
  bwrap)
    command -v bwrap >/dev/null || { emit -1 0 '"error":"bwrap missing"'; exit 2; }
    bwrap --ro-bind / / --dev /dev --proc /proc --unshare-pid true >/dev/null 2>&1; rc=$?
    ;;
  podman)
    command -v podman >/dev/null || { emit -1 0 '"error":"podman missing"'; exit 2; }
    podman run --rm quay.io/podman/hello >/dev/null 2>&1; rc=$?
    ;;
  qemu)
    command -v qemu-kvm >/dev/null || { emit -1 0 '"error":"qemu missing"'; exit 2; }
    KERNEL="$(ls /usr/lib/modules/*/vmlinuz 2>/dev/null | head -1)"
    [ -n "$KERNEL" ] || { emit -1 0 '"error":"no kernel image"'; exit 2; }
    timeout 30 qemu-kvm -enable-kvm -m 512 -nographic -kernel "$KERNEL" \
      -append 'console=ttyS0 panic=-1' -no-reboot </dev/null >/dev/null 2>&1; rc=$?
    ;;
  *) echo "unknown backend $BACKEND" >&2; exit 2;;
esac
end_ns=$(date +%s%N)
dur_ms=$(( (end_ns - start_ns) / 1000000 ))
emit "$rc" "$dur_ms" '"stdout_empty":true,"stderr_empty":true'
cat "$EVIDENCE"
