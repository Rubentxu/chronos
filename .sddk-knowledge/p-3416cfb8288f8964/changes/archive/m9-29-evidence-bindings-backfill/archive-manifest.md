# Archive Manifest — m9-29

| Field | Value |
|---|---|
| Cycle | m9-29-evidence-bindings-backfill |
| Head SHA | `8617ad0dde11f1a9e83a7f3c110dda3d691e374d` |
| Base SHA | `a65927d960d7a5b42cd425dc34befe3efe7c692f` |
| Tag | `v0.7.27` |
| Path | B-direct |
| Date | 2026-09-12 |

## Summary

Backfills the canonical `## Evidence bindings` section in archive-manifest.md
for m9-11 through m9-27. Each section lists the cycle's released artifacts
(cycle-artifacts/*.json + .md files) and the change-entry with their
computed SHA-256. Adds cross-check #21 to vault-drift-sweep.md enforcing
that all m9-* archive-manifests have a `## Evidence bindings` section.

## Artifact index

| Path | SHA-256 |
|---|---|
| `cycle-artifacts/p-3416cfb8288f8964/m9-29-evidence-bindings-backfill/apply-checkpoint.json` | `cb2821f7aca514599c126108adda4629538edd27e2bede9fb700918b94b9f2e6` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-29-evidence-bindings-backfill/merge-receipt.md` | `4488f7914ed47c575f7d117acf1000712107b4e01269577f04f5ded3a099afd0` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-29-evidence-bindings-backfill/release-receipt.md` | `1a9a6acc114531c43dcf52465714b3757fbdbc2d13207d67116386113299cc46` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-29-evidence-bindings-backfill/release-report.md` | `8479220f1fc6ab0d70e705736b83f26acbc0d51c2da89be2fbd6f94fbf724b43` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-29-evidence-bindings-backfill/verify-findings.json` | `f51ff30380ac709dcc15a9242b461f2741fd4099dd7304dafd75dddcb5374245` |
| `cycle-artifacts/p-3416cfb8288f8964/m9-29-evidence-bindings-backfill/verify-report.md` | `72c34317df72ba6ba6211500766b0d2c2d4266503af3d1ee5b1f598b26886367` |
| `.sddk-knowledge/p-3416cfb8288f8964/changes/m9-29-evidence-bindings-backfill/change-entry.md` | `da014a4bd4b2fad3fb6f598ffb912a84e38aa5ef32fc06cb1d21f5ad42dd7517` |

## Evidence bindings

Vault metadata only. Each cycle artifact listed with its computed
SHA-256 in the Artifact index above.

## Drift summary

m9-11..m9-27 archive-manifest.md files (17 total) were missing the
canonical `## Evidence bindings` section. m9-29 backfills it for all 17
with one bullet per cycle artifact (path → SHA-256).

## Cross-checks added

- #21 (`vault-drift-sweep.md`): all m9-* archive-manifests must have
  a `## Evidence bindings` section (with m9-01/m9-02 exemption for
  pre-cycle-artifacts era placeholder format).
