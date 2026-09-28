# release-receipt — v0.1.3 (ci-vault-drift-verify-findings-restoration)

- Capability receipt: `cap-git-tag-ccdd6cb3a872` (issued by `sddk release apply --route local`)
- Tag: `v0.1.3` (annotated object `d6413d038a17988f2e75738daf52ba29140272fe`)
- Tag peel (OBSERVED): `git rev-parse v0.1.3^{commit}` → `7b5246aead6a6fa3dd8f8d55bbfd90bf3b11df19`
- Remote tag (OBSERVED): `git ls-remote origin refs/tags/v0.1.3` → `d6413d038a17988f2e75738daf52ba29140272fe`
- Release apply result: `converged: true`, steps applied: 2 (`git.push`, `git.tag`)
- Semver: PATCH bump 0.1.2 → 0.1.3 (commit ef39a75b), lockstep with tag enforced by the release planner
- Provenance: DERIVED from engine capability receipts and local/remote git observation; canonical record is the SDDK ledger event chain.
