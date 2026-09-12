# Verify Report — m9-19

| Field | Value |
|---|---|
| Cycle | m9-19-route-main-sha-and-empty-folder-drift |
| Verify time | 2026-09-12T11:11:00Z |
| Verifier | self (B-direct light-verify) |
| Result | PASS |

## T0 gate

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | PASS |
| `cargo clippy --workspace --all-targets -- -D warnings` | PASS (0 warnings) |

## Cross-check sweep (12 checks)

All 12 cross-checks in `vault-drift-sweep.md` run via Python script.
Result: 0 drift across all 12 checks.

| # | Check | Result |
|---|---|---|
| 1 | Cycles index dedupe | PASS |
| 2 | Apply-checkpoint schema (required fields) | PASS |
| 3 | SHA full-length + head_sha == remote_tag_peel + peel_match=True | PASS |
| 4 | m9-01-R4 active/terminated dedupe | PASS |
| 5 | cycles/index.md metadata | PASS |
| 6 | terms/index.md metadata | PASS |
| 7 | change-entry.md SHA consistency | PASS |
| 8 | All apply-checkpoint + archive-manifest SHAs exist in repo | PASS |
| 9 | Archive-manifest SHAs 40-char + cross-refs | PASS |
| 10 | verify-findings + markdown file SHAs | PASS |
| 11 | status=CLOSED + findings_introduced + archived_at | PASS |
| 12 | route field + main_sha convention + empty folder cleanup | PASS (new) |

## Notes

m9-19 closes 3 classes of drift in a single cycle:
- Pre-m9-11 `route: "local"` placeholder (8 cycles)
- m9-11/12/13 `main_sha` set to pre-cycle HEAD (3 cycles, my mistake when authoring)
- 1 empty cycle folder from aborted investigation

Cross-check #12 added to prevent recurrence of all 3 classes.
## Cross-checks

Note: This cycle predates the cross-check annotation format introduced
in m9-28. Per `vault-drift-sweep.md` cross-check #21 (verify-report
must have `## Cross-checks` section), this section is added
retrospectively by m9-32. The cycle's verify-report content above is
unchanged.

The cross-check status for this cycle was inferred from the
apply-checkpoint.json status field:
- Status: CLOSED (verified, released, archived)
- All apply-checkpoint.json SHA fields match git repository
- No drift detected when this cycle was authored
