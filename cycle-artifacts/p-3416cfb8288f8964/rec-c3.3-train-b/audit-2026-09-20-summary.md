# Auditoría externa Chronos — Resumen ejecutivo (2026-09-20)

## Fuente

Auditoría técnica y arquitectónica del repositorio `Rubentxu/chronos`,
entregada el 2026-09-20T08:33Z. 15 secciones + plan de reconducción.

## Estado del proyecto al momento de la auditoría

- **main HEAD**: `65754dcb` (19 sep 2026, 23:28 UTC)
- **CI principal**: 5 workflows verdes sobre `65754dcb`
- **Reconstrucción**: REC-C3 abierto
- **Tren B (rec-c3.3-train-b)**: 6 slices + 1 vault row aplicados
  desde el resume de la mañana; HEAD `5ce52605`

## 4 observaciones accionables que afectan a Tren B

### 1. Orden sub-óptimo (audit §3.2 A2 + §12.2 R1/R2)

Auditor recomienda R1 (services → native) **antes** de R2 (services → store).
Tren B ejecutó R2 primero (slices C/D/F/E-rewire) porque era el rewire más
fácil de hacer bien. El rewire nativo (slice E-partial solo cubre advance/step;
el full rewire de `LiveProbeSession` queda pendiente).

**Decisión tomada**: forward-only (B7). El coste de revertir `d7cdbae9` y
rehacer el orden supera el valor del orden "ideal". El native rewire queda
fichado como el siguiente ciclo concreto (propuesto: `REC-C3.3.3-Tren-B-extension`
o `REC-C3.4-native`).

### 2. CounterexampleRepository sin consumidor real (audit §13)

El slice D introdujo un port (`CounterexampleRepository`) y un adapter
(`SessionStoreBackedCounterexampleRepository`) pero `ChronosCounterexampleService`
sigue usando `&SessionStore` directamente. El port existe pero **no tiene
consumidor de producción**. La auditoría §13 dice: "no crear una abstracción
que todavía no tenga un consumidor real".

**Decisión tomada**: marcar el port como **experimental** hasta que un
consumidor real (el rewire de counterexample service) aterrice. El adapter
sirve como referencia de cómo sería ese rewire; no se elimina porque su
código es correcto y pequeño.

### 3. Harness sin chequeo de identidad binario ↔ commit (audit §8.3)

`AGENTS.md §1` documenta el riesgo de "binario obsoleto mide código viejo".
El harness `McpTestClient::start()` no enforce la garantía: acepta cualquier
binario que exista. Esto significa que un sandbox test puede pasar evaluando
código de un commit anterior sin que el fallo se detecte.

**Decisión tomada**: cambio transversal — fuera del scope de Tren B.
Propuesto como ciclo dedicado `REC-C0.5-harness` (o equivalente). Diseño
mínimo viable: `BinaryIdentity { sha256, mtime }` capturada en `start_path()`,
logueada al inicio de cada test, con un opt-in `CHRONOS_MCP_EXPECTED_SHA`
para fallo duro si no coincide.

### 4. ChronosServer god-object (audit §6.2)

389KB de fuente en `crates/chronos-mcp/src/server.rs`. La auditoría
recomienda extracción **vertical por flujo** (ej. session_start →
events_read → session_stop), no división masiva en N archivos.

**Decisión tomada**: fuera del scope de Tren B. Ownership: REC-C4 (cuando
la hexagonal esté cerrada, REC-C3.5 / REC-C3.4). Tren B no debe entrar
en ChronosServer salvo para añadir el field `archive` y propagar
`SessionsContext` — exactamente lo que hizo el slice F.

## Lo que NO está en desacuerdo con la auditoría

- ✅ Sesiones: el port `SessionArchive` se usa de verdad (SessionsService
  consume el port en d7cdbae9). El adaptador `SessionStoreBackedSessionArchive`
  tiene un consumidor real y verificado por 8 integration tests.
- ✅ Composición: `composition.rs` mantiene bootstrap-scoped vs session-scoped
  distinction (lo que la auditoría §3.3 elogia).
- ✅ ExecutionLog: separación evidencia/retención/mantenimiento preservada.
- ✅ SRP: ChronosServer no se tocó salvo para añadir el field `archive` y
  propagar 5 constructions. El auditor lo señala como futuro trabajo, no
  como bloqueo.
- ✅ ISP: `SessionArchive` es una interfaz estrecha, no una copia 1:1 de
  `SessionStore` (5 métodos: save, load, list, delete, count_events, is_persistent).
  Esto sigue la regla de auditoría §3.2 A1 ("el contrato debe describir
  necesidades de aplicación, no replicar la firma del store").

## Decisiones operativas tomadas en este commit

1. Forward-only: no revertir `d7cdbae9`. Native rewire = próximo ciclo.
2. Counterexample port = experimental hasta consumidor real.
3. Harness identity = ciclo transversal aparte.
4. ChronosServer god-object = REC-C4 territory.

## Lo que el auditor dice que NO deberíamos hacer (audit §13, §14)

- ❌ Reescribir Chronos desde cero
- ❌ Iniciar nuevas capacidades antes de cerrar REC-C3..C7
- ❌ Crear runtime distribuido
- ❌ Bus de eventos paralelo al ExecutionLog
- ❌ Sustituir todas las interfaces por un nuevo framework
- ❌ Dividir masivamente los módulos grandes en un solo ciclo
- ❌ Abrir hitos posteriores a REC-C7 antes de tiempo

Nada de lo ejecutado en Tren B cae en estas categorías.
