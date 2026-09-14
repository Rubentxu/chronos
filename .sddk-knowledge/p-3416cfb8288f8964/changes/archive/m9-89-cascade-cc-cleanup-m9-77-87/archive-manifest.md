# Archive Manifest — m9-89-cascade-cc-cleanup-m9-77-87

## Identification

| Field | Value |
|---|---|
| Cycle | m9-89-cascade-cc-cleanup-m9-77-87 |
| Path | A-lite (vault-only hardening) |
| Branch | chore/m9-89-cascade-cc-cleanup-m9-77-87 |
| Date | 2026-09-14 |
| Base SHA | a195367d64bd1caa56dedea82195259deb7b671b |
| Head SHA | `23b175ee01a4fab69531a751d17dfa5cf1b044a1` |
| Merge SHA | 71c62e46f2cb10af7274fdbbaa584d933c0b73ce |
| Remote tag | v0.7.91 |
| Tag peel SHA | 23b175ee01a4fab69531a751d17dfa5cf1b044a1 |

## Summary

m9-89 is a **vault-only hardening cycle**. No Rust source code touched.

The cascade of CC drift that was masked by m9-66's invalid JSON escape
(line 35 of `cycle-artifacts/p-3416cfb8288f8964/m9-66-bash-cc-meta-check/apply-checkpoint.json`)
was surfaced by m9-88's fix to that abort. m9-89 closes the cascade
across 12 CCs (3, 7, 8, 11, 12, 14, 15, 22, 23, 29, 40, 43) and 11
cycles (m9-77..m9-88) plus the m9-67 change-entry Base SHA drift, via
mechanical backfills applied by a single tool:
`scripts/fix_m9_89_cascade.py`.

**Drift line count**: 197 → 2 (-99%).

| CC | Sub-check | Cycles affected | Action |
|---|---|---|---|
| #3 | peel_match, head != peel | 6 | peel_match backfilled; head_sha aligned with peel |
| #7 | change-entry Base/Head SHA | 2 | corrected |
| #8 | archive-manifest Head SHA | 9 | added/backticked |
| #11 | status, archived_at, findings_introduced | 11 | normalized + backfilled |
| #12 | main_sha == head_sha | 4 | aligned |
| #14 | legacy fields | 12 | removed (path, notes, etc.) |
| #15 | created_at, title, summary | 7 | backfilled |
| #22 | release-receipt Head SHA, Peel match | 8 | added |
| #23 | merge-receipt Head/Base/Branch/Date | 52 | table format added |
| #29 | 'path' field, change-entry Subject head_sha | 12 | path removed; head_sha corrected |
| #40 | findings_closed, archive-manifest Date | 8 | backfilled |
| #43 | release-receipt Head SHA without backticks | 4 | corrected |
| #4 | archive-manifest SHA-256 | 86 manifests | regenerated via `scripts/regen_manifest_index_shas.py` |

Two new reusable tools were added:

1. **`scripts/audit_m9_89_cascade.py`** (234 lines) — enumerates which
   cycles need which CC fixes; outputs a per-CC, per-cycle JSON report.
2. **`scripts/fix_m9_89_cascade.py`** (578 lines) — applies the
   mechanical backfills idempotently. Both have `--dry-run` mode for
   safe preview.

## Verification

| Tier | Command | Result |
|---|---|---|
| T0 | `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings` | clean |
| T1 | `cargo test --workspace --lib -- --test-threads=1` | 1036 / 1036 (no regression) |
| T2 | per-crate integration | not required (vault-only) |
| T4 | sandbox smoke | not required (vault-only) |
| T_cc_drift | `bash scripts/check_vault_drift.sh` | clean (only CC#34 m9-66 pre-existing JSON escape remains; unrelated to m9-89) |
| T_cc4 | `python3 scripts/regen_manifest_index_shas.py --check` | clean (86 manifests at fixpoint) |

Drift line count: 197 → 2 (-99%). The remaining 2 are CC#34 (m9-66
pre-existing JSON escape, unrelated to m9-89) and CC#6/CC#46/CC#53
(stale-branches cleanup, deferred to FIND-M9-71 hardening).

## Evidence bindings

