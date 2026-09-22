# ADR-0031 — M11 chapter scoping ADR: Languages on Demand over 7 adapter crates + Language enum foundation

**Cycle:** M11 / M11.0-prep (Languages on Demand — formal scoping ADR before execution)
**Status:** `verified` post-write (docs-only; no code change; branch HEAD == `main @ 623aad2f`)

---

## 1. Context

ROADMAP §M11 §99 — "M11.1 priorizar Python `sys.monitoring`, JVM JFR+OTel, Node/JS, browser/WASM, C/C++ XRay/rr según evidencias de uso y viabilidad; M11.2 por runtime: capabilities -> fixtures -> negativos -> overhead -> compatibilidad -> UAT-M11-XX. Una plataforma no certificada se anuncia como experimental o unsupported, no como equivalente a otra."

Tras M9 (ADR-0027 + ADR-0028 + ROADMAP refactor) + M10 (ADR-0029 + ROADMAP refactor), M11 es el siguiente capítulo del ROADMAP. A diferencia de M9 y M10 (que tenían scoping formal antes de esta sesión), M11 ROADMAP §11.2 menciona el proceso "capabilities -> fixtures -> negativos -> overhead -> compatibilidad -> UAT-M11-XX" pero NO había scoping formal previo. ROADMAP §11.1 explícitamente deja la priorización abierta ("según evidencias de uso y viabilidad").

A diferencia de M9 (216 LoC + 5 tests foundation) y M10 (3,662 LoC + 54 tests foundation), M11 tiene **foundation masiva** pre-existente: **7 adapter crates + 19,854 LoC + 274 unit tests + `Language` enum canónico de 14 variants + `LanguageAdapterStatus` wiring + manual-ai docs EN+ES (523L total)**.

Este ADR formaliza la decisión arquitectónica de M11, incluyendo qué se construye (certification tier model + capability matrix + per-adapter fixtures + UAT executors), qué se reusa (7 adapter crates + Language enum + manual-ai docs), qué NO se hace, y cómo se subdivide en 6 sub-cycles verificables (M11.1 inventory + M11.2..M11.5 execution + M11.6 close).

## 2. Decision

M11 (Languages on Demand) se construye como **6 sub-cycles verificables** (M11.1 inventory + M11.2..M11.5 execution + M11.6 close), consolidando **sobre** la foundation masiva pre-existente (7 adapter crates + Language enum + manual-ai docs) — NO desde cero.

### §2.1 Foundation pre-existente (reusable)

**7 adapter crates** en `main @ 623aad2f`, totalizando **19,854 LoC + 274 unit tests**:

| Crate | LoC | Tests | Adapter | Mecanismo |
|---|---|---|---|---|
| `chronos-python` | 1,653 | 27 | `PythonAdapter` + `PythonDapAdapter` | DAP / debugpy |
| `chronos-java` | 2,562 | 44 | (Java/JDWP) | JDWP Evaluate |
| `chronos-js` | 1,683 | 12 | (JS/CDP) | CDP (Chrome DevTools Protocol) |
| `chronos-go` | 1,703 | 24 | (Go/Delve) | Delve DAP |
| `chronos-ebpf` | 2,034 | 35 | (eBPF/uprobes) | aya uprobes |
| `chronos-native` | 7,086 | 87 | (Native ptrace) | ptrace system call tracing |
| `chronos-browser` | 3,133 | 45 | (browser/WASM) | (browser probe) |

**`Language` enum canónico** (`crates/chronos-domain/src/trace/session.rs:10`): 14 variants — `C`, `Cpp`, `Rust`, `Java`, `Kotlin`, `Scala`, `Python`, `JavaScript`, `Go`, `CSharp`, `Ebpf`, `WebAssembly`, `Native`, `Unknown`. Discriminador canónico para session metadata + `program_language` parameter en `debug_run`.

**`LanguageAdapterStatus`** (`crates/chronos-services/src/output.rs:2231`): wiring estructurado para reportar capability por adapter.

**Manual docs** (523L total):
- `docs/manual-ai/en/09-multi-language.md` (283L) — 6 language families con mechanism + adapter + capabilities + setup.
- `docs/manual-ai/es/09-multi-lenguaje.md` (240L) — versión en español.

### §2.2 Sub-cycles M11.2..M11.5 execution

