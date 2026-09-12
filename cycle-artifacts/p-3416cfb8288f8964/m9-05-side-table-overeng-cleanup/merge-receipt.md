# Merge Receipt: m9-05-side-table-overeng-cleanup

## Subject

| Field | Value |
|---|---|
| Cycle | p-3416cfb8288f8964/m9-05-side-table-overeng-cleanup |
| Branch | main |
| Main SHA | 07d01d5869ff6e3ffed29315b761e6e476f3b70d |
| Base SHA | c9e541750e39458b2b28c5d19ad0ca11d0e73d5b |
| Route | local |
| At | 2026-09-12T08:39:51Z |

## Evidence

```
$ git checkout main
$ git merge --ff-only feat/m9-05-side-table-overeng-cleanup
Fast-forward (no commit created; -m option ignored)
 crates/chronos-cli/tests/replay_integration.rs                              |  48 ++--
 crates/chronos-store/src/counterexample_storage.rs                          | 269 ++++++++++++++++-----
 crates/chronos-store/src/storage.rs                                         |   9 +-
 .../m9-05-side-table-overeng-cleanup/verify-report.md                        |  90 ++++++++++++
 .../m9-05-side-table-overeng-cleanup-scoping.md                              | 108 +++++++++++
 5 files changed, 426 insertions(+), 98 deletions(-)

$ git push origin main
To github.com:Rubentxu/chronos.git
   c9e5417..07d01d5  main -> main

$ git fetch origin main && test "$SHA" = "$(git rev-parse origin/main)"
VERIFIED: HEAD == origin/main == 07d01d5869ff6e3ffed29315b761e6e476f3b70d
```

## Receipt

- `git.push` capability: direct push to `origin/main`
- Full local HEAD SHA confirmed at `origin/main` after push
- Merge: fast-forward from base `c9e5417` to head `07d01d5`
- Tree: clean (`git status --porcelain` shows only `.jcode/` which is gitignored)