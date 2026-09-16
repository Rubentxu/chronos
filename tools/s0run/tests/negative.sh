#!/usr/bin/env bash
# SANDBOX-S0.1b negative tests: an unavailable or broken environment must
# produce an explicit result, never a silent degradation to the host.
set -uo pipefail
cd "$(dirname "$0")/.."
SC="scenarios/child-process-lifecycle.json"
fail=0

echo "== N1: podman unavailable => explicit skip/unsupported"
out=$(S0_FORCE_UNAVAILABLE=podman python3 s0run.py --scenario "$SC" --env podman)
echo "$out" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d['result']=='skip', d['result']
assert 'not available' in d['reason'], d['reason']
assert d['metrics']=={}, 'a skipped env must not report metrics'
print('  N1 PASS:', d['reason'])
" || fail=1

echo "== N2: broken image/mount => hard fail, never host fallback"
out=$(S0_PODMAN_IMAGE=docker.io/library/does-not-exist-s0:0 python3 s0run.py --scenario "$SC" --env podman 2>/dev/null)
echo "$out" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d['result']=='fail', d['result']
assert d['errors'], 'a container failure must surface as an error'
assert not any(
    a.get('kind')=='stdout_capture' and 'parent-start' in (a.get('stdout') or '')
    for a in d['artifacts']
), 'host output leaked: the scenario must NOT have run on the host'
print('  N2 PASS:', d['errors'][0][:100])
" || fail=1

echo "== N3: each result is labelled with its actual environment"
for e in host bwrap; do
  python3 s0run.py --scenario "$SC" --env "$e" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d['environment']=='$e', d['environment']
print('  N3 PASS:', d['environment'], d['result'])
" || fail=1
done

echo "NEGATIVE_TESTS=$([ $fail -eq 0 ] && echo PASS || echo FAIL)"
exit $fail
