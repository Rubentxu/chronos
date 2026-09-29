# Archive Manifest — m9-80-property-policy-ownership

**Status**: `BLOCKED` — the cycle is fully released and verified, but
`archive.complete` cannot be honestly closed.

## Release state (observed)

| Field | Value | How verified |
|---|---|---|
| Merge commit | `7874e5c8e972172c024b07f60746f0e06df92f9d` | ancestor of `main` |
| Remote tag | `v0.7.82` | `git ls-remote --tags origin` |
| Annotated tag object | `c77333500da4ebb8f46e3b85cb0f9bfbf7bd4bd5` | `git ls-remote` |
| Tag peel | `7874e5c8e972172c024b07f60746f0e06df92f9d` | equals merge commit exactly |
| Implementation commit | `ccf8811` — "m9-80: make observe_property_target public + final verification (T5)" | in `main` |
| Cycle diff | `82e219f..ccf8811`, 18 files, +1000/-450 | `git diff --stat` |

## Verification evidence

| Check | Result |
|---|---|
| `cargo test -p chronos-domain -p chronos-services` | 964 passed, 0 failed, 0 ignored across 22 suites, exit 0 |
| `cargo fmt --check` | exit 0 |
| `cargo clippy -p chronos-domain -p chronos-services --all-targets -- -D warnings` | exit 0 |
| Targeted: `cargo test -p chronos-domain --lib property` | 25 passed, 0 failed |
| Targeted: `cargo test -p chronos-services --lib hypothesis` | 22 passed, 0 failed |
| `sddk ledger verify` | valid, 435 events |
| `verify-findings.json` verdict | `pass` |

## Gates satisfied

- `implementation-complete` — `gate-implementation-complete-0d59684c60477b6f-4`
- `tests-pass` — `gate-tests-pass-f0782ea44e4b8242-1`
- `policy-compliant` — `gate-policy-compliant-f0782ea44e4b8242-1`
- `debt-severity-assigned` — `gate-debt-severity-assigned-f0782ea44e4b8242-1`
- `debt-priority-assigned` — `gate-debt-priority-assigned-f0782ea44e4b8242-1`
- `no-pending-effects` — `gate-no-pending-effects-e436f7b5a2e819c6-1`
- `release-uat-approved` — `gate-release-uat-approved-e436f7b5a2e819c6-1`
- `ledger-valid` — `gate-ledger-valid-bce384455e0b56f9-1`

## Blocking gate

`vault-index-current` — recorded **failed**
(`gate-vault-index-current-bce384455e0b56f9-2`).

Observed via `sddk knowledge verify --root . --scope .`:

```
registry_present: false
valid: false
entries: 0
untracked: 178
incidences: 0
```

sha256 of that output: `0edf4c36d54fbd8bc84a4b7919f2cb34dd37c22ebb97bc53bb286a003afe6a86`

This is **cycle-independent**. It is the same shared blocker that keeps `v014`
at `RELEASED/archive`: the knowledge vault has no registry and 178 repository
documents are untracked. Repairing it is a vault mutation, which the default-deny
permission registry does not authorize me to perform.

## Correction log

An earlier `vault-index-current` receipt recorded in this session
(`gate-vault-index-current-bce384455e0b56f9-1`) carried an invented
`output_digest`. Receipt `-2` supersedes it with the real digest of
`sddk knowledge verify`. The `failed` outcome was correct in both; only the
digest was fabricated.

## Debt carried forward

`m980-D1` (medium/P3): `compare_ord` is a 361-line function at
`crates/chronos-domain/src/property.rs:245-605`.
`m980-D2` (low/P4): one production `unwrap()` at line 432.
Neither blocks release; see `debt-ledger.md`.
