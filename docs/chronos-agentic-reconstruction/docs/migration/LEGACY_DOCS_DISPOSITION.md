# Legacy documentation disposition

The existing `docs/propuestas/` documents are useful design history but must not remain the active roadmap.

## `01_VISION_Y_PARADIGMA.md`

Keep: move away from record-everything, tactical microscope, tripwires/black-box concept, differential debugging.

Supersede:

- universal "semantic eBPF" decoding Python/JVM object models;
- fixed ring buffer as central truth;
- claim that CAS yields exact semantic divergence in O(1).

Replacement: reconstruction overview, adaptive instrumentation and roadmap M7.

## `02_ARQUITECTURA_LLM_EVENT_BUS.md`

Keep: low-level observation separated from semantic resolution; agent composes primitives.

Supersede: dual raw/semantic bus as primary storage, raw-memory semantic decoding as default, ring-only persistence model.

Replacement: target architecture, ExecutionLog and evidence/trust specs.

## `03_PLAN_DE_IMPLEMENTACION_Y_LIMPIEZA.md`

Keep: collapse duplicate adapter contracts, non-blocking lifecycle, tripwire work, simplify MCP.

Explicitly reject/supersede:

- deleting `chronos-sandbox`;
- deleting breakpoint/watchpoint code solely because it is human-centric before evaluating internal backend utility;
- ring-buffer-only persistence;
- time estimates as milestone completion criteria.

Replacement: roadmap, milestone acceptance, UAT strategy and ADR-0010.

## Recommended repository action

1. retain the three files as historical records;
2. add a "Historical / superseded" banner;
3. link them to `docs/reconstruction/00_RECONSTRUCTION_OVERVIEW.md`;
4. avoid deleting history in the first reconstruction PR.
