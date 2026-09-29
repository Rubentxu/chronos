# Release Receipt — release-pipeline-honesty-v014 (v0.1.4)

- cycle_id: p-3416cfb8288f8964/release-pipeline-honesty-v014
- project_id: p-3416cfb8288f8964
- workspace_id: w-361237634265a0a7d986676e
- path: B-direct
- phase: release
- change: fix(release): gate linux-only capture code and make release workflows honest
- implementation_commit: 3d935b6a3d022f8ddc85b511e7dca414de010f2d
- version_bump_commit: 5842075e40885c16449c147647c3ec084483a344 (0.1.3 -> 0.1.4)
- release_commit: 98c4cd2341058872102f458655791ec53f21bd8c
- head_sha: 98c4cd2341058872102f458655791ec53f21bd8c
- base: main
- route: local (no forge integration)
- tag_name: v0.1.4
- tag_sha: v0.1.4 (annotated; tag object d617c17799d0b65ed6ad1af3e6694a8e98c79c1a)
- tag_peel: 98c4cd2341058872102f458655791ec53f21bd8c
- tag_peel_match: true
- tag_pushed: true (origin, 2026-09-29)
- git_push_receipt: cap-git-push-f81988cbd29f
- git_tag_receipt: cap-git-tag-a3b00ed3b2c8
- release_converged: true
- release_at: 2026-09-29T15:28:51Z
- releaser: jcode-orchestrator (sddk-release route)

## Canonical SHA fields (m9-28+ format, CC#22)

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/release-pipeline-honesty-v014 |
| Head SHA | `98c4cd2341058872102f458655791ec53f21bd8c` |
| Remote branch | `main` = `98c4cd2341058872102f458655791ec53f21bd8c` |
| Remote tag | `v0.1.4` (annotated, object `d617c17799d0b65ed6ad1af3e6694a8e98c79c1a`) |
| Remote tag_peel | `98c4cd2341058872102f458655791ec53f21bd8c` |
| Peel match | `true` |

## Postcondition verification (OBSERVED, independent of the capability receipt)

These were re-checked with local Git after the release, not copied from the
`sddk release apply` response:

| Check | Command | Observed |
|---|---|---|
| trunk convergence | `git rev-parse HEAD` vs `git rev-parse origin/main` after `git fetch origin main --tags` | both `98c4cd2341058872102f458655791ec53f21bd8c` -> MATCH |
| tag is annotated | `git cat-file -t "$(git rev-parse v0.1.4)"` | `tag` (not `commit`, so not lightweight) |
| tag peel | `git rev-parse 'v0.1.4^{}'` | `98c4cd2341058872102f458655791ec53f21bd8c` |
| remote refs | `git ls-remote origin main 'refs/tags/v0.1.4*'` | `98c4cd23...  refs/heads/main`; `d617c177...  refs/tags/v0.1.4`; `98c4cd23...  refs/tags/v0.1.4^{}` |

## Local CI gate (pipelinek v0.39.0, mandatory per AGENTS.md)

Canonical command:

```
pipelinek run --rerun --db .pipelinek/db.sqlite --control-root .pipelinek/control .pipeline.kts
```

| Run | runId | Tree | Result |
|---|---|---|---|
| `e20056c9-8e4d-4f82-8c47-c2544d5710ee` | genuine | `a744e8c0` (v0.1.4, pre-handoff-commit) | `Pipeline finished with SUCCESS`, exit 0, 5/5 stages success |
| `0b8662a3-0df8-4651-bca9-2d074cc05dd0` | genuine, **release SHA** | `98c4cd23` (v0.1.4) | `Pipeline finished with SUCCESS`, exit 0, 5/5 stages success |

The release SHA gate executed all 41 journal events: `CompilationStarted`,
5x `StageStarted`/`StageFinished(success)`, and `RunFinished(success)`, with
`diagnostics: []`. Real test execution, not a cache replay:
`execution_log_read_e2e` -> `2 passed; 0 failed; 0 ignored` in 33.43 s,
talking JSON-RPC over the wire to a freshly built `chronos-mcp`.

Script identity: `.pipeline.kts` sha256
`c53a8be46f69cb6b04339e168f117e2a83992ffddec3353276551eb9136ed42c`, unchanged
across this session (no drift, consistent with `git log -- .pipeline.kts`).

## Scope

Published commits on top of `origin/main` @ `18188695`:

| SHA | Subject |
|---|---|
| `3d935b6a` | fix(release): gate linux-only capture code and make release workflows honest |
| `5842075e` | chore(release): bump workspace version 0.1.3 -> 0.1.4 |
| `edd607a3` | docs(cycle): add release-pipeline-honesty-v014 implementation and verification artifacts |
| `a744e8c0` | docs(cycle): version the remaining historical cycle artifacts |
| `98c4cd23` | docs(session): version the rec-c3-ci-hygiene close handoff and ignore per-machine skill-registry state |

`98c4cd23` was authored this session because three untracked files made the
worktree dirty and `sddk release apply` refuses to release from a dirty
worktree. The three files were classified, not blanket-stashed:

- `.atl/.skill-registry.md` + `.atl/.skill-registry.cache.json` are generated
  by `gentle-ai skill-registry refresh` and embed 152 absolute
  `/home/rubentxu/...` paths, so they are per-machine scratch. Added to
  `.gitignore` alongside the already-ignored `.pipelinek/` and `.jcode/`.
- `session-handoff/CIH_H_HANDOFF_2026-09-19_close.md` is a genuine 151-line
  project record whose siblings are all tracked; committed rather than
  ignored, content preserved verbatim.

## Honest limitations (carried forward, not resolved by this release)

- **macOS runner not yet observed.** The verify report's only known unknown is
  that a real macOS runner execution is pending the push. This receipt makes
  the tag available; it does NOT claim the macOS leg is green. The local
  evidence is a Linux -> darwin cross-`cc` `cargo check`/`clippy`, which the
  prior report already classified as a local cross-compilation artifact.
- **Tag-triggered workflows are out of scope.** Per git-contract Rule 5, any
  Release/Docker workflow the `v0.1.4` tag triggers is external integration
  status, not cycle status. This receipt does not wait for, enable, or
  cancel them.
- **GPG signing unavailable in this environment.** Consistent with the
  pre-existing note in ROADMAP §0.3, the tag is annotated but unsigned.
- The pipelinek cache-staleness defect observed this session is tracked
  separately in `SESSION-JOURNAL` / debt and is NOT fixed by this release.
