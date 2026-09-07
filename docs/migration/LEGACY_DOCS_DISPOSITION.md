# Legacy docs disposition

Maps `docs/propuestas/*` (legacy decisions) to their reconstruction
successor or marks them "no successor — keep as historical context".
Source of truth: `docs/chronos-agentic-reconstruction/docs/migration/LEGACY_DOCS_DISPOSITION.md`.

## Mapping (initial)

| Legacy file | Successor | Action |
|-------------|-----------|--------|
| `docs/propuestas/04_RECONSTRUCCION_AGENTIC.md` | `docs/chronos-agentic-reconstruction/docs/reconstruction/00_RECONSTRUCTION_OVERVIEW.md` | superseded |

(Other `docs/propuestas/*` files: enumerate in follow-on cycles M1+ as
part of the docs migration. No deletions; legacy files remain for
historical context.)

## Why no deletions

The reconstruction is convergent — preserve useful code, correct
contracts, then grow. Legacy proposals influenced the reconstruction but
contain decisions now superseded. They remain readable in
`docs/propuestas/` so a future reader can trace the design history
without leaving the repository.
