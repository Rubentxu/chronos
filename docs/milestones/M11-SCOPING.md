# M11-SCOPING — Lenguajes por demanda y capacidad verificable: catalogación de 7 adapters foundation pre-existente

> **Estado del slice (2026-09-22)**: docs-only scoping. NO se ha ejecutado ningún sub-cycle de M11 en `main @ 67ca0b1b`. Foundation pre-existente **masiva** identificada — 7 crates adapter + `Language` enum canónico con 14 variants + documentación EN+ES. Scoping propuesto con 4 sub-cycles M11.2..M11.5 + M11.1 inventory.

## §1 ROADMAP §M11 §99 ref

ROADMAP §M11 §99 — "M11.1 priorizar Python `sys.monitoring`, JVM JFR+OTel, Node/JS, browser/WASM, C/C++ XRay/rr según evidencias de uso y viabilidad; M11.2 por runtime: capabilities -> fixtures -> negativos -> overhead -> compatibilidad -> UAT-M11-XX. Una plataforma no certificada se anuncia como experimental o unsupported, no como equivalente a otra."

## §2 Estado actual del repo (inspección directa)

`main @ 67ca0b1b` tiene **foundation pre-existente masiva** para M11. NO es "construir adapters desde cero"; es "consolidar, certificar y publicar capability matrix honesta". Inventario:

### §2.1 Crates adapter pre-existentes (7 workspaces)

| Crate | LoC | Tests | Adapter | Mecanismo |
|---|---|---|---|---|
| `chronos-python` | 1,653 | 27 | `PythonAdapter` + `PythonDapAdapter` | DAP / debugpy |
| `chronos-java` | 2,562 | 44 | (Java/JDWP) | JDWP Evaluate |
| `chronos-js` | 1,683 | 12 | (JS/CDP) | CDP (Chrome DevTools Protocol) |
| `chronos-go` | 1,703 | 24 | (Go/Delve) | Delve DAP |
| `chronos-ebpf` | 2,034 | 35 | (eBPF/uprobes) | aya uprobes |
| `chronos-native` | 7,086 | 87 | (Native ptrace) | ptrace system call tracing |
| `chronos-browser` | 3,133 | 45 | (browser/WASM) | (browser probe) |
| **TOTAL** | **19,854** | **274** | 7 adapters | 6 mecanismos distintos |

### §2.2 Language enum canónico

`crates/chronos-domain/src/trace/session.rs:10` define `pub enum Language` con 14 variants: `C`, `Cpp`, `Rust`, `Java`, `Kotlin`, `Scala`, `Python`, `JavaScript`, `Go`, `CSharp`, `Ebpf`, `WebAssembly`, `Native`, `Unknown`. Es el discriminador canónico para session metadata y `program_language` parameter en `debug_run`.

### §2.3 Documentación (multi-language)

- `docs/manual-ai/en/09-multi-language.md` (283L) — 6 language families con mechanism + adapter + capabilities + setup por lenguaje.
- `docs/manual-ai/es/09-multi-lenguaje.md` (240L) — versión en español.
- `docs/ROADMAP.md` §M11 §99 — capítulo formal.

### §2.4 Capability status (output wiring)

`crates/chronos-services/src/output.rs:2231` define `pub struct LanguageAdapterStatus` — wiring estructurado para reportar capability por adapter.

Es decir: hay **foundation** (7 crates adapter + Language enum + capability wiring), **documentation** (manual-ai EN+ES), y **chapter** (ROADMAP §M11 §99). Falta:

1. **Capability matrix ejecutable** (qué funciona end-to-end vs qué es stub vs qué es no-certificado). ROADMAP §11.2 dice "capabilities -> fixtures -> negativos -> overhead -> compatibilidad" — proceso de certificación por adapter.
2. **UAT-M11-XX ejecutables** (ROADMAP §11.2 los menciona pero no están en MILESTONE_ACCEPTANCE.md §M11 todavía).
3. **Per-adapter certification tier** (CERT-1/2/3/4 según H1.2 deployment profiles + capabilities).
4. **Honest reporting de no-certificados** (ROADMAP §0 último: "Una plataforma no certificada se anuncia como experimental o unsupported, no como equivalente a otra"). Esto es ADR-0004 enforcement.

## §3 Sub-cycles propuestos M11.2..M11.5

Cada sub-cycle entrega capacidad verificable + tests incrementales (per ADR-0004). Patrón seguido: m8-01..m8-06, M9.2..M9.5, M10.2..M10.5.

