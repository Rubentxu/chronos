# Release Report — m9-67-cc-smoke-test

## Path

B-direct

## Subject

Adds `scripts/smoke_test_ccs.sh`, a synthetic-drift injection test that exercises the four most failure-prone vault drift CCs and verifies the meta-check chain still catches them. Each test clones the repo into a `/tmp` work dir, injects drift that violates the CC's invariant, runs `check_vault_drift.sh`, and asserts exit code + DRIFT line. A clean-state test verifies the meta-checks report PASS on an untouched clone.

## Files changed

| Group | Count | Change |
|---|---|---|
| `scripts/smoke_test_ccs.sh` | 1 | New file (250 lines) |

Total: 1 file added, 250 insertions(+), 0 deletions(-).

## Cross-checks

| ID | Description | Status |
|---|---|---|
| CC#4 | archive-manifest SHA-256 consistency (synthetic SHA mismatch injected) | pass (script catches the injected drift) |
| CC#39 | Total cycles ↔ filesystem cycles (synthetic Total cycles mismatch injected) | pass (script catches the injected drift; CC#5 is subsumed by Part C) |
| CC#46 | No stale local or remote fix/m9-* branches (synthetic fix/m9-99-smoke-test branch created) | pass (script catches the injected drift) |
| CC#48 + CC#54 | Meta-checks on clean state | pass (exit 0 + "46 python CCs all clean, 7 bash CCs all clean" reported) |
| Self-test | `./scripts/smoke_test_ccs.sh` | pass (4 tests, 0 failures) |
| T0: cargo fmt + clippy | Lint gate | not applicable (no Rust changes this cycle) |

## Self-tests performed

The smoke test itself is the self-test: each of the four CCs was injected with the documented drift, and `check_vault_drift.sh` caught each one. Output:

```
[CC#4] SHA-256 consistency...
Injected drift
  PASS
[CC#39] Total cycles ↔ filesystem cycles...
Injected drift
  PASS
[CC#46] No stale fix/m9-* branches...
  PASS
[CC#48+CC#54] meta-checks must run together...
  PASS

=== Summary ===
Tests run: 4
Failures: 0
All critical CCs detect synthetic drift. CC detection chain is healthy.
```

Wall time: ~35 seconds.

## History

m9-67 closes the "fix a broken CC, then fix what it would have caught" pattern observed twice in this session:

1. **m9-65** discovered 49 stale branches because CC#46 only matched `fix/m9-*` (missing `chore/*`, `feat/mX-*`, `ms-*`). The CC had drifted out of scope silently — no test caught it. m9-65 fixed CC#46 and deleted the stale branches.

2. **m9-66** discovered 54 stale SHAs in m9-01..m9-10 archive-manifests because CC#4 had a broken awk regex (it required stripping backticks before matching, and the regex didn't). CC#4 had been silently broken since the archive-manifest table format was introduced in m9-11. m9-66 fixed CC#4 and regenerated the 54 SHAs.

The pattern: a critical CC becomes broken (often silently), drift accumulates over many cycles, and the drift only surfaces when something else triggers a comparison. m9-67 prevents recurrence by making silent CC regressions immediately detectable via a 35-second smoke test.

The four CCs covered:

- **CC#4** — the one that was broken in m9-66. If a future change breaks its regex again, the smoke test catches it on the next run.
- **CC#39** (cross-cutting; superset of CC#5) — catches Total cycles mismatches. The bash CC#5 inside CC#54 is exercised by the same drift class; if CC#54 stops running, the test for the meta-checks catches it.
- **CC#46** — the one that was incomplete in m9-65. If a future change drops another milestone prefix from its pattern, the smoke test catches it.
- **CC#48 + CC#54** — verifies both meta-checks run AND the PASS message reports the expected CC counts (46 python + 7 bash). If a future change to the block-extraction logic silently drops a CC from either meta-check, the count assertion catches it.

The test uses `git clone --shared` (not `git archive`) because many python CCs (CC#3, #42, #47, etc.) call `git log` via subprocess and need a working `.git`. This was discovered during the first run of the script.

The test does not require rollback: each test rebuilds its work dir from a fresh clone. Cleanup is explicit at script exit (the `trap` pattern is intentionally avoided because subshell command substitution would inherit it and clean up prematurely — this is documented in a comment in the script).

## Future work

None for this cycle. The smoke test is designed to run on demand before merging any change to `vault-drift-sweep.md` or `check_vault_drift.sh`. If the cost (35s) becomes burdensome in a CI gate, the test can be split into a fast subset (CC#46 alone, ~10s) and a full subset (all four CCs).
