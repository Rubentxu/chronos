# Documentation patch manifest

## Replace

- `/README.md`

## Add

- `/docs/README.md`
- `/docs/reconstruction/00_RECONSTRUCTION_OVERVIEW.md`
- `/docs/architecture/TARGET_ARCHITECTURE.md`
- `/docs/specs/*`
- `/docs/adr/*`
- `/docs/roadmap/*`
- `/docs/testing/*`
- `/docs/migration/*`
- `/docs/research/TECHNOLOGY_BASELINE.md`
- `/docs/spikes/SPIKE_BACKLOG.md`
- `/docs/gui/EXECUTION_EXPLORER.md`
- `/docs/propuestas/04_RECONSTRUCCION_AGENTIC.md`

## Do not delete in first integration PR

- `docs/propuestas/01_*`, `02_*`, `03_*`;
- `chronos-sandbox`;
- breakpoint/watchpoint implementation code;
- legacy MCP tools.

Deprecation/removal happens only after replacement paths satisfy milestone acceptance.

## Suggested first PR

Documentation + M0 issue decomposition only. Do not mix this merge with a large code rewrite.
