# Release report — sandbox-stale-binary-dependency-closure

cycle_id: p-3416cfb8288f8964/sandbox-stale-binary-dependency-closure
WorkItem: efcfafa5-ec00-4a1c-8d8f-ee92392a4b96
path: B-direct
route: local
status: success

main_sha: 5838027fc92014c642b7baec7add4a72a67574e3
tag: v0.1.2 (annotated, tag object df161d1a0c10f35491bc51976d47edf7494f397d, peels to main_sha)
main_synced: true (origin/main == main_sha, ls-remote verified after push)

## Git effects (typed capability receipts)

- git.push: cap-git-push-569bfc5bbfe1 → {branch: main, sha: 5838027fc92014c642b7baec7add4a72a67574e3}
- git.tag:  cap-git-tag-925fd0fc702a → {tag: v0.1.2, annotated: true, sha: 5838027fc92014c642b7baec7add4a72a67574e3}
- sddk release apply converged: true

## Commits included (c1712133..5838027f)

- e7c43064 fix(sandbox): derive the stale-binary watch set from the resolved dependency closure (the WorkItem change, SDDK-aligned and semantically closed)
- 1ba6835c chore(sddk): add default-deny agent permission registry for governed release path (permissions.yaml required by sddk release apply)
- 5838027f chore(release): bump workspace version 0.1.1 -> 0.1.2 (semver lockstep; v0.1.1 was already a published historical tag peeling to 33b4f790, rec-c3-1; retagging forbidden by contract)

## Receipts

- merge-receipt: cycle-artifacts/p-3416cfb8288f8964/rec-c3.2-stale-binary-closure/merge-receipt.json (artifact art-9ff7160afcba-4c42a741)
- release-receipt: cycle-artifacts/p-3416cfb8288f8964/rec-c3.2-stale-binary-closure/release-receipt.json (artifact art-53f84f6c6893-b4bc4737)

## Gates (release.complete)

- no-pending-effects: passed → gate-no-pending-effects-7e069ec44682b4fc-1
  Evidence: trunk push complete (origin/main == main_sha), annotated remote tag v0.1.2 verified peeling to main_sha; CI/CD, forge releases, assets and signatures explicitly excluded per contract.
- release-uat-approved: waived → gate-release-uat-approved-7e069ec44682b4fc-1
  Evidence: B-direct hotfix without UAT scope, consistent with all 67 prior waived receipts of this project's release.complete closures.

## Runtime transition

- release.complete: succeeded; status RELEASED; phase archive; event evt-4cf11c00-cc31-419a-89e4-1b3ece4e7848 (sequence 6)
- ledger verified after transition: 388 events, chain intact (last_hash 524d744e...)

## Incidents and recovery

1. First release attempt refused by dirty-worktree precheck caused by pre-existing untracked files; set aside under /tmp/chronos-release-stash and restored byte-identical after the release.
2. Tag v0.1.1 postcondition failed: remote tag already existed peeling to historical commit 33b4f790 (rec-c3-1). No retag performed (contract forbids). Root cause: workspace version frozen at 0.1.1 since rec-c3-1. Recovery: real patch bump to 0.1.2 (Cargo.toml + Cargo.lock via cargo metadata, rc=0) and release retried with v0.1.2.
3. permissions.yaml (default-deny registry, SDDK011) was missing; created with grants only for sddk-release and committed, because the release CLI hard-requires it.
4. A gate named release-receipt is not registered in this project's workflow (per historical receipts); the canonical release.complete gate pair for B-direct here is no-pending-effects (passed) + release-uat-approved (waived).

## Files Inventory

inventory: unavailable in this release report invocation (sddk cycle inventory not run for this cycle; no blocking unavailable_reason to declare: repository and rev valid). Follow-up: archive phase persists its own inventory per its contract.

## Verification binding

The released SHA contains the exact change verified by:
- verification-report art-7dc4f4671abf-4f513819 (rec_c2_2 9/9 green; stale refusal observed and cleared after rebuild; pipelinek 7141cde8 SUCCESS)
- gate receipts: gate-tests-pass-32f9ecb67430b0a9-1, gate-policy-compliant-32f9ecb67430b0a9-1 (transition phase.verify.complete.b-direct)

blockers: []
optional_distribution: not_requested
next_phase: archive (consumes release-receipt, emits archive-manifest)
