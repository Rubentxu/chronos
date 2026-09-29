# CURRENT.md — operating pointer

Last updated: 2026-09-29, end of session 28. Authority for cycle state is `sddk cycle status`, not this file.
Full chronology: `.sddk/CLOSE-OUT-2026-09-29.md` (2,096 lines, 28 entries).

## Goal / milestone

Release-pipeline honesty (M10-era roadmap, all 10 chapters CLOSED; no feature work queued).
`v0.1.4` is published. The remaining work is **reconciling SDDK cycle state against Git and
fresh evidence** — not feature work.

## Last verified state

- `HEAD == origin/main` = `a9cc5b76`, working tree clean.
- Ledger valid: **465 events**, `sha256:6381bc82021a0d117bdcce8e21d73eaa55a905f32d290e8033db4fa080770cb7`.
- `v0.1.4` re-verified immutable: peels to `98c4cd2341058872102f458655791ec53f21bd8c`. **Never moved or recreated.**
- `pipelinek` PASS with `--rerun` (run `baefb5fc`): 9 `StepStarted` / 9 `StepFinished` /
  5 `StageFinished`, `RunFinished: success`, failure kinds **NONE**. Also PASS without `--rerun`
  (run `d8cbe899`, 9 `StepStarted`) — so neither was a vacuous success this time.
- `.pipeline.kts` SHA-256 `c53a8be46f69cb6b04339e168f117e2a83992ffddec3353276551eb9136ed42c`.

## Cycle state (re-derived, not copied)

| Cycle | State | Why it is not advancing |
|---|---|---|
| `v014`, `m9-80`, `rec-c3-3-1` | `RELEASED/archive` | vault gate fails; `on_failure` absent from 2.2.27 |
| `rec-c1-5-closure` | `RELEASE_PENDING` | merge/release receipts genuinely absent |
| `m10-readpath-production-entry` | `RELEASE_PENDING` | **no `uat.toml`**, so defaults require signatures |
| `m9-81-counterexample-table-classifier` | `OPEN/Plan` | design node rests on a substitution that fails the design MUST; no CLI reversal |
| `m5-preflight-clippy-drift-cleanup` | `PAUSED` | deliberate `pause-receipt.json`, `reason: context_switch` |
| `train-b`, `m9-88-find-m9-81-fk-investigation` | `BLOCKED` | recorded this session with real evidence |
| `m9-04`, `m9-62`, `m9-63`, `m9-64` | `BLOCKED` | released and verified, but **zero ledger events** → unreplayable |

## Corrections made this session (all in the close-out)

- **m9-81 design substitution was unsound.** The design phase MUSTs "every decision MUST have a
  rationale"; `spec.md` has **0** rationale markers. `sddk cycle verify-references` reports it
  `aligned` because it validates the CAS chain, not artifact legitimacy. **A green check is not
  evidence of a sound artifact.** The transition cannot be reversed (no undo in the frontier).
- **The ledger understates `main`.** 26 merged feature branches, exactly **16 have no cycle
  record** (10 verified as exact slug matches, 12 with zero distinctive-token hits, 4 hand-checked
  as different subjects) — including the whole m9-84…m9-87 CC001 arc. Exact, not a bound.
- **Artifact split is 96, not 8.** 104 repo-tree cycle directories, 8 with an XDG counterpart.
- **ADR-0011 makes XDG canonical**, so the repo tree is the violation: **1,203 committed files**
  (909 `cycle-artifacts/`, 294 `.sddk-knowledge/`), un-ignored, since m9-01. Note the repo's own
  ADR-0011 is a *different* decision — three accepted ADRs share that number.

## Open blockers — all Human-authority

