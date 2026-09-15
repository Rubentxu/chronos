# Terms Index

Terms, findings, and known issues tracked across cycles.

## Active (open / in-progress)

None at this time.

## Backlog (deferred)

None at this time.

## Terminated

| ID | Severity | Cycle | Status | Fingerprint | Remediation summary |
|---|---|---|---|---|---|
| `coupling-001` | MEDIUM | m10-ms-cap-discovery-followup | terminated (closed) | `d98925d40cf0b9a597002dad7185be11c98f178238bbe2ba6ba6da2904d4df5f` | Added `#[cfg(test)] mod toolset_sync_check` with two `#[test] fn`s verifying ALL_TOOL_NAMES ↔ #[tool] registrations; drift now fails build-time test instead of being a silent constant. |
| `overeng-001` | MEDIUM | m10-ms-cap-discovery-followup | terminated (closed) | `dc391df667a960aa58fdefa482ec98587f16b923ff2fdff39479dd7c4a2ac618` | Extracted `toolset_guard` private method; collapsed 10 byte-identical dispatch guard blocks to a single source of truth; net -30 LOC at call-sites. |

## References

See `adrs/` for architectural decision records.
See `changes/archive/` for closed cycle manifests.

## Metadata

| Campo | Valor |
|---|---|
| Project | chronos |
| Vault | `.sddk-knowledge/p-3416cfb8288f8964/` |
| Last updated | 2026-09-15T14:30Z |
| Last archive | m10-vault-index-reconcile |
| Active findings | 0 |
| Backlog findings | 0 |
| Terminated findings | 2 |