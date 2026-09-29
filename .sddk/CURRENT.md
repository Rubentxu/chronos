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
| `rec-c3-3-1` | `CLOSED` | **Already archived** — manifest `status: CLOSED`, closed 2026-09-18. Earlier sessions wrongly listed it as blocked. |
| `m9-04`, `m9-62`, `m9-63`, `m9-64`, `m9-80`, `v014` | **No record at all** | `cycle status` → `STORAGE_NOT_FOUND`; `cycle next` → "no replayable state events". Genuine released work, **zero ledger events**. Not *blocked* — never recorded. |
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
| 1 | **Knowledge ingestion defect (upstream)** | **Not** "180 orphans needing a hand-fix." `owner` **is** a declared field in the plan schema — `null` for all 180, `reason` for all 180. The vault is **not empty** (163 files: 17 adrs / 37 specs / 77 cycles). **14 of 43 adr candidates are already in the vault** yet all carry `existing_entry_id: null` + `disposition: quarantine`; dedup is populated 0× in all 4 plans. The 0011 collision has already reached the vault (1 there, 3 in repo). Importing blind would duplicate 14 ADRs and add two colliding `0011`s to a **Human-only** surface. Decision needed: why is `owner` never populated and why does dedup never fire? |
| 2 | **Vault blocks the archive sub-step** | `on_failure` really is absent (0 hits in all 4 `prompts/sddk/workflows/*.yaml`) **but a route exists** — each declares `failure_modes` with explicit condition/action pairs. The mechanism: archive's gate is a conjunction and `finalize-knowledge-graph` is one of its sub-steps, so the Human-only vault refusal fails archive in place. **`rec-c3-3-1` is already `CLOSED`**; all 9 archive manifests are CLOSED. **Correction:** `m9-80`/`v014` are *not* blocked by this — they have no cycle record at all (see 4a). |
| 3 | **No project `uat.toml`** | `~/.local/share/sddk/projects/p-3416cfb8288f8964/uat.toml` does not exist; defaults apply (`minor = required`, `developer`+`architect`). Writing it changes the release gate project-wide. |
| 4 | **SDDK escalation-policy divergence (4a)** | 1,203 repo-local SDDK artifact files (909 `cycle-artifacts/` + 294 `.sddk-knowledge/`) where `escalation-policy.md` places them in user/XDG space. **Also SIX unreplayable cycles** (zero ledger events) with genuine released work: `m9-04`, `m9-62`, `m9-63`, `m9-64`, `m9-80`, `v014`. **`rebuild` is tested and cannot fix them — circular:** `rebuild` needs a lease, `lock acquire` needs the cycle row to exist. Only `cycle start` would work, and that creates a *new* record, not a reconstruction. Human decision: accept retroactive `cycle start`, or accept permanent absence. |
| 4b | **Project ADR numbering — NOT an SDDK blocker** | 7 numbers duplicated across 16 files; 187 of 961 citations ambiguous. SDDK has no ADR surface and no SDDK artifact depends on these numbers (19 of 20 apparent citations were my own notes). **Docs hygiene; blocks no cycle.** Do not file next to the items that block archiving. |
| 5 | **m9-81 unsound design node** | Recorded at ledger seq 8; no `revert`, and `cycle.supersede` is not in its frontier. Route field says `A-full` while every artifact says `B-direct`; no subcommand amends a path. |
| 6 | **`sddk lint`: 3 pack errors** | `SDDK005` (no `schemas/`), `SDDK009`, `SDDK014` (no `manifest.toml`). **All three turn on one question: is this project an SDDK pack?** If not, suppress as N/A. `SDDK009`'s prescribed fix is **inapplicable here** — `sddk generate docs` produces a file that self-describes as generated from `workflow/workflow.yaml`, which this repo does not have; plain `--check` validates the *framework* default (current) while `sddk lint` checks the *in-repo* one (absent). **Do not commit that file** — it would claim a provenance the repo lacks. |
| 7 | **Tracked `.bak` in the artifact tree** | `cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/apply-checkpoint.json.bak` is tracked, though **0** `.bak` files are tracked repo-wide. The artifact contract does not generate backups. Not SDDK-actionable; left alone. |

## Next concrete action

1. Human repairs the vault as a `Human` actor, **and** upstream adds `on_failure` targets
   (blockers 1 and 2 are separate fixes; neither alone unblocks the three archives).
2. Human writes `uat.toml` (blocker 3) → m10-readpath can close.
3. Human rules on ADR-0011 divergence and the 0011 numbering collision (blocker 4).
4. Human voids or repairs m9-81's ledger sequence 8 (blocker 5).

## Do not

- Do not move, recreate, or re-point `v0.1.4`. It peels to `98c4cd23`.
- Do not report **`m9-80` or `v014`** as CLOSED/archived. Neither has an archive
  directory, and neither has a cycle record at all (see the cycle table). The
  vault is **not** the cause. (`rec-c3-3-1` *is* CLOSED; its manifest records it.)
- **Never use `sddk cycle narrative` as evidence.** It renders `Cycle completed`
  for *any* input — a fabricated `zzz-not-a-real-cycle` also reports completion.
  It is a static template. Use `cycle status` / `cycle next` instead.
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
