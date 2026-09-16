# HANDOFF — REC-C0.5-A close + REC-C0 state

**Fecha**: 2026-09-16
**Autor**: orchestrator session
**Branch**: `feat/rec-convergence-truth-gate`
**HEAD**: `36983ed0` (rec-c0-5-a-followup: fix two CI script bugs surfaced by push)
**Base previa**: `d7039e90` (REC-C0.5-B)

---

## REC-C0.5-A (#28) — VAULT DRIFT SWEEP CLOSURE

**Estado**: ✅ CLOSED LOCAL + REMOTE CI GREEN (for in-scope gates).

### Lo que se hizo

El handoff previo decía "1 línea de drift restante". **Era incorrecto**: la
deriva real eran **21 líneas** (20 CC#4 + 1 CC#5). El error venía de no
haber re-ejecutado `check_vault_drift.sh` antes de empezar el ciclo.

### Diagnóstico

1. **CC#4 (20 líneas)**: SHA-256 stale en 13 archive-manifest.md que
   referencian `cycles/index.md` y otros archivos cambiados desde
   REC-C0.5-C. Fix mecánico: `python3 scripts/regen_manifest_index_shas.py`.

2. **CC#5 (1 línea)**: awk pattern `^\| m9-` siempre devolvía 0 porque
   `cycles/index.md` usa tokens sin guion (`| m0 |`, `| m10 |`,
   `| rec-c0 |`). El bug existía desde m9-47 cuando CC#39 Part C
   sustituyó a CC#5 — pero el awk nunca se alineó. Smoke test
   (`smoke_test_ccs.sh:137`) ya documentaba la supersesión.

### Cambios — vault drift closure (commit `cbbfebe5`)

1. `vault-drift-sweep.md` — bloque bash de CC#5 alineado con CC#39 Part C
   (filesystem count con dedup logic idéntico). Bloque ilustrativo
   bajo "### 5." actualizado con la nueva semántica.

2. `cycles/index.md` — añadidas filas para `rec-c0-5-b-probe-inject-capability`
   (SHA `98f9dba4…`) y `rec-c0-5-c-fixture-discovery` (SHA `02c2a552…`).
   `Total cycles` se mantiene en 98 (autoridad de CC#39). `Last archive`
   actualizado a `rec-c0-5-c-fixture-discovery`.

3. 13 `archive-manifest.md` — 20 filas SHA-256 regen'd por
   `scripts/regen_manifest_index_shas.py` (CC#4).

4. `cycle-artifacts/.../rec-c0-5-c-fixture-discovery/verify-findings.json` —
   sintetizado retroactivamente para satisfacer CC#18 (el folder se
   creó antes de que el schema se enforce para REC-C0.*). Síntesis
   documentada en `_note`.

### Cambios — CI follow-up (commit `36983ed0`)

Al pushear el cierre del vault drift, dos CI checks fallaron por bugs en
scripts (no por contenido del ciclo). Ambos arreglados en este commit:

1. **`scripts/validate_cycle_artifacts.py`** — `--root` default era un path
   absoluto de mac del developer (`/var/mnt/DiscoChino2-fast/...`). Linux CI
   fallaba con "path not found". Ahora default = path relativo
   `cycle-artifacts/p-3416cfb8288f8964`, resuelto contra `__file__` del
   script. Funciona desde cualquier cwd en cualquier host.

2. **`scripts/check_architecture_contracts.py`** — `verify_no_new_legacy`
   excluía solo archivos bajo `/tests/` pero NO bloques `#[cfg(test)]`
   dentro de archivos source. REC-C0.5-B introdujo `EventBus` en
   `crates/chronos-services/src/observe.rs:800` dentro del módulo de tests
   (línea legítima). El gate disparaba en cada push. Ahora escanea cada
   `.rs` en HEAD, trackea brace depth desde cada `#[cfg(test)]`, y skip
   líneas dentro de cualquier bloque cfg(test). La exclusión por path
   `/tests/` se preserva para subdirs.

### Verificación

**Local**:
```
bash scripts/check_vault_drift.sh
→ vault-drift-sweep: PASS (48 python CCs all clean, 7 bash CCs all clean)

python3 scripts/regen_manifest_index_shas.py --check
→ (passes silently)

python3 -m pytest scripts/tests/test_regen_manifest_index_shas.py
→ 13 passed

python3 scripts/validate_cycle_artifacts.py
→ Cycle-artifact completeness gate PASSED on 6 cycle(s)

bash scripts/smoke_test_ccs.sh
→ Tests run: 6, Failures: 0
→ All critical CCs detect synthetic drift. CC detection chain is healthy.

CHRONOS_CONTRACT_BASE_REF=9cc44ce3 python3 scripts/check_architecture_contracts.py
→ Architecture/spec fitness gate PASSED.

cargo fmt --all -- --check → PASS
cargo clippy --workspace --all-targets -- -D warnings → PASS
cargo test --workspace --lib --exclude chronos-native --exclude chronos-e2e
  → all 16 crates green
cargo test -p chronos-sandbox --test probe_inject (REC-C0.5-B regression)
  → 4/4 PASS
```

**Remote CI (HEAD = 36983ed0)**:
- ✅ **Vault Drift Sweep** (run 35069269338): success
- ✅ **Architecture Contracts** (run 35069269341): success
- ⏳ **CI** (run 35069269333): in progress (workspace tests)
- ⏳ **Coverage** (run 35069269330): in progress

Los dos gates críticos para REC-C0.5-A (vault drift + architecture
contracts) están en verde en CI. CI + Coverage siguen corriendo pero
incluyen los 34 fallos fuera de scope (REC-C1, bucket D, §6.5) que ya
estaban documentados como pendientes en el handoff previo.

### Resto de la workspace (NO en scope de REC-C0.5-A)

34 fallos sin tocar (todos fuera del scope vault-drift):
- 5 query_filters / query_edge_cases → REC-C1
- 13 chronos-e2e test_ptrace_capture → bucket D
- 6 chronos-native m2_function_frame_capture → REC-C1 / bucket D
- 4 tripwires_tools → bucket C pre-existing
- 4 ptrace_tracer lib flakes → §6.5 needs `--test-threads=1`
- 2 lib tests misceláneos

---

## REC-C0 status global

**Sub-ciclos REC-C0.5**:
- REC-C0.5-A (#28): ✅ CLOSED LOCAL + CI GREEN (in-scope gates)
- REC-C0.5-B (#29): ✅ CLOSED LOCAL (capability-aware probe_inject)
- REC-C0.5-C (#30): ✅ CLOSED LOCAL (sandbox fixture discovery)

**Pendiente para CLOSE REC-C0 a nivel global** (cuestiones de política/fork):
1. **REC-C1** — events_read pagination/offset correctness (5 fallos query)
2. **Bucket D** — ptrace perms decisions (13 + 6 fallos)
3. **Bucket C pre-existing** — 4 tripwires_tools
4. **§6.5 ptrace flakes** — `--test-threads=1` recipe (4 + 2 fallos)

Los puntos 2-4 son **scope de decisión de política**, no de fix
técnico: ¿excluir del workspace run, mantener como opt-in, o añadir
runner específico con ptrace?

---

## Convenciones recordatorias

- ✅ Sin `#[ignore]` para capability-dependent tests — REC-C0.5-B usa UAT gated
- ✅ Sin fabrication — SHAs desde `git rev-parse`
- ✅ Conventions AGENTS.md (FixtureResolver, build deps formales, docs en código)
- ✅ Format REC-C0.5-B: `REC-C0.5-B: <summary>` + issue body + DoD + verification
- ✅ Cycle artifacts: `apply-checkpoint.json` solo cuando `status: CLOSED` (release step)
- ✅ `verify-findings.json` con `subject.{head_sha, base_sha}` + `verifications[]`
- ✅ **Scripts no deben hardcodear paths del developer** — usar siempre
  paths relativos resueltos contra `__file__` del script

## Quick start próxima sesión

```bash
# Pre-flight
cd /var/mnt/DiscoChino2-fast/Proyectos/rust/chronos
git fetch origin main && git checkout main && git pull --ff-only
git checkout feat/rec-convergence-truth-gate && git pull --ff-only

# Verify state (must all PASS)
bash scripts/check_vault_drift.sh
python3 scripts/validate_cycle_artifacts.py
CHRONOS_CONTRACT_BASE_REF=main python3 scripts/check_architecture_contracts.py

# Próximo ciclo candidato: REC-C1 (events_read pagination) — depende de scope/priority
```

## Watch CI

Último push verificado:
- `36983ed0` rec-c0-5-a-followup: fix two CI script bugs

Critical gates (in REC-C0.5-A scope) GREEN:
- Vault Drift Sweep ✅
- Architecture Contracts ✅

CI + Coverage siguen corriendo para runs pre-existentes (no bloquean
REC-C0.5-A local).
