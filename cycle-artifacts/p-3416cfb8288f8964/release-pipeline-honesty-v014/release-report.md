# Release Report — release-pipeline-honesty-v014

- cycle_id: p-3416cfb8288f8964/release-pipeline-honesty-v014
- project_id: p-3416cfb8288f8964
- workspace_id: w-361237634265a0a7d986676e
- path: B-direct
- phase: release
- status: success
- route: local
- change: fix(release): gate linux-only capture code and make release workflows honest
- tag: v0.1.4
- head_sha: 98c4cd2341058872102f458655791ec53f21bd8c
- implementation_commit: 3d935b6a3d022f8ddc85b511e7dca414de010f2d
- work_item: a7695ce6-d1f4-4def-9a1d-5a5224cf79fc (Done)
- next_phase: archive
- optional_distribution: not_requested

## Gate receipts

| Gate | Receipt | Outcome | Basis |
|---|---|---|---|
| `no-pending-effects` | `gate-no-pending-effects-1e68ae303737c6f8-1` | passed | Local Git postconditions re-verified independently; 0 pending local effects. CI/CD, hosted releases, assets, signing and post-tag distribution are explicitly excluded by git-contract Rule 4/5 and are NOT claimed here. |
| `release-uat-approved` | `gate-release-uat-approved-1e68ae303737c6f8-2` | passed | `sddk uat gate release --tag v0.1.4 --release-type patch` returned `action=skip, approved=true`. Project `uat.toml` does not exist, so the documented default `[release_gate] patch = skip` policy applies. **This is a policy skip, not executed UAT.** |

Receipt `-2` supersedes `-1` for the UAT gate. The first evaluation carried a
placeholder `output_digest`; it was replaced with the real
`sha256:fd1e8c0652ad01c4a655c2e11fcf40ccb7b368a163ecbfb25a8cfd9bd9bd7b27`
computed from the actual command output. Recorded rather than hidden.

## Required artifacts

| Artifact | Path | sha256 |
|---|---|---|
| merge-receipt | `cycle-artifacts/p-3416cfb8288f8964/release-pipeline-honesty-v014/merge-receipt.md` | `a24c798932141ef0175e9bad4b65a7a5a4bde560a83a5cf9d40479c9a47d8a07` |
| release-receipt | `cycle-artifacts/p-3416cfb8288f8964/release-pipeline-honesty-v014/release-receipt.md` | `c68b9f75ab37a1c406b5e1239233ffc6b0eaf8fc600b4a63c1e9f63eb037d2e4` |
| verify-report | `cycle-artifacts/p-3416cfb8288f8964/release-pipeline-honesty-v014/verify-report.md` | `e534ca720d816fc356aa8c995e907905a386a63e7b5b796661a557704d6a06ba` |
| implementation-receipt | `cycle-artifacts/p-3416cfb8288f8964/release-pipeline-honesty-v014/implementation-receipt.json` | (pre-existing) |

## Git effects (all verified, none pending)

| Effect | Receipt | Result |
|---|---|---|
| `git.push` | `cap-git-push-f81988cbd29f` | `main` -> `98c4cd2341058872102f458655791ec53f21bd8c` |
| `git.tag` | `cap-git-tag-a3b00ed3b2c8` | annotated `v0.1.4`, peels to `98c4cd23...` |
| `sddk release apply` | — | `converged: true`, `skipped: []` |

Postconditions re-checked with local Git after a fresh fetch, not read back
from the capability response:

- `HEAD == origin/main == 98c4cd2341058872102f458655791ec53f21bd8c`
- `git cat-file -t v0.1.4` -> `tag` (annotated, not lightweight)
- `git rev-parse 'v0.1.4^{}'` -> `98c4cd2341058872102f458655791ec53f21bd8c`
- combined postcondition digest `sha256:d191749b3f6ba185e0813c2b1e17777ffdb1ac77108feee03d77cd116617f631`

## Local CI gate (pipelinek v0.39.0)

Executed on the exact release SHA `98c4cd23`:

- runId `0b8662a3-0df8-4651-bca9-2d074cc05dd0`
- `Pipeline finished with SUCCESS`, exit 0, `RunFinished/success`, `diagnostics: []`
- 5/5 stages `success`; `execution_log_read_e2e` -> 2 passed / 0 failed / 0 ignored (33.43 s)
- `.pipeline.kts` sha256 `c53a8be46f69cb6b04339e168f117e2a83992ffddec3353276551eb9136ed42c` (no drift)

**Cache caveat, disclosed:** a first `--rerun`-less invocation returned
`SUCCESS` in 5.33 s while replaying a cached result whose journal contained no
`StepStarted` events and whose cached compilation key `9f26bb05...` was
byte-identical to the pre-v0.1.4 tree. That green was stale evidence and was
NOT used. The two runs cited above were forced with `--rerun` and genuinely
re-executed. The underlying cache-invalidation defect is recorded as new debt.

## Files Inventory

inventory-unavailable: cycle inventory artifact not built for this cycle
(`sddk cycle inventory` was not run; the repository is git-initialised, so this
is a missing artifact rather than a `git-not-initialized` condition). Carried
into `blockers[]` per the release phase contract.

## Honest limitations (not resolved by this release)

1. **macOS runner unverified.** The push makes the macOS CI leg runnable, but
   no macOS runner result is observed. Local evidence is a Linux -> darwin
   cross-`cc` `cargo check`/`clippy`. Not claimed as a green macOS build.
2. **Tag-triggered workflows not awaited.** Per git-contract Rule 5, the
   Release/Docker workflows that `v0.1.4` triggers are external integration
   status, not cycle status.
3. **Tag is annotated but unsigned.** GPG signing is unavailable in this
   environment (consistent with ROADMAP §0.3).
4. **UAT was skipped by policy, not executed.** See the gate table above.
5. **pipelinek cache staleness is open debt**, not fixed here.
6. Pre-existing out-of-scope items from `verify-report.md` remain open: the
   `chronos-native --lib` parallel-mode hang (serial mode is the supported
   invocation), and `CapDockerfileRefresh` stays OPEN.
