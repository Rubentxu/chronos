#!/usr/bin/env python3
"""Derive CI, coverage, and debt-sentinel test buckets from the contract ledger."""
from __future__ import annotations
import argparse, json, subprocess, sys, tomllib
from pathlib import Path

def fail(message: str) -> None:
    raise ValueError(message)

def leaf(test_id: str) -> str:
    return test_id.rsplit('::', 1)[-1]

def target(test_id: str) -> str:
    return test_id.split('::', 1)[0]

def suite_targets(suite: str) -> list[dict[str, str]]:
    result=[]
    for part in suite.split(' + '):
        path=Path(part)
        if len(path.parts) < 3 or path.parts[-2] != 'tests' or path.suffix != '.rs':
            fail(f"invalid concrete suite path: {part!r}")
        result.append({'package': path.parts[-3], 'target': path.stem})
    return result

def inventory(root: Path, runs: list[dict[str,str]]) -> dict[str,set[str]]:
    found={f"{r['package']}::{r['target']}":set() for r in runs}
    for r in runs:
        key=f"{r['package']}::{r['target']}"
        p=subprocess.run(['cargo','test','-p',r['package'],'--test',r['target'],'--','--list'],cwd=root,text=True,capture_output=True)
        if p.returncode: fail(f"inventory failed for {key}: {p.stderr[-1000:]}")
        found[key]={line.split(':',1)[0].strip() for line in p.stdout.splitlines() if ': test' in line}
    return found

def derive(root: Path, out: Path, check_inventory: bool) -> int:
    with (root/'reconstruction-contracts.toml').open('rb') as f: ledger=tomllib.load(f)
    base=ledger.get('baseline_scope',{})
    deferred=base.get('deferred',{}).get('failures',[])
    privileged=base.get('privileged',{}).get('failures',[])
    seen={}
    concrete=[]; runs=[]
    for bucket, entries in [('deferred',deferred),('privileged',privileged)]:
        for e in entries:
            tests=e.get('tests', [])
            count=e.get('count',0)
            if bucket == 'deferred' and count and not tests: fail(f"{e.get('id','<unknown>')}: count > 0 requires concrete tests")
            if bucket == 'deferred' and tests and count != len(tests): fail(f"{e['id']}: count={count}, tests={len(tests)}")
            for tid in tests:
                if tid.count('::') < 1: fail(f"{e['id']}: malformed test ID {tid!r}")
                if tid in seen and not e.get('allow_overlap',False): fail(f"{tid} appears in both {seen[tid]} and {bucket} without allow_overlap")
                seen[tid]=bucket
            if bucket == 'deferred':
                ts=suite_targets(e.get('suite',''))
                expected_targets={target(t) for t in tests}
                if expected_targets != {x['target'] for x in ts}: fail(f"{e['id']}: suite targets must exactly cover declared test IDs")
                concrete.append({k:e[k] for k in ('id','suite','count','owner_gate','reason','tests')})
                runs.extend(ts)
    # Deterministic run plan, wholly ledger-derived.
    runs=sorted({(r['package'],r['target']) for r in runs})
    run_plan=[{'package':p,'target':t} for p,t in runs]
    if check_inventory:
        available=inventory(root,run_plan)
        declared={(target(t),leaf(t)) for e in concrete for t in e['tests']}
        for t,l in declared:
            candidates=[r for r in run_plan if r['target']==t and l in available[f"{r['package']}::{r['target']}"]]
            if len(candidates) != 1: fail(f"{t}::{l}: inventory match count is {len(candidates)}, expected exactly 1")
        # Skip patterns are leaf filters. Each can only match declared tests in its target run(s).
        for pattern in sorted({leaf(t) for _,t in declared}):
            matches={(r['target'],name) for r in run_plan for name in available[f"{r['package']}::{r['target']}"] if pattern in name}
            unexpected=matches-declared
            if unexpected: fail(f"skip filter {pattern!r} also matches undeclared tests: {sorted(unexpected)}")
    patterns=sorted({leaf(t) for e in concrete for t in e['tests']})
    manifest={'version':2,'generated_from':'reconstruction-contracts.toml','deferred':concrete,'runs':run_plan,'skip_patterns':patterns}
    out.mkdir(parents=True,exist_ok=True)
    (out/'cargo-skip.txt').write_text('\n'.join(patterns)+'\n')
    (out/'sentinel-manifest.json').write_text(json.dumps(manifest,indent=2)+'\n')
    print(f"derived {len(patterns)} exact deferred test filters and {len(run_plan)} ledger run targets")
    return 0

def main():
    a=argparse.ArgumentParser();a.add_argument('--root',type=Path,default=Path(__file__).resolve().parent.parent);a.add_argument('--out-dir',type=Path,default=Path('.sddk-state/test-buckets'));a.add_argument('--check-inventory',action='store_true');x=a.parse_args()
    try:return derive(x.root.resolve(),x.out_dir.resolve(),x.check_inventory)
    except (ValueError,tomllib.TOMLDecodeError) as e: print(f'derive_test_buckets: ERROR: {e}',file=sys.stderr);return 2
if __name__=='__main__': raise SystemExit(main())