| Sub-cycle | Scope | Deliverables | Tests |
|---|---|---|---|
| **M11.1** (inventory) | ESTE slice | M11-SCOPING.md + ADR-0031 foundation inventory (incluye discovery de 7 crates adapter + Language enum 14 variants + manual-ai EN+ES) | (ADR docs-only) |
| **M11.2** | Capability matrix ejecutable | `chronos-services::language_capabilities::CapabilityMatrix` con certification tier por adapter (CERT-1 stub / CERT-2 partial / CERT-3 certified / CERT-4 production); integration con `LanguageAdapterStatus` wiring; sandbox test fixture ejecutando cada adapter contra su runtime mínimo; honest reporting de fallos | +12 unit + 7 sandbox (uno por adapter) |
| **M11.3** | Per-adapter fixtures + negativos | Fixtures positivos + negativos por adapter (e.g., chronos-python ejecuta script trivial + script con syntax error + script con debugpy unavailable); negative tests documentan comportamiento fail-closed; integration con `LanguageAdapterStatus` reporting | +21 integration (3 por adapter × 7) |
| **M11.4** | Overhead measurement + compatibility matrix | Per-adapter overhead measurement (similar a M7.4 cost baseline); compatibility matrix (qué versiones de Python/Java/Node/Go/etc. están certificadas); UAT-M11-01..07 executors (uno por adapter); ADR (M11) por adapter con findings | +14 unit + 7 sandbox + 7 UAT |
| **M11.5** | Unsorted runtime (Python `sys.monitoring`, browser/WASM, etc.) | New runtime support scope (Python 3.13 `sys.monitoring` podría sustituir DAP/debugpy cuando los casos de uso lo justifiquen); experimental vs certified tier separation; integration con OPS chapter scope; capability matrix actualizada | +14 unit + 7 sandbox |
| **M11.6** (close) | Integration + close report | full T1/T2/T3 sobre `main`; close report `docs/milestones/M11-CLOSE.md`; tag `m11-languages-on-demand.0`; actualizar STATE + JOURNAL + ROADMAP §M11 §99 con check de cierre | T1+T2+T3 |

**Rationale para 5 sub-cycles (M11.2..M11.6)**: sigue el patrón m8-01..m8-06 + M9.2..M9.6 + M10.2..M10.6. M11.6 es close-of-record. Foundation masiva (19,854 LoC + 274 tests + 14-variant enum) reduce scope de M11.2 (capability matrix, NO construcción desde cero) + M11.3 (fixtures, los adapters ya existen). M11.4 introduce overhead measurement + UAT executors nuevos. M11.5 es para nuevos runtime (experimental scope).

## §4 Architecture decisions

### §4.1 D1: Reusar `Language` enum como discriminador canónico

**Decisión**: M11.2 capability matrix usa `Language` enum de `chronos-domain::trace::session` (14 variants) como discriminador. NO introducir nuevo enum paralelo.

**Rationale**: Foundation canónica pre-existente; duplicar = mentira arquitectónica.

### §4.2 D2: Certification tier honesto (CERT-1..CERT-4)

**Decisión**: M11.2 introduce tier explícito por adapter: CERT-1 stub (compila pero no ejecuta), CERT-2 partial (ejecuta happy path), CERT-3 certified (ejecuta + UAT pass), CERT-4 production (CERT-3 + perf budget + security review). Reporte honesto: un adapter CERT-1 se anuncia como experimental.

**Rationale**: ROADMAP §11.2 explicito: "Una plataforma no certificada se anuncia como experimental o unsupported, no como equivalente a otra". ADR-0004 enforcement.

### §4.3 D3: Per-adapter fixtures + negativos (fail-closed)

**Decisión**: M11.3 introduce fixtures positivos + negativos por adapter. Negativos: script con syntax error, runtime unavailable, permissions insufficient, dependency missing. Cada negativo testea fail-closed behavior (no Silent Lie).

**Rationale**: Adapter que retorna éxito cuando falla = Silent Lie. Negative testing obligatorio per ADR-0004.

### §4.4 D4: Overhead measurement en M11.4, NO en M11.2

**Decisión**: M11.4 (no M11.2) introduce overhead measurement per adapter. M11.2 entrega capability matrix estructural primero.

**Rationale**: Separación de concerns — capability matrix es qué funciona; overhead es cuánto cuesta. Mezclar = scope creep.

### §4.5 D5: UAT-M11-01..07 (uno por adapter) en M11.4

**Decisión**: M11.4 introduce 7 UAT executors (uno por adapter). Cada UAT ejecuta el adapter contra su runtime mínimo y verifica capability tier.

**Rationale**: UAT es la verdad operacional. Sin UAT ejecutable, "capability matrix" es promesa, no entrega.

### §4.6 D6: M11.5 es scope experimental, NO replacement

**Decisión**: M11.5 introduce runtime support nuevo (Python 3.13 `sys.monitoring`, browser/WASM improvements) como **experimental** tier separado, NO replacement de adapters CERT-3+.

**Rationale**: ROADMAP §11.1 dice "priorizar según evidencias de uso y viabilidad". Nuevos runtime empiezan en experimental hasta validar.

## §5 Risks

### §5.1 R1: Adapter en CERT-1 es honesto pero inútil

