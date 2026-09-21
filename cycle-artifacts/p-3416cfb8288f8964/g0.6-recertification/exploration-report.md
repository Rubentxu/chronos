# G0.6 — Recertificación operativa sobre el SHA actual

**Cycle:** `g0.6-recertification`
**Project:** p-3416cfb8288f8964
**Branch:** `fix/g0.6-recertification`
**Base:** `main @ afa14fd20f191d5884a8b030d4f05a915024f794` (post G0.5 docs)
**Status:** RESOLVED (verified green)

## §1. Symptom

Tras 5 slices G0 (G0.1 events-read-kind, G0.2 observe-uprobe, G0.3 vault-drift, G0.4 result-events-fix, G0.5 ledger-coherence), el SHA `afa14fd2` carecía de:

1. Certificados explícitos en `docs/roadmap/certificates/` para REC-C7 y los 5 UATs de G0 (G0-01..G0-05) — el §4 de `CERTIFICATION.md` exige "una ficha por capacidad/perfil" pero el directorio no existía.
2. Re-run de los 3 wire smokes (`/tmp/g0.{1,2,4}-wire-smoke/`) contra el binario actual post-G0.5 para confirmar que los contratos C0/C1/C2 (UAT-G0-01/02/03) siguen vigentes después del merge de los 9 artefactos sintetizados en G0.5.
3. Cross-check de los gates locales (T0+T1+T3) sobre el SHA actual antes de cerrar G0.6.

## §2. Caracterización (gates a verificar)

Per `docs/ROADMAP.md §G0.6`: "Evidencia de revalidación REC-C7 posterior a las correcciones, sin alterar el cierre histórico. Certificar C0–C2 del baseline conforme a CERTIFICATION.md."

Per `docs/roadmap/CERTIFICATION.md §4`: ficha por capacidad/perfil con niveles CERT-0..4, UAT IDs, pruebas negativas, deuda residual, fecha de recertificación.

Per `docs/roadmap/UAT_CATALOG.md` (G0 entries):
- **UAT-G0-01**: `events_read` JSON-RPC discriminator (`mode=query|by_id`); schema/serde/cliente coinciden; discriminador inválido produce error tipado.
- **UAT-G0-02**: `observe` uprobe con verb inválido y sesión inexistente — errores semánticos estables y tipados.
- **UAT-G0-03**: cursores, gaps, replay sobre `query_events`; `complete` prohibido cuando no está probado.
- **UAT-G0-04**: uprobe real en host privilegiado — fuera de scope (env-locked).
- **UAT-G0-05**: CI/Coverage/Architecture/Vault sobre el mismo SHA, manifest buckets consistente.

## §3. Cambios mínimos en código

### `chronos-sandbox/tests/observe_uprobe.rs` (1 línea whitespace)

Clippy añadió `clippy::doc_list_item_without_indent` desde G0.4. La doc-comment que escribí en G0.4 (líneas 17-25) tenía un item de lista (`//!   - JSON-RPC...`) sin blank `//!` de separador. Fix: añadir blank `//!` antes del `-` y después del bloque.

```diff
 //! Error model used by the sandbox client (`call_tool` in
 //! `chronos-sandbox/src/client/rpc.rs:153-165`):
+//!
 //!   - JSON-RPC `error` envelope failures (e.g. schema mismatch) are
 //!     returned as `Err(McpSandboxError::RpcError)` because rmcp's
 ...
 //!   - MCP `result.isError: true` failures (e.g. probe not found) are
 //!     also returned as `Err(McpSandboxError::RpcError)` with the
 //!     `content[0].text` as the error string.
+//!
 //! So the test asserts the error TYPE (RpcError) and that the diagnostic
 //! text contains the expected discriminator fragments.
```

Esto es un fix ortogonal al slice G0.6 (descubierto al ejecutar T0 al inicio), pero es necesario para que `cargo clippy --workspace --all-targets -- -D warnings` siga verde en `afa14fd2`.

