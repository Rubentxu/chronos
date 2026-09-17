# Release Report: rec-c2.2-accepted-raw-seam

## Verification chain

| Stage | Verdict | Subject SHA | Artifact |
|---|---|---|---|
| verify | **PASS** | `1619fb25` | `verify-report.md` (sha256 `69f4567cf6d21b45a2029de4e003f6a7f5d4e03e8205886cb77a3a1bbe623317`) |
| debt-verify | **PASS_WITH_WARNINGS** | `1619fb25` | `debt-report.json` (sha256 `93c616aa81ee52d501af11cca95c898be566c00a1c3152acf59030282805c649`) |

Debt gate coverage: all 4 required A-lite clusters (coupling, overeng, smells,
duplication) completed, `failed_clusters` empty, `fail_closed: true`.
8 findings: 0 critical, 0 high, 1 medium (pre-existing, owned by FIND-C2.2-04),
7 low.

## Publication

| Fact | Value |
|---|---|
| Branch | `feat/rec-c2.2-accepted-raw-seam` |
| Base | `e4fd938c` |
| Merge | fast-forward into `main` |
| **main SHA** | `f02ab311` |
| **origin/main SHA** | `f02ab311` (verified equal after push) |
| Annotated tag | `rec-c2.2-accepted-raw-seam` |
| Tag object | `4b14bb3e` (annotated: object != peel) |
| Local peel | `f02ab311` |
| Remote peel | `f02ab311` (verified with `git ls-remote origin 'refs/tags/...^{}'`) |
| Worktree at push | clean |

## The subject SHA is an ancestor of the release SHA, on purpose

`verify-report.md` and `debt-report.json` are bound to `1619fb25`, while `main`
is at `f02ab311`. This is stated rather than smoothed over. The intervening
commits are:

```text
2b86d3ec  chore: checkpoint commits + FIND-C2.2-04/05
1619fb25  fix: EbpfAdapter::read_since refuses instead of evicting (FIND-C2.2-06)
4985c9bc  chore: verify + debt artifacts
f02ab311  chore: adopt the phase's final debt artifacts and verified hashes
```

`1619fb25` is itself the fix that the first verify round's answer produced, and
the rebind of the verify report to it was done by the verify phase (8 commands
re-run on that HEAD).

The delta `1619fb25..f02ab311` is **artifacts only**, and this is checkable rather
than asserted:

```bash
git diff --name-only 1619fb25 f02ab311 | grep -v '^cycle-artifacts/'
# (empty)
```

No `crates/`, `chronos-sandbox/`, `scripts/` or `legacy-evb-inventory.json`
change exists in that range, so the code at `f02ab311` is byte-identical to the
code the gates verified. The release gate (fmt, clippy `-D warnings`, ratchet
default + `--strict`, architecture contracts, and the three sandbox suites 9/9)
was additionally re-run on the release SHA itself.

## Ratchet

```text
production uses: 28 -> 21
CANONICAL destructive reads: 6 -> 0
check_legacy_evb.py           PASSED (21 tracked, baseline 28, CANONICAL=0)
check_legacy_evb.py --strict  PASSED   <- the REC-C2 close gate
```

Every decrement came from a real disappearance. The ratchet failed with a
`stale waiver` on each entry *before* the entry was removed, so it was the tool
that proved the deletion, not the commit message.