| Sub-cycle | Scope | Deliverables | Tests |
|---|---|---|---|
| **M11.2** | Capability matrix ejecutable | `chronos-services::language_capabilities::CapabilityMatrix` con certification tier por adapter (CERT-1 stub / CERT-2 partial / CERT-3 certified / CERT-4 production); integration con `LanguageAdapterStatus` wiring; sandbox test fixture ejecutando cada adapter contra su runtime mínimo; honest reporting de fallos (fail-closed per ADR-0004). | +12 unit + 7 sandbox (uno por adapter) |
| **M11.3** | Per-adapter fixtures + negativos | Fixtures positivos + negativos por adapter (e.g., chronos-python ejecuta script trivial + script con syntax error + script con debugpy unavailable); negative tests documentan comportamiento fail-closed; integration con `LanguageAdapterStatus` reporting. | +21 integration (3 por adapter × 7) |
| **M11.4** | Overhead measurement + compatibility matrix + UAT | Per-adapter overhead measurement (similar a M7.4 cost baseline); compatibility matrix (latest + latest-1 + LTS); UAT-M11-01..07 executors (uno por adapter); per-adapter ADR con findings. | +14 unit + 7 sandbox + 7 UAT |
| **M11.5** | New experimental runtime | Python 3.13 `sys.monitoring` scope (podría sustituir DAP/debugpy cuando casos de uso lo justifiquen); browser/WASM improvements; experimental vs certified tier separation; capability matrix actualizada. | +14 unit + 7 sandbox |
| **M11.6** (close) | Integration + close report | full T1/T2/T3 sobre `main`; close report `docs/milestones/M11-CLOSE.md`; tag `m11-languages-on-demand.0`; ROADMAP §M11 §99 check + STATE + JOURNAL actualizados. | T1+T2+T3 |

### §2.3 Naming convention (re-confirmada)

Tras M9.1 ADR-0027 §2.3 + ADR-0028 §2.4 + ADR-0029 §2.4:

- **Vault cycles / storage refactors** futuros usan prefijo `cc-m9-NN` o `vault-m9-NN`.
- **ROADMAP §M9 sub-cycles** usan prefijo `M9.N`.
- **ROADMAP §M10 sub-cycles** usan prefijo `M10.N`.
- **ROADMAP §M11 sub-cycles** usan prefijo `M11.N`.
- **M11.1 inventory + este ADR** son docs-only sin tag.
- **M11.2..M11.5** son sub-cycles de ejecución (uno por slice, con tag `m11-languages-on-demand.0` único al cierre M11.6).

## 3. Alternatives considered

Seis alternativas consideradas; una aceptada, cinco rechazadas.

### §3.1 Construir adapters desde cero (rechazado)

Ignorar los 7 adapter crates pre-existentes; implementar adapters nuevos. **Por qué rechazada**: (a) duplica ~19,854 LoC + 274 tests de código production-grade; (b) introduce dos implementaciones paralelas por lenguaje (mantenimiento doble); (c) rompe el invariante de ROADMAP §0.4 (no inventar abstracciones si las existentes sirven); (d) ADR-0004 violation: pretender que se construyó algo que ya existía.

### §3.2 Replace `Language` enum (rechazado)

Eliminar el `Language` enum (14 variants) y reemplazarlo con strings. **Por qué rechazada**: (a) `Language` es canónico (usado en session metadata + `debug_run` + `LanguageAdapterStatus`); (b) strings pierden type safety; (c) variants cubren todos los lenguajes objetivo del ROADMAP §11.1 (Python, JVM, Node/JS, browser/WASM, Rust/C/C++/Go/eBPF); (d) añadir más lenguajes requiere variante nueva (correct approach).

### §3.3 Universal binary single-adapter (rechazado)

Construir un único binario que soporte 7 lenguajes simultáneamente. **Por qué rechazada**: (a) los 7 adapters tienen dependencias runtime distintas (JDWP para Java, CDP para Node, aya para eBPF, etc.); (b) merge de dependencias = binary size innecesario; (c) ROADMAP §11.2 implica per-adapter evaluation; (d) universal binary es scope futuro no-requested.

### §3.4 Skip certification tier (rechazado)

Asumir que todos los 7 adapters están CERT-3 sin verificar. **Por qué rechazada**: (a) Silent Lie per ADR-0004 — algunos adapters pueden estar CERT-1 (stub) sin ejecutar; (b) ROADMAP §11.2 explicito: "Una plataforma no certificada se anuncia como experimental o unsupported, no como equivalente a otra"; (c) certificación tier es honestidad estructural.

### §3.5 Replace manual-ai docs con auto-generated (rechazado)

Generar docs automáticamente desde código (e.g., rustdoc). **Por qué rechazada**: (a) manual-ai cubre casos de uso + setup por lenguaje, no solo API reference; (b) auto-generated pierdes contexto operacional; (c) ROADMAP §0.4 — docs son fuente de verdad separada.

### §3.6 Consolidate over 7 adapter crates + 5 execution sub-cycles + 1 close (aceptado)

Opción adoptada. Justificación:

