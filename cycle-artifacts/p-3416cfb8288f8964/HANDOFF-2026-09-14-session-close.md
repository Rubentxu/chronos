# Handoff — Cierre de sesión 2026-09-14: M10 roadmap auto-mode

**Leer esto primero al reanudar.** Estado persistido también en memoria del
proyecto (buscar tags `m10`, `closed`, `v0.7.101`, `v0.7.102`).

---

## 1. Estado del repo (verificado al cerrar)

- Repo: `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos`
- Branch: `main` — **HEAD `1161b55c884062142b559e650b82aaf0d7354cc9` == origin/main** (working tree limpio)
- Últimos tags: `v0.7.100` (m9-98, peel `59229283`), `v0.7.101` (ms-property-policy, peel `95c998e7`), `v0.7.102` (ms-evt-typed, peel `0998aa73`)
- CC sweep: PASS (48 python + 7 bash CCs, 98 manifests); `regen_manifest_index_shas.py --check` limpio
- Ramas de ciclo: todas eliminadas tras publicación

## 2. Ciclos cerrados en esta sesión (los 3, publicados y archivados)

| Ciclo | Tag | Resumen |
|---|---|---|
| **m9-98-m902r4-ledger-closure** | v0.7.100 | B-direct: fila obsoleta m9-02-R4 retirada del ledger activo (m9-91 ya entregó `counterexample_bundle_events`). Net Rust: cero. |
| **m10-ms-property-policy** | v0.7.101 | A-min slice S3: `Property::evaluate_feed` / `evaluate_feed_violation` en chronos-domain = único dueño de la política de feed vacío (nunca `Pass` falso); `observation_log` + `state_recorder` delegan. Aceptación F6/F10 verificados. |
| **m10-ms-evt-typed** | v0.7.102 | B-direct per ADR-0003: `EventsReadParams.event_types` → `Vec<EventType>` (rmcp rechaza desconocidos en parse time); `EventType::from_snake_case` en domain = único dueño (21 variantes); `parse_event_type` borrado de server.rs (grep == 0); shims v1 delegan con vocabulario completo; test round-trip 21 variantes. |

## 3. Aprendizajes operativos de la sesión (importantes)

1. **Convención docs-peel (CC#3/12)**: `head_sha == main_sha == remote_tag_peel`
   = commit taggeado. Para ciclos con merge a main después, taggear el merge
   commit hace que los tres coincidan (patrón usado en v0.7.101/102: tag sobre
   el merge commit). El merge commit va en merge-receipt como `Main merge SHA`.
2. **CC#15/18/22/23/34/44/55**: todo ciclo nuevo necesita en cycle-artifacts:
   `title`+`summary` en apply-checkpoint, `verify-findings.json` (con
   `cycle_id`), `## Summary` + `## Files Inventory` en verify-report, `Branch`
   en merge-receipt, `Peel match | true` en release-receipt, `## Cross-check`
   en change-entry. Corregirlos antes del push evita cascadas.
3. **Lock de cargo con agentes concurrentes**: otra sesión de agente estaba
   ejecutando `just test-unit` / `cargo test --workspace`. Solución: usar
   `CARGO_TARGET_DIR=$JCODE_SCRATCH_DIR/m10-target` aislado. Al reanudar,
   comprobar `pgrep -fa cargo` antes de ejecutar gates.
4. **Flake ptrace documentado (AGENTS.md §6.5)**: `chronos-native` lib debe
   correr con `--test-threads=1` (103 tests, ~13 s serial). Un binario colgado
   47 min a 0% CPU en paralelo era este flake.
5. **Hito MS-RACE-FIX ya estaba cerrado**: fue m9-61 (v0.7.63). El registro CLI
   `m10-ms-race-fix` (status CLOSED/phase verify, "no replayable state events")
   es un duplicado obsoleto — ignorarlo.
6. **Workflow aplicado por ciclo** (funcionó bien): branch `feat/m10-*` →
   implementar + tests → T0 → T2 → T4-smoke (`e2e_connectivity` mínimo, con
   `CHRONOS_MCP_PATH` al binario recién construido) → apply-checkpoint →
   verify-findings + verify-report → merge `--no-ff` a main → tag patch bump →
   push main+tag → recibos con SHAs reales (`git rev-parse` SIEMPRE) → vault
   sync (change-entry, archive-manifest, cycles/index.md, terms Last archive) →
   regen fixpoint → CC sweep → commit+push → borrar rama.

## 4. Roadmap M10 — estado y siguientes pasos

Fuente de verdad: `~/.local/share/sddk/projects/p-3416cfb8288f8964/cycle-artifacts/p-3416cfb8288f8964/m10-product-evolution-propose/specs-package/roadmap.md`

| Hito | Estado |
|---|---|
| MS-RACE-FIX | ✅ (m9-61, v0.7.63) |
| MS-PROPERTY-POLICY | ✅ (v0.7.101) |
| MS-EVT-TYPED | ✅ (v0.7.102) |
| **MS-CAP-DISCOVERY** | ⬜ **SIGUIENTE** — A-min, T2+T4-smoke, spec `capabilities-discovery.md`, ADR-0002 |
| MS-TIMESTAMP-NEWTYPES | ⬜ A-min, T2, sin dependencias (alternativa si CAP-DISCOVERY se bloquea) |
| MS-EBPF-LINK-LIFETIME | ⬜ A-min, T2, independiente |
| MS-INV-ORCHESTRATOR | ⬜ desbloqueado (dependía de RACE-FIX + PROPERTY-POLICY) — A-lite grande, spec `agent-investigation.md`, ADRs 0001/0006-0010 |

Nota MS-EVT-TYPED: la aceptación sandbox UAT completa del ADR
(`events_read{event_types=["memory_alloc"]}` contra fixture que aloca) queda
implícita en el wire typing; si al hacer MS-CAP-DISCOVERY aparece el fixture,
verificar entonces.

## 5. Plan para mañana (orden exacto)

1. Pre-flight: `sddk adopt status --root . --scope .` (adoptado, status complete), `git fetch origin main`, verificar HEAD == origin/main.
2. **MS-CAP-DISCOVERY** (A-min): leer spec `specs-package/specifications/capabilities-discovery.md` + ADR-0002; branch `feat/m10-ms-cap-discovery`; tier T0+T2+T4-smoke; tag `v0.7.103`.
3. Tras CAP-DISCOVERY: evaluar MS-TIMESTAMP-NEWTYPES (rápido, T2) y luego plantear MS-INV-ORCHESTRATOR (A-lite grande — probablemente dividir o delegar con swarm).
4. Mantener convenciones de la sección 3 (evitar repetir la cascada de CCs del cierre de m9-98).

## 6. Deudas / hallazgos arrastrados (no bloqueantes)

- FIND-M9-81 (sddk CLI externo), FIND-M9-74 (ptrace flake, mitigado con serial), FIND-M9-71 (observación no bloqueante).
- Registro CLI obsoleto `m10-ms-race-fix` (duplicado de m9-61) — no tocar salvo que el framework lo exija.
- `crates/chronos-mcp/src/server.rs.bak` existe en el repo — candidato a limpieza en un ciclo vault-only futuro.