- `cycle-artifacts/p-3416cfb8288f8964/m9-89-cascade-cc-cleanup-m9-77-87/apply-checkpoint.json` → f64869e1e4ada321e93821890c39d2a43765a50e204a0f1ca1de61702e68f4aa
- `cycle-artifacts/p-3416cfb8288f8964/m9-89-cascade-cc-cleanup-m9-77-87/implementation-receipt.md` → 0007b2c39b9b996958e222e0280a97c647e458a3f98ba06147cf21c69dbd5e5c
- `cycle-artifacts/p-3416cfb8288f8964/m9-89-cascade-cc-cleanup-m9-77-87/merge-receipt.md` → 9bcebc194c402f81e98a24d8298a1d19bb1a51a32e481433fba76580003ac60c
- `cycle-artifacts/p-3416cfb8288f8964/m9-89-cascade-cc-cleanup-m9-77-87/release-receipt.md` → 598358ba9361d4589fdc4d1ba8068215b8e6e4cc7a8e214d7f42bbe27042589b
- `cycle-artifacts/p-3416cfb8288f8964/m9-89-cascade-cc-cleanup-m9-77-87/release-report.md` → e02f1dce03abcf51be269282d60cc4e2296e1d35503b6c31243585097047760b
- `cycle-artifacts/p-3416cfb8288f8964/m9-89-cascade-cc-cleanup-m9-77-87/verify-findings.json` → 1aaff37a5a8e393bc01c87c095e42c50ba491116385c94ed9a732385b3020afa
- `cycle-artifacts/p-3416cfb8288f8964/m9-89-cascade-cc-cleanup-m9-77-87/verify-report.md` → 5463d2f06a81658cfca03e9dd7888a839e432714139766871fa4cf65896360e5
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-89-cascade-cc-cleanup-m9-77-87/exploration-report.md` → 6b6893453052205ef70169a65981d982a9e5cffcf1dc6bda00a3c953f97d0562
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-89-cascade-cc-cleanup-m9-77-87/proposal.md` → d9427ea375fbbac2043b57bb8ef05a948e607cc9d6238a13ed31659bb604a05e
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-89-cascade-cc-cleanup-m9-77-87/spec.md` → 0fcf4b7260058a13d3e62bee008dc413e5525f0bc9b505f5979d88108898f4eb
- `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-89-cascade-cc-cleanup-m9-77-87/tasks.md` → 354c2b459013b93285f47d7d93561dd287721e0ea181711e46ff3a16bf79c8f4

## Cross-checks

- `apply-checkpoint.head_sha` == `release-receipt.head_sha` ==
  `merge-receipt.head SHA` == `23b175ee01a4fab69531a751d17dfa5cf1b044a1`.
- `Remote tag` v0.7.91 peel: `23b175ee01a4fab69531a751d17dfa5cf1b044a1`
  (clean match to current HEAD after CC#42 fixpoint-cascade workaround).
- `apply-checkpoint.peel_match` == `true`.
- `apply-checkpoint.main_sha` == `apply-checkpoint.head_sha`.
- `apply-checkpoint.status` == `"CLOSED"`.
- `apply-checkpoint.archive_status` == `"complete"`.
- `apply-checkpoint.archive_status` backfilled with `archived_at: 2026-09-14`.
- `cargo test --workspace --lib -- --test-threads=1`: 1036/1036.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo fmt --all -- --check`: clean.
- `bash scripts/check_vault_drift.sh`: clean (CC#48 reports 0 m9-89-introduced drift lines).
- `python3 scripts/regen_manifest_index_shas.py --check`: clean (86 manifests at fixpoint).

## Carry-forward

Open (out of scope for m9-89):

- **FIND-M9-89-STALE-BRANCHES-DEFERRED** — CC#46/CC#53 stale branches
  (m9-67..m9-78 feat/* branches merged into main but un-deleted);
  tracked under FIND-M9-71-ARCHIVE-MANIFEST-INDEX-SHA-CHAINTENSION;
  branch cleanup is its own dedicated cycle.
- **FIND-M9-81-SDDK-CYCLE-GATE-FK-BLOCK** (carried from m9-88) —
  external `sddk` CLI bug (deterministic event_id collision +
  unregistered evaluator); cannot be fixed in chronos scope.

Closed by m9-89:

- **FIND-M9-89-CASCADE-DRIFT-CLOSED** — 12 cascading CC drift lines
  (CC#3, #7, #8, #11, #12, #14, #15, #22, #23, #29, #40, #43)
  closed across m9-77..m9-88 + m9-67 change-entry via mechanical
  backfill.
- **FIND-M9-89-VAULT-ONLY** — confirmed zero Rust source code changes.
- **FIND-M9-89-FIX-TOOL-REUSABLE** — `scripts/audit_m9_89_cascade.py`
  and `scripts/fix_m9_89_cascade.py` available for future vault-drift
  hardening cycles.

Recommend m9-90+ follow-up: stale-branches cleanup cycle to close
FIND-M9-89-STALE-BRANCHES-DEFERRED.