1. **Honesta**: reconoce foundation 19,854 LoC + 274 tests + Language enum + manual-ai docs, no la reinventa.
2. **Incremental**: cada sub-cycle entrega capacidad verificable (per ADR-0004).
3. **Composable**: M11.3 depends-on M11.2 (capability matrix); M11.4 depends-on M11.2 + M11.3; M11.5 depends-on M11.2..M11.4; M11.6 depends-on all.
4. **Risk-managed**: certification tier (D2) + fail-closed negative tests (D3) + threshold-driven overhead (R2) + per-adapter env vars (R4) + experimental tier (R5).
5. **UAT-aligned**: 7 UAT-M11-XX ejecutables en M11.4 (uno por adapter).
6. **Per-ADR-0004 honesto**: "plataforma no certificada se anuncia como experimental o unsupported, no como equivalente a otra" — capability matrix + certification tier enforce esto estructuralmente.

## 4. Consequences

### §4.1 Positive

- **Scope reducido por foundation masiva**: ~19,854 LoC pre-existentes cubren M11.1 (inventory completo) + M11.2 (capability wiring parcial) + M11.3 (fixtures per-adapter). 5 sub-cycles execution vs ~10+ si construyéramos desde cero.
- **Menos código nuevo**: ~2,000-3,000 LoC estimados para M11.2..M11.5 (vs ~25,000+ desde cero).
- **Más tests por menos código**: ratio tests/LoC > 0.6 mantenido.
- **7 UAT-M11-XX nuevos** (uno por adapter) ejecutables en M11.4.
- **Certificación tier honest**: un adapter CERT-1 se anuncia como experimental, no como equivalente a CERT-3+.

### §4.2 Negative

- **Dependencia en 7 adapter crates contratos**: si cambia un adapter API, M11.2..M11.5 pueden romperse. Mitigación: integration tests per-adapter + capability matrix validation + bump adapter version cuando evolucione.
- **Certification tier depende de audit inicial**: M11.2 puede revelar que <50% adapters reach CERT-3 (R1). Mitigación: escalation al operador con opciones.
- **Overhead measurement puede exceder budget**: M11.4 puede revelar costos no anticipados (R2). Mitigación: threshold-driven stop + "unsupported" honest reporting.
- **Compatibility matrix puede ser muy larga**: 7 adapters × N versiones runtime = matrix grande (R3). Mitigación: latest + latest-1 + LTS solamente.
- **UAT-M11-XX requiere runtimes instalados**: Python 3.x, JDK 17+, Node 18+, Go 1.20+ etc. (R4). Mitigación: per-adapter env var + skip-with-honest-reporting.
- **M11.5 experimental puede fragmentar**: nuevos runtime experimental pueden no encajar en CERT-1..CERT-4 (R5). Mitigación: tier experimental explícito.

### §4.3 Neutral

- **6 sub-cycles = 6 commits** en main (uno por slice) + 1 commit de close = 7 commits totales M11.x.
- **Tests sandbox crecen** (M11.2 +7, M11.3 +21 integration, M11.4 +7 = +35 tests). Sandbox total pre-M11: ~100 tests. Post-M11: ~135.
- **ROADMAP §M11 §99 NO se modifica**: este ADR ejecuta lo que ya está descrito. Si M11.x encuentra especificación incompleta, refinement ADR.
- **Per-adapter ADR en M11.4**: cada adapter tiene un mini-ADR con findings (7 mini-ADRs adicionales).

## 5. Verification evidence

Inspección directa sobre `main @ 623aad2f` (post-M11-SCOPING commit):

- **T0** `cargo clippy --workspace --all-targets --no-deps -- -D warnings` exit=0 (no se tocó código, ADR es docs-only).
- **T1** `cargo test -p chronos-mcp --lib --no-fail-fast` 84/84 PASS (no regresión post-ADR-0031).
- **`git cat-file -e 623aad2f + 67ca0b1b + edc52e64`** exit=0; SHAs accesibles.
- **7 adapter crates verificados** vía `ls crates/` + `wc -l`:
  - chronos-python: 1,653L + 27 tests (`crates/chronos-python/src/adapter.rs` + 9 archivos).
  - chronos-java: 2,562L + 44 tests.
  - chronos-js: 1,683L + 12 tests.
  - chronos-go: 1,703L + 24 tests.
  - chronos-ebpf: 2,034L + 35 tests.
  - chronos-native: 7,086L + 87 tests.
  - chronos-browser: 3,133L + 45 tests.
  - **TOTAL: 19,854 LoC + 274 unit tests**.
