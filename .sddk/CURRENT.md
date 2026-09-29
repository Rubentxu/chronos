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
| `m9-04`, `m9-62`, `m9-63`, `m9-64` | **Partially recorded** | **RETRACTED: my "no record / zero events" claim was wrong.** Real ids `m9-04-side-table-key-layout` (5 events), `m9-62-bounded-stop-probe`, `m9-63-stop-drain-cc`, `m9-64-vault-drift-ci` (3 each). All have events **and** artifact dirs. They lack only the `cycle.created` event. |
| `m9-80-property-policy-ownership`, `release-pipeline-honesty-v014` | **Fully recorded** | **RETRACTED: my "no cycle record" claim was wrong.** 19 and 8 events, both with `cycle.created`. `m9-80` has *more* events than the genuinely-`CLOSED` `rec-c3-3-1` (14). I searched the bare strings `m9-80`/`v014` as if they were cycle ids — they are prefixes. |
| `rec-c1-5-closure` | `RELEASE_PENDING` | merge/release receipts genuinely absent |
| `m10-readpath-production-entry` | `RELEASE_PENDING` | **no `uat.toml`**, so defaults require signatures. **Corrected:** it *does* have a `cycle.created` event — my "no cycle record" note was wrong. Only the UAT/certification gap (blocker 3) stands. |
| `m9-81-counterexample-table-classifier` | `CLOSED` | **Verified**: tag `v0.7.83` peels to `fdc5accf…` on `origin/main`. No design phase required (B-direct). Not in the ledger → 4a. |
| `m5-preflight-clippy-drift-cleanup` | **NOT A CYCLE** | **RETRACTED: my "`PAUSED` with a genuine context-switch receipt" was fabricated.** It is a **git branch** `chore/m5-preflight-clippy-drift-cleanup` (local + origin) and nothing else: 0 artifacts, 0 manifest, 0 `pause-receipt.json`, 0 pause/`context_switch` events in the 465-event ledger, 0 files declaring a paused status. **There is no `PAUSED` cycle in this project.** Do not "resume" it — there is nothing to resume. |
| `train-b`, `m9-88-find-m9-81-fk-investigation` | `BLOCKED` | recorded this session with real evidence |

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
| 1 | **Knowledge: 180 quarantines — DIAGNOSIS RESOLVED, only the label left** | **No Human needed to diagnose.** The rule is compiled into the binary and is **unconditional**: `source has owner and an unambiguous relation` — **no per-kind exemption**, so `incidence` was never special, just the kind we'd been populating. Fresh `knowledge scan --format json`: **180 candidates, all `quarantine`, all one reason — `source has no declared owner`**. **0/180 have `owner`; 180/180 lack it.** `existing_entry_id` is empty for all 180, so **this is not the dedup problem I'd suspected.** `relation` is **already populated on all 180** — so only `owner` is missing, and the fix is narrower than "owners for 180 docs" implied. **Kinds affected (wider than I'd recorded):** manifest 115, adr 43, roadmap 10, specification 9, repository_context 2, term 1. *My prior claim that disposition was `quarantined` was wrong — it's `quarantine`; my script filtered on the wrong value and I read the resulting zero as meaningful.* **Remaining question is narrow:** which role label — `orchestrator` (as on 27 incidences), `sddk-team`, or `release`? **Not written unilaterally:** assigning accountability for 180 records changes what a future `knowledge ingest` would register as authoritative, and touches the tree `vault-drift.yml` gates on. No `owner:` written, no `ingest` run, scan redirected to scratch — tree clean. |
| 2 | **Archive needs `delivery_kind`; NO cycle declares it** | **Mechanism corrected — it is not a permissions problem.** Archive's gate is `vault-receipt-verified` + `vault-index-current` + `release-bypass-declared`, and step 1 is `sddk release vault`, which **fails unless `delivery_kind == ManagedClosureDelivery`** is in the cycle manifest (and `cycle.status == BLOCKED`). `vault-index-current` is gate **3 of 3**, not the failure point. **Zero** cycles declare `delivery_kind` — **including all 9 genuinely `CLOSED`** — and **0** `vault-receipt` files exist, so this command has never succeeded here. The Human-only vault refusal may also apply but is **untested and not first-order**. Governing **ADR-0075 is absent from the bundle**. `sddk release vault` was **not run** (it emits a receipt + ledger append). |
| 3 | **`uat.toml` absent — but it is only HALF the blocker** | Verified independently: `sddk uat config show` → `exists: no (defaults shown)`, `major/minor = required`, `human: developer+architect`. **The cycle's own report gives two independent reasons** and I had been reporting only the weaker one: **(a) real UAT execution** is required and *explicitly not reachable with unit tests* — needs `sddk uat plan --release`, real agent sessions on `execution_log_read` over MCP, `uat ingest`, `uat report`. Writing `uat.toml` **cannot** substitute for this. **(b) two human signatures** — signing for an unreviewed human fabricates an approval. Merge itself is real: `f94864ad…` is a commit and an ancestor of `origin/main`. A Human could write the config and still be blocked. |
| 4 | **Repo vendors its SDDK state (historical, load-bearing)** | Policy **confirmed verbatim**: `escalation-policy.md` §"SDDK Artifacts Live in User Space (ADR-0011)" — "SDDK **never writes inside a project repo**", artifacts belong in XDG, and "**Never commit a repo-local SDDK working path** or copy vault knowledge into `docs/`". Yet **1,203 tracked files** (909 `cycle-artifacts/` + 294 `.sddk-knowledge/`), not ignored. **Corrected nature:** these were committed **by hand** in `docs(...)` commits from m9-01 (`25948147`, `48a9cff5`); **614 commits** touch them. **Not** a live tool bug — nothing violates the policy now. **Not cheap to fix:** removing them rewrites 614 commits, and `.github/workflows/vault-drift.yml` *reads* the repo copy (triggers on `changes/**`, `cycles/index.md`, `terms/index.md`), so deleting it kills the only automated drift protection. **ADR-0011 absent from bundle.** Also owns the **4 partially-recorded cycles** (`m9-04/62/63/64`, which lack only `cycle.created`) — *my "7 absent cycles" claim is RETRACTED: `m9-80` and `v014` are fully recorded (19 and 8 events) and `m9-81`/`m10-readpath` are real cycles too.* Human call on the four: retroactive `cycle start`, or permanent absence. |
| 4b | **Project ADR numbering — NOT an SDDK blocker** | 7 numbers duplicated across 16 files; 187 of 961 citations ambiguous. SDDK has no ADR surface and no SDDK artifact depends on these numbers (19 of 20 apparent citations were my own notes). **Docs hygiene; blocks no cycle.** Do not file next to the items that block archiving. |
| 5 | **`sddk lint`: 3 pack errors, exits 1** | Verified unpiped: `EXIT=1`, errors `SDDK005` (no `schemas/`), `SDDK009`, `SDDK014` (no `manifest.toml`). *(An earlier `EXIT=0` was `tail`'s status, not lint's.)* **All three turn on one question: is this project an SDDK pack?** If not, suppress as N/A. `SDDK009`'s prescribed fix is **inapplicable here** — `sddk generate docs` produces a file that self-describes as generated from `workflow/workflow.yaml`, which this repo does not have; plain `--check` validates the *framework* default (current) while `sddk lint` checks the *in-repo* one (absent). **Do not commit that file** — it would claim a provenance the repo lacks. |
| 6 | **Tracked `.bak` in the artifact tree** | `cycle-artifacts/p-3416cfb8288f8964/rec-c3.3-train-b/apply-checkpoint.json.bak` is tracked, though **0** `.bak` files are tracked repo-wide. The artifact contract does not generate backups. Not SDDK-actionable; left alone. |
| 7 | **Framework ships 0 ADRs and 0 arch-specs, but cites 22 of them** | Bundle has `adr-template.md` and **nothing else**. Cited: **11 ADR numbers** (`ADR-0011`×18 across 13 files, `0070`×8, `0047`×4, `0075`/`0096`/`0016`/`0003`×2, `0129`/`0020`/`0009`/`0006`×1) and **11 spec ids**, of which `arch-spec-049-sddk-configuration-model-v1` — cited by `prompts/sddk/orchestrator.md` as the home of config laws/precedence and told to agents to read — **is not shipped**. This is why blockers **1, 2 and 4a** cannot be evidenced: not SDDK misbehaving, but a **distribution defect upstream of this repo**. Highest-leverage single fix available; a Human call. *(My earlier "four absent docs" framing was loose: `on_failure` is **not** a gap — 0 occurrences; `failure_modes` ×11 is the real, present vocabulary.)* |
### WITHDRAWN (were listed as blockers, now disproved)

- **m9-81 "unsound design node" — WITHDRAWN.** `m9-81` is **CLOSED** with verified
  provenance: tag `v0.7.83` (annotated `e821718e`) peels exactly to merge SHA
  `fdc5accf64be1fcf780913243aec0496ad48e7fe`, an ancestor of `origin/main`; commits
  `80cca0d`/`a3f59ea`/`45b53df` all exist. There is **no** "design node at ledger
  seq 8" — m9-81 has **zero ledger events**. And `spec.md` needs no rationale markers
  because **B-direct has no design phase** (`load-skill → execute → branch-creation →
  verify → release → archive`). Only residue: absent from the ledger, which is 4a.
- **"rec-c3-3-1 archive failed" — WITHDRAWN** (session 35). Its manifest reads `CLOSED`.

## Next concrete action

1. Human repairs the vault as a `Human` actor, **and** upstream adds `on_failure` targets
   (blockers 1 and 2 are separate fixes; neither alone unblocks the three archives).
2. Human writes `uat.toml` (blocker 3) → m10-readpath can close.
3. Human rules on ADR-0011 divergence and the 0011 numbering collision (blocker 4).
4. Human voids or repairs m9-81's ledger sequence 8 (blocker 5).

## Do not

- Do not move, recreate, or re-point `v0.1.4`. It peels to `98c4cd23`.
- Do not report **`m9-80` or `v014`** as unrecorded — **my earlier "neither has a cycle
  record" claim is RETRACTED.** They are `m9-80-property-policy-ownership` (19
  events) and `release-pipeline-honesty-v014` (8), both with `cycle.created`.
  I had grepped the bare prefixes as if they were cycle ids. Neither is
  archived, which is a separate and accurate point.
- **Do not attribute an archive failure to a permission until you have read
  `prompts/sddk/phases/archive.md` §Procedure.** Step 1 is
  `sddk release vault`, gated on `delivery_kind == ManagedClosureDelivery`, which
  **no cycle here declares**. A YAML `sub_steps_owned_by_agent` list is not the
  gate.
- **Never use `sddk cycle narrative` as evidence.** It renders `Cycle completed`
  for *any* input — a fabricated `zzz-not-a-real-cycle` also reports completion.
  It is a static template. Use `cycle status` / `cycle next` instead.
- **Never read a whole-store aggregate as current state.** Three instances:
  `cycle narrative` (any name "completes"), `sddk workflow show` (nonexistent
  command, empty output read as "not declared"), and a whole-DB pipeline-journal
  scan returning `StepFailed: 1` that belongs to a **superseded 16:03 run** while
  the last run is clean. Always scope to the current run/cycle and run a control.
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