**Riesgo**: Si la mayoría de adapters están en CERT-1 (stub), M11 entrega poca capacidad operativa.

**Mitigación**: Audit inicial de cada adapter en M11.2 inventory. Si <50% reach CERT-3, escalate al operador con opciones (más ciclos M11.2..M11.5 o aceptar scope reducido).

### §5.2 R2: Overhead measurement puede exceder budget

**Riesgo**: M11.4 overhead measurement puede revelar costos no anticipados (similar a M7.4 cost baseline).

**Mitigación**: Threshold-driven stop; budget explícito en M11.4 ADR; fallback a "unsupported, overhead exceeds budget" honesto.

### §5.3 R3: Compatibility matrix puede ser muy larga

**Riesgo**: 7 adapters × N versiones de runtime cada uno = matrix grande (e.g., Python 3.10..3.13 × chronos-python = 4 versiones; similar para Java/Node/Go/etc.).

**Mitigación**: Certificar latest + latest-1 + LTS solamente. Out-of-scope: versiones EOL.

### §5.4 R4: UAT-M11-XX ejecutables requieren runtimes instalados

**Riesgo**: UAT-M11-XX necesita Python 3.x, JDK 17+, Node 18+, Go 1.20+, etc. instalados en el host de CI.

**Mitigación**: Per-adapter env var (`CHRONOS_PYTHON_BIN`, `CHRONOS_JAVA_BIN`, etc.) + skip-with-honest-reporting si runtime unavailable.

### §5.5 R5: M11.5 experimental puede fragmentar capability matrix

**Riesgo**: Nuevos runtime experimental pueden no encajar en el tier model CERT-1..CERT-4.

**Mitigación**: M11.5 introduce tier experimental explícito; capability matrix lo refleja sin ambigüedad.

## §6 UAT mapping

| UAT | Source | Sub-cycle |
|---|---|---|
| UAT-M11-01 (chronos-python happy path) | New, ROADMAP §M11 §99 | M11.4 |
| UAT-M11-02 (chronos-java happy path) | New | M11.4 |
| UAT-M11-03 (chronos-js happy path) | New | M11.4 |
| UAT-M11-04 (chronos-go happy path) | New | M11.4 |
| UAT-M11-05 (chronos-ebpf happy path) | New | M11.4 |
| UAT-M11-06 (chronos-native happy path) | New | M11.4 |
| UAT-M11-07 (chronos-browser happy path) | New | M11.4 |

7 UATs = 1 por adapter. Cada UAT es ejecutable per-adapter contra su runtime mínimo.

## §7 Out-of-scope (M11 chapter)

1. **Add new language not in `Language` enum** (la enum tiene 14 variants; añadir otra requiere ADR previo).
2. **Replace existing adapter** (los 7 adapters son production-grade; consolidar, no reemplazar).
3. **Cross-language debugging** (un trace unificando Python+Go+Rust es scope futuro).
4. **Universal binary** (un único binario que soporte 7 lenguajes es scope futuro).
5. **Compile-time language detection** (runtime detection es lo que existe; compile-time es research).
6. **Probabilistic language detection** (nunca; el `Language` enum es discriminador canónico).
7. **GUI language picker** (scope frontend, no API).
8. **Replace manual-ai documentation** (es docs-only existente; M11 actualiza, NO reemplaza).

## §8 References

- ROADMAP §M11 §99 (`docs/ROADMAP.md`).
- MILESTONE_ACCEPTANCE.md §M11 (a crear en M11.4).
- UAT_CATALOG.md §M11 — UAT-M11-01..07 (a crear en M11.4).
- ADR-0029 §7 — M10 chapter status (peer reference).
- ADR-0028 §2.2 — M9 sub-cycles pattern reference.
- `crates/chronos-python/src/adapter.rs` (1653L, 27 tests) — `PythonAdapter` + `PythonDapAdapter`.
- `crates/chronos-java/` (2562L, 44 tests) — Java/JDWP adapter.
- `crates/chronos-js/` (1683L, 12 tests) — JS/CDP adapter.
- `crates/chronos-go/` (1703L, 24 tests) — Go/Delve DAP adapter.
- `crates/chronos-ebpf/` (2034L, 35 tests) — eBPF/uprobes adapter.
- `crates/chronos-native/` (7086L, 87 tests) — Native ptrace adapter.
- `crates/chronos-browser/` (3133L, 45 tests) — Browser/WASM adapter.
- `crates/chronos-domain/src/trace/session.rs:10` — `Language` enum 14 variants canónico.
- `crates/chronos-services/src/output.rs:2231` — `LanguageAdapterStatus` capability wiring.
- `docs/manual-ai/en/09-multi-language.md` (283L) — product docs (6 language families).
- `docs/manual-ai/es/09-multi-lenguaje.md` (240L) — versión en español.
- ADR-0004 (no falsear una entrega) — aplicado en §4 + §5.