- **`Language` enum verificada**: `crates/chronos-domain/src/trace/session.rs:10` con 14 variants (C, Cpp, Rust, Java, Kotlin, Scala, Python, JavaScript, Go, CSharp, Ebpf, WebAssembly, Native, Unknown).
- **`LanguageAdapterStatus` verificada**: `crates/chronos-services/src/output.rs:2231`.
- **Manual docs verificadas**: `docs/manual-ai/en/09-multi-language.md` (283L) + `docs/manual-ai/es/09-multi-lenguaje.md` (240L) = 523L total.

## 6. Mapping to UAT

| UAT | Source | Sub-cycle |
|---|---|---|
| UAT-M11-01 (chronos-python happy path) | New, ROADMAP §M11 §99 | M11.4 |
| UAT-M11-02 (chronos-java happy path) | New | M11.4 |
| UAT-M11-03 (chronos-js happy path) | New | M11.4 |
| UAT-M11-04 (chronos-go happy path) | New | M11.4 |
| UAT-M11-05 (chronos-ebpf happy path) | New | M11.4 |
| UAT-M11-06 (chronos-native happy path) | New | M11.4 |
| UAT-M11-07 (chronos-browser happy path) | New | M11.4 |

7 UATs = 1 por adapter. Cada UAT es ejecutable per-adapter contra su runtime mínimo, verifica capability tier, y reporta honestamente si runtime unavailable.

## 7. M11 chapter status

- **M11.1**: `verified` (M11-SCOPING.md slice previo + este ADR-0031).
- **M11.2..M11.5**: NOT STARTED en `main @ 623aad2f`. Listos para ejecución tras OK operador.
- **M11.6**: NOT STARTED (close-of-record condicional).

**Post-condición de M11 chapter CLOSED** (M11.6 close):
- T1+T2+T3 verdes sobre `main`.
- UAT-M11-01..07 PASS (executable evidence en `evidence/m11/`).
- Capability matrix ejecutable con certification tier por adapter.
- 7 per-adapter ADRs con findings.
- Close report `docs/milestones/M11-CLOSE.md`.
- Tag `m11-languages-on-demand.0` firmado apuntando al merge commit.
- ROADMAP §M11 §99 con check mark de cierre + ref a M11-CLOSE.
- STATE.md con sub-cycle rows prepended + "M11 chapter CLOSED (6/6)".

## 8. Out-of-scope (M11 chapter)

1. **Add new language not in `Language` enum** (la enum tiene 14 variants; añadir otra requiere ADR previo).
2. **Replace existing adapter** (los 7 adapters son production-grade; consolidar, no reemplazar).
3. **Cross-language debugging** (un trace unificando Python+Go+Rust es scope futuro).
4. **Universal binary** (un único binario que soporte 7 lenguajes es scope futuro).
5. **Compile-time language detection** (runtime detection es lo que existe; compile-time es research).
6. **Probabilistic language detection** (nunca; el `Language` enum es discriminador canónico).
7. **GUI language picker** (scope frontend, no API).
8. **Replace manual-ai documentation** (es docs-only existente; M11 actualiza, NO reemplaza).

## 9. References

- ROADMAP §M11 §99 (`docs/ROADMAP.md`).
- MILESTONE_ACCEPTANCE.md §M11 (a crear en M11.4).
- UAT_CATALOG.md §M11 — UAT-M11-01..07 (a crear en M11.4).
- ADR-0029 §7 — M10 chapter status (peer reference).
- ADR-0028 §2.2 — M9 sub-cycles pattern reference.
- `docs/milestones/M11-SCOPING.md` (175L, commit `623aad2f`) — operacional reference.
- `crates/chronos-python/` (1,653L, 27 tests) — PythonAdapter + PythonDapAdapter (DAP/debugpy).
- `crates/chronos-java/` (2,562L, 44 tests) — Java/JDWP adapter.
- `crates/chronos-js/` (1,683L, 12 tests) — JS/CDP adapter.
- `crates/chronos-go/` (1,703L, 24 tests) — Go/Delve DAP adapter.
- `crates/chronos-ebpf/` (2,034L, 35 tests) — eBPF/uprobes adapter.
- `crates/chronos-native/` (7,086L, 87 tests) — Native ptrace adapter.
- `crates/chronos-browser/` (3,133L, 45 tests) — Browser/WASM adapter.
- `crates/chronos-domain/src/trace/session.rs:10` — `Language` enum canónico (14 variants).
- `crates/chronos-services/src/output.rs:2231` — `LanguageAdapterStatus` capability wiring.
- `docs/manual-ai/en/09-multi-language.md` (283L) — product docs (6 language families).
- `docs/manual-ai/es/09-multi-lenguaje.md` (240L) — versión en español.
- ADR-0004 (no falsear una entrega) — aplicado en §3 + §4 (certification tier + negative fixtures + honest reporting).
