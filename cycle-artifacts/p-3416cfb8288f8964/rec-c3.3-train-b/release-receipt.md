# Release Receipt — rec-c3.3-train-b (REC-C3.3.3 Tren B)

- cycle_id: rec-c3.3-train-b
- milestone: REC-C3.3.3
- path: A-min
- head_sha: a1c628e6c5f31ba3d224d93461781ce828ba0b91 (merge commit of PR #33)
- slice_f_capture_session_commit: d439fb6e5595ca13d2b94333350aff9a84b2e90f
- base_commit: f9ce02fb (pre-pipelinek-slice base per task instruction; git-side lineage: fa5eb582 → … → a1c628e6)
- tag_name: rec-c3.3-train-b.0
- tag_sha: rec-c3.3-train-b.0 (annotated; peel `^{}` = a1c628e6c5f31ba3d224d93461781ce828ba0b91)
- tag_peel_match: true
- tag_pushed: true (origin, 2026-09-24)
- ci_gates_pass_count: 5/5 on PR #33 (runs arch 359662390467..359662390998: deferred_debt_verification, reconstruction_contracts, ci_test, coverage, cargo_deny_audit_cyclonedx); PR state MERGED
- local_gate: pipelinek `Pipeline finished with SUCCESS`, runId 3260a48d-7adf-4f08-b5f9-507d0ff1c408 (executed this session on a1c628e6, exit 0, RunFinished/success in journal)
- local_tests_post_merge: 16 lib suites OK / 0 FAILED (51s, exit 0) — evidence recorded at apply closure; CI ci_test pass covers full battery
- release_at: 2026-09-24T08:37:00Z
- releaser: jcode-orchestrator (glm-5-turbo executor for capture_session slice; sddk-release executor this session)

## Canonical SHA fields (m9-28+ format, CC#22)

| Field | Value |
|---|---|
| Cycle | rec-c3.3-train-b |
| Head SHA | `a1c628e6c5f31ba3d224d93461781ce828ba0b91` |
| Remote tag | `rec-c3.3-train-b.0` |
| Remote tag_peel | `a1c628e6c5f31ba3d224d93461781ce828ba0b91` |
| Peel match | `true` |

## Notes
- Tag scheme chosen `<cycle>.<N>` matching repo pattern (m4-f1-closed.0, m6-otel-correlation.0, h1-quality-debt.0, …).
- HEAD == origin/main verified before tag (no scope_drift).
- Receipt commit follows this file; docs-only change on main.
