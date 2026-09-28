# REC-C3-hexagonal-closure · Etapa C

## Scope

Close the audit §13 "store → native" smell: remove the optional
`chronos-native` dependency from `chronos-store` and the
`address_normalization` feature that gated it.

## Commit

`0b099308` — refactor(store): remove optional chronos-native
dependency and address_normalization feature (REC-C3-hexagonal-closure
Etapa C)

## Investigation finding

The audit-grounded hexagonal closure would have moved `AddressNormalizer`
to `chronos_domain::ports` and let `chronos_native::SymbolOffsetNormalizer`
implement it. **But** before this slice, deep investigation revealed
the smell was sharper than "store → native":

- `chronos-store::diff` defined its own `AddressNormalizer` trait
  gated behind a Cargo feature.
- `chronos-native::address_normalizer` defined a **different**
  `AddressNormalizer` trait + `SymbolOffsetNormalizer` implementation
  with its own `SymbolOffset` struct.
- **Nothing in the workspace ever enabled the feature or consumed
  either trait.** Verified via
  `grep -rn "address_normalization" crates/ Cargo.toml`.

The feature was a maintainability trap: two parallel trait
definitions, zero call sites, and an optional dependency that pulled
in the entire native crate for nothing.

## R-roadmap refinement (REC-C3.5-C.1 follow-up, deferred)

The correct long-term design is:
1. `chronos_domain::ports::AddressNormalizer` as a single canonical
   port.
2. `chronos_native::SymbolOffsetNormalizer: impl AddressNormalizer`.
3. `chronos_services::diff` consumes the port via the composition
   root.
4. The feature flag comes back as a *real* capability, not dead code.

This work requires a real consumer wired through the composition root
before it lands — per the audit §13 guideline "no abstraction without
a real consumer", the trait itself was dead code dressed up as a port.
Removing it closes the smell directly. REC-C3.5-C.1 is filed for the
rebuild when a downstream consumer needs ASLR-aware diff.

## Wiring changes

- `crates/chronos-store/Cargo.toml`: removed
  `chronos-native = { optional = true }` and the
  `address_normalization` feature.
- `crates/chronos-store/src/diff.rs`: stripped the entire
  `#[cfg(feature = "address_normalization")]` block (trait shadow,
  structs, gated impl, helper). The non-gated `compare_impl` is now
  the only impl.
- `DiffReport`: dropped `normalized_hash`, `addresses_normalized`,
  `addresses_raw`, `warnings` — all four were unused outside the
  module (verified via
  `grep -rn "normalized_hash\|addresses_normalized\|addresses_raw\|warnings" crates/chronos-services crates/chronos-mcp crates/chronos-cli`
  returning no hits).
- `crates/chronos-store/benches/cas_bench.rs`: dropped the trailing
  `None` argument.
- `crates/chronos-services/src/diff.rs`: same trailing-`None` removal
  in the service's call.

## Verification

- `cargo check --workspace --all-targets --exclude chronos-sandbox
  --exclude chronos-e2e`: clean.
- `cargo test -p chronos-store --lib`: 84 passed (5 diff tests
  adapted to the new 6-arg signature).
- `cargo test -p chronos-services --lib`: 391 passed.
- `cargo test -p chronos-mcp --lib`: 109 passed.
- `cargo build --bin chronos-mcp`: clean. Binary SHA256:
  `064945e6f9d8f0acf6e4b625128bea38790b2f92ec3031acd51906c14998a8e5`
  (mtime 2026-09-20 13:44:28 +0200).
- T4 smoke: `chronos-sandbox e2e_connectivity` (1 test, 19.45s) +
  `analytics_tools` (4 tests, 80.59s) all green.

## Decoupling check

```text
$ grep -rn "chronos-native\|use crate::ptrace" \
    crates/chronos-store/Cargo.toml crates/chronos-store/src/
crates/chronos-store/src/diff.rs:4: ... (doc comment)
crates/chronos-store/src/diff.rs:7: ... (doc comment)
```

**Store has zero compile-time or runtime references to chronos-native
after this slice.** The two grep hits are inside the file-level doc
comment explaining why the feature was removed.

## Open follow-ups

- **REC-C3.5-C.1** — when a downstream consumer needs ASLR-aware
  diff, land the canonical port + adapter pattern (port in domain,
  adapter in native, service consumes via composition root).
- **REC-C3.5-C.2** — delete the shadow module
  `chronos-native/src/address_normalizer.rs` (verified unused via
  `grep` across the workspace). Focused slice to keep blast radius
  small.