| # | Blocker | Why an agent cannot clear it |
|---|---|---|
| 1 | **Knowledge vault** — 180 quarantined sources, all `"source has no declared owner"` | `knowledge import` refuses: `actor_kind System not admitted … (admitted: [Human])`. It is a **Human-only surface**, not a grantable capability. Sources are SDDK's *own* archived manifests (115 manifest / 43 adr / 10 roadmap / 9 spec / 2 context). Owner format is **undocumented** — no `owner:` field exists anywhere in the vault or framework. |
| 2 | **`on_failure` absent from the entire 2.2.27 bundle** | Upstream defect. A failed gate has no route, which is why v014/m9-80/rec-c3-3-1 cannot archive. |
| 3 | **No project `uat.toml`** | `~/.local/share/sddk/projects/p-3416cfb8288f8964/uat.toml` does not exist; defaults apply (`minor = required`, `developer`+`architect`). Writing it changes the release gate project-wide. |
| 4 | **ADR-0011 divergence** | 1,203 committed SDDK files; 4 cycles unreplayable. Plus an ADR numbering defect: **7 numbers duplicated across 16 files** (0007–0013), every collision pairing an `m4*` milestone ADR with a topical one — two series numbered independently. All `Accepted`, all different subjects. **187 of 961 repo-wide citations are bare** and therefore ambiguous; `ADR-0010` alone is cited 53×. Spans ≥2 directories. Renumbering needs someone who knows which series is canonical. |
| 5 | **m9-81 unsound design node** | Recorded at ledger seq 8; no `revert`, and `cycle.supersede` is not in its frontier. Route field says `A-full` while every artifact says `B-direct`; no subcommand amends a path. |
| 6 | **`sddk lint`: 3 pack errors** | `SDDK005` (no `schemas/`), `SDDK009`, `SDDK014` (no `manifest.toml`). **All three turn on one question: is this project an SDDK pack?** If not, suppress as N/A. `SDDK009`'s prescribed fix is **inapplicable here** — `sddk generate docs` produces a file that self-describes as generated from `workflow/workflow.yaml`, which this repo does not have; plain `--check` validates the *framework* default (current) while `sddk lint` checks the *in-repo* one (absent). **Do not commit that file** — it would claim a provenance the repo lacks. |

## Next concrete action

1. Human repairs the vault as a `Human` actor, **and** upstream adds `on_failure` targets
   (blockers 1 and 2 are separate fixes; neither alone unblocks the three archives).
2. Human writes `uat.toml` (blocker 3) → m10-readpath can close.
3. Human rules on ADR-0011 divergence and the 0011 numbering collision (blocker 4).
4. Human voids or repairs m9-81's ledger sequence 8 (blocker 5).

## Do not

- Do not move, recreate, or re-point `v0.1.4`. It peels to `98c4cd23`.
- Do not report v014, m9-80 or rec-c3-3-1 as CLOSED/archived. The `vault-index-current` gate
  **failed**; closing would require a pass never observed.
- Do not delete the git-tracked `.sddk-knowledge/` copy. `vault-drift.yml` triggers on it and
  `check_vault_drift.sh` reads it — removing it disables the only automated drift protection.
- Do not trust a `pipelinek` SUCCESS without checking for real `StepStarted` events in the
  journal. A cached run can report SUCCESS while executing nothing.
- Do not read `Pipeline finished with SUCCESS` as "the workspace is green". `test-sandbox-read-path`
  is the only test stage and runs **2** E2E tests. It is not a full-workspace gate.
- Do not pass an existing document as a differently-named artifact. The engine accepts it and
  `verify-references` calls it `aligned`; only the phase contract catches the substitution.
- Do not enumerate cycles from the ledger export alone — the snapshot store holds cycles with
  **zero** ledger events, and that census missed three `OPEN` cycles this session.
- Do not report `release-uat-approved` as executed UAT. No `uat.toml` exists, so it was never run.
- Do not read a path-scoped `git log -1 -- <path>` as a full-history log; it returns the newest
  commit *touching that path*, which may be an older prior-session commit.
- Do not ship a mechanism for a failure without checking the logs for that mechanism's own marker.
- Do not touch the three stashes — they are not mine (`cih-d-stash-non-mine`,
  `CIH-C.1 unstashed`, `WIP on rec-c3-ci-hygiene`). Verified intact 2026-09-29.
- Do not sign an absent `capture_session` without first checking the
  documentation/code mismatch. Carried from the superseded
  `.sddk/CURRENT-CHECKPOINT.md` (2026-09-24), which is retained as history only.
- Do not mutate a source file while a suite runs against the binary built from it. Doing so
  produced 3 `analytics_tools` failures that were the staleness guard working correctly, not a
  regression, and it invalidated the run.
- Do not re-run the E2E suite and call it green without building `chronos-mcp` first, and do not
  share one MCP server across `#[tokio::test]` functions — each test needs its own.
- Do not report the full sandbox suite as green from a **truncated** run; verify it reached its end.