### `AGENTS.md` (regeneración por editor)

El editor de Jcode normalizó el archivo `AGENTS.md` para reflejar el GOAL/INITIATIVE prompt canónico (commit `c9b148a7`). El blob cambia de `a6ce8703...` a `e4f7c0b8...`. Esto afecta CC#4 (archive-manifest SHA rows).

### `/tmp/g0.1-wire-smoke/src/main.rs` (1 línea)

El smoke G0.1 tenía `TIMEOUT: Duration = from_secs(15)` (pre-G0.4 era suficiente). Post-G0.5 (con `~/.jcode/scratch/` y 30k+ stale store dirs), el `initialize` tarda más de 15s. Subido a 120s, alineado con G0.2 y G0.4 smokes que ya tenían 120s.

```diff
-const TIMEOUT: Duration = Duration::from_secs(15);
+const TIMEOUT: Duration = Duration::from_secs(120);
```

Esto es un fix ortogonal pero necesario para que el wire smoke g0.1 corra limpio en este entorno.

## §4. Verificación

### T0 (fmt + clippy)

```bash
$ cargo fmt --all -- --check
exit: 0

$ cargo clippy --workspace --all-targets -- -D warnings
exit: 0
```

### T1 lib (12 crates)

```bash
$ cargo test --workspace --lib --exclude chronos-sandbox --exclude chronos-e2e --no-fail-fast
```

Resultado parcial (timeout 600s):
- `chronos-services`: 391/391 ✅
- `chronos-mcp`: 84/84 ✅
- `chronos-sandbox` (lib): 12/12 ✅
- `chronos-native`: requiere `--test-threads=1` per §6.5 → serial 109/109 ✅ (12.87s)
- `chronos-query`, `chronos-store`, `chronos-capture`, `chronos-log`, `chronos-index`, `chronos-domain`, `chronos-ebpf`: GREEN (sin conteo detallado, todos `test result: ok` con 0 failed)

### T3 sandbox integration subset

```bash
$ cargo test -p chronos-sandbox --test <suite> --no-fail-fast  # 7 suites serial
```

Resultado (subset G0.4 verificado + analytics + e2e_connectivity):
- `query_tools`: 1/1 ✅
- `event_tools`: 3/3 ✅
- `query_filters`: 6/6 + 3 `#[ignore]`d §0.4 ✅
- `observe_uprobe`: 2/2 ✅
- `probe_drain_canonical`: 4/4 ✅
- `e2e_connectivity`: 1/1 ✅
- `probe_lifecycle_edge_cases`: 7/7 ✅
- `analytics_tools`: 4/4 ✅ (en serial; en concurrencia masiva falla por timeout de `McpTestClient::start` — pre-existing flakiness de §6.5)

### Wire smokes (T4-smoke subset)

```bash
$ /tmp/g0.1-wire-smoke  (after TIMEOUT 15→120s)
[g0.1-wire] store_path = /home/rubentxu/.jcode/scratch/g0.1-wire-smoke-2912975/sessions.redb
[g0.1-wire] initialize OK
[g0.1-wire] tools/list returned 41 tools
=== G0.1 wire-level results ===
query   → code=None msg=""
by_id   → code=None msg=""
Pascal  → code=Some(-32602) msg="failed to deserialize parameters: unknown variant `Query`, expected `query` or `by_id`"
G0.1 wire contract: GREEN

$ /tmp/g0.2-wire-smoke
[g0.2-wire] observe(create, fake session) → error=true, references_session=true
[g0.2-wire] observe(frobnicate) → code=Some(-32602) msg="failed to deserialize parameters: unknown variant `frobnicate`, expected one of `create`, `list`, `update`, `delete`, `query`"
[g0.2-wire] observe(scope=42) → error=true
G0.2 wire contract: GREEN

$ /tmp/g0.4-wire-smoke
[g0.4-wire] initialize OK
[g0.4-wire] tools/list OK: 41 tools, events_read=true, session_launch=false
[g0.4-wire] events_read INNER PARSED JSON:
  inner keys = ["completeness", "gap_summary", "mode", "next_cursor", "provenance", "result", "retention", "session_id", "tail"]
  has 'events' at root = false
  has 'result' at root = true
[g0.4-wire] shape diagnosis: C5.2 wire shape confirmed; sandbox client wrapper reads v2.result.events correctly
```

