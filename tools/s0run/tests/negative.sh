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

echo "== N4: podman absence of image => unsupported, no host run"
out=$(S0_PODMAN_IMAGE=docker.io/library/does-not-exist-s0:0 python3 s0run.py --scenario "$SC" --env podman 2>/dev/null)
echo "$out" | python3 -c "
import json,sys
d=json.load(sys.stdin)
assert d['result']=='fail', d['result']
prep=d['prepared']
assert prep['image_status']=='unsupported', prep['image_status']
assert not any('parent-start' in (a.get('stdout') or '') for a in d['artifacts']), 'host output leaked'
print('  N4 PASS: image absent -> unsupported at prepare, nothing ran')
" || fail=1

echo "== N5: podman staging invariants (no bind mounts, no registry at execute)"
python3 -c "
import pathlib,re
src=pathlib.Path('s0run.py').read_text()
seg=src[src.index('class PodmanAdapter'):src.index('def run(scenario')]
assert '--pull=never' in seg, 'execute must not be able to pull'
assert '\"-v\"' not in seg and '--volume' not in seg, 'no bind mounts allowed'
assert 'tar-stream-in/out' in seg
print('  N5 PASS: --pull=never present, no -v/--volume in the podman adapter')
" || fail=1

echo "== N6: podman cleanup verified (no stray containers after a run)"
before=$(podman ps -a --format '{{.Names}}' | grep -c '^s0run-' || true)
python3 s0run.py --scenario "$SC" --env podman >/dev/null 2>&1 || fail=1
after=$(podman ps -a --format '{{.Names}}' | grep -c '^s0run-' || true)
if [ "$before" = "$after" ]; then echo "  N6 PASS: no stray s0run-* containers (before=$before after=$after)"; else echo "  N6 FAIL: stray containers ($before -> $after)"; fail=1; fi

echo "NEGATIVE_TESTS=$([ $fail -eq 0 ] && echo PASS || echo FAIL)"
exit $fail
