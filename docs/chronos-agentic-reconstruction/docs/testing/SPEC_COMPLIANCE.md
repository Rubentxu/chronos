# Specification compliance and drift protocol

## Purpose

Chronos must not confuse **declared**, **implemented** and **verified**. The canonical machine ledger is `/reconstruction-contracts.toml`; human-readable specs and close reports explain intent, but the ledger records current truth.

## Status model

- `verified` — implementation exists and a reproducible behavioral UAT proves the contract.
- `partial` — useful implementation exists but one or more required semantics remain unproven/unimplemented.
- `gap` — the required behavior is absent or contradicted by current implementation.
- `planned` — intentionally future work; no current delivery claim.
- `blocked` — implementation cannot proceed without a recorded dependency/decision.

`closed` is a milestone/cycle state, not a requirement-compliance state. A previously closed cycle may therefore leave a `partial` or `gap` requirement; convergence must own it explicitly.

## Evidence hierarchy

Strongest to weakest:

1. real sandbox UAT through the public boundary;
2. integration test through production components;
3. unit/property test of a domain/application invariant;
4. static architecture fitness rule;
5. source inspection;
6. documentation/close report.

Items 5–6 cannot alone justify `verified`.

## Required traceability

Every `verified` requirement records:

```text
requirement id
 -> implementation evidence path(s)
 -> UAT/test identifier(s)
 -> reproducible verification command
 -> owning roadmap gate
```

If semantics change, the ADR/spec, ledger and UAT must change in the same cycle.

## Anti-drift rules

### D1 — Architecture debt is a shrinking baseline

Known forbidden dependency edges are listed in `reconstruction-contracts.toml`. CI fails on a new edge. CI also fails when a baseline edge disappears but its waiver is not removed.

### D2 — Legacy APIs are ratcheted

The architecture checker scans added production Rust lines for selected legacy symbols. Existing legacy usage is migration debt; new usage is forbidden.

### D3 — No closure by inventory

A cycle cannot claim a contract merely because a file, type, endpoint, DTO or test name exists. Verification must assert the semantics named by the contract.

### D4 — Negative-path UAT is mandatory

Every trust-sensitive API requires tests for failure/unknown/incomplete/gap cases as well as the happy path.

### D5 — Completeness is earned

The literal value `complete` may be emitted only if the producing service can prove coverage of the authoritative evidence interval and knows that no Gap intersects it.

### D6 — Capability claims are tested

README/API capability tables are generated/reconciled from verified ledger entries. A language/backend may be called "supported" only at the depth actually verified.

### D7 — Milestone namespace is explicit

Reconstruction milestones use `M*`; convergence gates use `REC-C*`; repository/vault housekeeping cycles must not reuse those names as product-delivery claims.

## Review template for every future cycle

A verify report must answer:

1. What invariant or user/agent outcome changed?
2. Which production path proves it?
3. Which real fixture exercises it?
4. What negative/failure case was tested?
5. What evidence can still be unknown/incomplete?
6. Which dependency/legacy count decreased or increased?
7. Did any capability claim or README text change?
8. Does replay produce the same semantic result?
9. Is the mechanism deterministic enough for an agent to act on it?
10. What remains explicitly unsupported?

## REC-C7 drift audit

Before convergence closes:

- run the strict architecture checker;
- run all-features compile;
- run workspace tests and dedicated sandbox UATs;
- inspect the ledger for all `REC-C*` owners;
- confirm no P0/P1 false-confidence route remains;
- confirm documentation claims are a subset of verified capabilities;
- confirm legacy/dependency waivers are empty or an ADR explicitly moves them beyond convergence (exception requires user-visible risk statement).