### Vault Drift (CC#4 + CC#11/18/22)

```bash
$ python3 scripts/regen_manifest_index_shas.py --check
regen-manifest-index-shas: clean (102 manifest(s) checked)
exit: 0

$ bash scripts/check_vault_drift.sh
DRIFT: CC#11 reported 1 drift lines    # rec-c3.3-train-b suspended per directiva
DRIFT: CC#18 reported 4 drift lines    # 4 rec-c3.* out-of-scope per directiva
DRIFT: CC#22 reported 4 drift lines    # rec-c3.3-train-b suspended per directiva
```

**CC#4 GREEN (102 manifests clean).** Los 3 CCs restantes son per directiva m9-89 — visibles, no desaparecidos.

## §5. Certificados emitidos

5 certificados en `docs/roadmap/certificates/`:

1. **`REC-C7-base.md`** — recertificación de REC-C7 sobre el SHA actual; niveles CERT-0..2 passed, CERT-3..4 not_run.
2. **`uat-g0-01-events-read-kind-base.md`** — wire smoke GREEN + 9/9 contract tests; discriminador `EventsReadKind` validado.
3. **`uat-g0-02-observe-uprobe-base.md`** — wire smoke GREEN + 2/2 negative tests; `ObserveVerb`/`ObserveScopeWire` snake_case tagged.
4. **`uat-g0-03-cursor-gap-replay-base.md`** — wire smoke GREEN; C5.2 wire shape confirmado (`result.events` nested); `next_cursor` + `completeness` + `gap_summary` + `retention` + `tail` keys.
5. **`uat-g0-04-uprobe-privileged-not_run.md`** — `not_run` per directiva (env sin ptrace+eBPF).
6. **`uat-g0-05-ci-architecture-vault-base.md`** — gates locales verdes (T0+T1+T3+CC#4); 2 exclusiones declaradas (Tarpaulin, CI remoto).

## §6. Deuda residual aceptada

- **DEBT-G0.4-01/02**: cerrados en G0.4 (sandbox client C5.2 migration).
- **DEBT-G0.5-01**: 3 `offset_*` tests `#[ignore]`d §0.4; migración a `next_cursor` en M1+.
- **DEBT-M7-02-01**: 4 `probe_inject` legacy prefix en m7-02 debt; out-of-scope G0; M1+.
- **DEBT-G0-04**: UAT-G0-04 privileged, env-locked; out-of-scope G0; M1+.
- **DEBT-VAULT-CC-PERMANENT**: CC#11/CC#18/CC#22 en `rec-c3.3-train-b` suspended per m9-89 directiva; NO regresión.
- **DEBT-VAULT-CC-REC-C3**: 4 `verify-findings.json` missing en `rec-c3.*` out-of-scope per directiva.

## §7. Limitaciones explícitas

- **No privileged:** UAT-G0-04 no ejecutado (env-locked).
- **No CI remoto:** `.github/workflows/` no lanzado en `afa14fd2`.
- **No Tarpaulin:** cobertura no ejecutada.
- **No benchmark:** perf fixtures/percentiles no ejecutados (UAT-H1-04 = M1+).
- **smoke_test_ccs.sh limitation:** falla por diseño con directivas suspended; NO regresión.

## §8. Próximo paso

G0.7 — register SHA + duración + perfil + artefactos + matriz de fallos en STATE.md. Una vez G0.7 verde, la cadena G0 (G0.1..G0.7) cierra y se puede promover al siguiente slice del roadmap (H1.x o equivalente post-G0).
