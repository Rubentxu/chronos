# Catálogo de UAT y certificación por hito

**Política:** [CERTIFICATION.md](CERTIFICATION.md). **Roadmap:** [ROADMAP.md](../ROADMAP.md). Especificaciones históricas válidas: `docs/chronos-agentic-reconstruction/docs/roadmap/MILESTONE_ACCEPTANCE.md` y `docs/chronos-agentic-reconstruction/docs/testing/`. Estas son **definiciones de prueba nuevas**, no resultados aprobados. Un caso pasa sólo en el binario del SHA, con fixture real, evidencias y perfil documentados.

Formato del recibo: `UAT-ID | commit | binario/hash | host/kernel/runtime | fixture/seed | comando | esperado | observado | resultado | logs/run | evidencia de negative paths | revisor`.

## G0 — Veracidad y continuidad de la línea base

| ID | Escenario y aserción observable |
|---|---|
| **UAT-G0-01** | Invocar `events_read` por JSON-RPC con `mode=query` y `mode=by_id`: schema publicado, Serde, herramienta y respuesta coinciden. Discriminador inválido produce error tipado, nunca fallback. |
| **UAT-G0-02** | `observe` uprobe antes de PID y ante sesión inexistente: errores semánticos de capacidad/no encontrado estables y tipados; no exigir prefijos v1. |
| **UAT-G0-03** | Sobre dos consumidores de 10.000 records, avance de productor, reinicio, gap forzado y retención: cursores independientes; `complete` prohibido cuando no está probado; sin robo/destrucción de evidencia. |
| **UAT-G0-04** | Capturar e instalar/detener un uprobe real en host privilegiado y verificar liberación de recursos. Sin privilegios -> `blocked`; no simular el resultado. |
| **UAT-G0-05** | Sobre un único SHA: CI+Coverage+Architecture+Vault completamente verdes, manifest de buckets consistente y artefacto/test-binary identificables; si algún workflow está rojo, no revalidar REC-C7. |

## H1 — Contratos, seguridad, recuperación, rendimiento

| ID | Escenario y aserción observable |
|---|---|
| **UAT-H1-01** | Mismo lockfile/toolchain reconstruye binario; dependencias/SBOM reproducibles y sin vulnerabilidades críticas conocidas no aceptadas. |
| **UAT-H1-02** | Permisos de ejecución para perfil local y remoto definido: ruta válida funciona; fuera de política rechaza; casos symlink/path traversal, proceso no autorizado y timeout generan errores explícitos. |
| **UAT-H1-03** | Sesión en fallo de inicialización, cancelación concurrente, SIGKILL y reinicio: no se filtran probes ni se declara sealed sin pruebas; replay y retención conservan gaps. |
| **UAT-H1-04** | Benchmarks repetibles con host/kernel/seed/fixture, tamaño de sesión y percentiles; criterios fijados ex ante para append, lectura, replay, CPU/memoria y efecto en tracee. |

## M4-F0 / M4-F1 — Instrumentación adaptativa

| ID | Escenario y aserción observable |
|---|---|
| **UAT-M4G-01** | Servicio Go con OTel existente: Chronos reutiliza contexto/procedencia e identifica mecanismo soportado; si OBI/Auto SDK no están disponibles lo declara explícitamente. |
| **UAT-M4G-02** | Go checkout bug: coarse -> instrumento temporal determinista -> mutation/property -> validación de parche; árbol de fuentes original intacto y comparación binario normal vs instrumentado. |
| **UAT-M4R-01** | Rust state corruption: tracing/OTel + probe tipada temporal muestran valor before/after con evidencia; XRay/USDT/eBPF con ADR de viabilidad y límites de plataforma medidos. |
| **UAT-M4R-02** | Fixture de bug sensible al timing: instrumentación profunda altera el fenómeno, detector identifica perturbación y cambia a mecanismo ligero preservando `unknown/unsupported` si falta evidencia. |

## M6 — Contexto distribuido

| ID | Escenario y aserción observable |
|---|---|
| **UAT-M6-01** | Petición atraviesa dos servicios y llega a mutación y property violation: correlación trace/span externo -> invocación Chronos -> mutación con IDs distintos; dos peticiones concurrentes no mezclan contexto. |
| **UAT-M6-02** | Sin contexto, span huérfano, reloj monotónico diferente o OTLP temporalmente indisponible: error/procedencia completos, sin timestamps Unix inventados; ingestión y exportación recuperables/idempotentes dentro del contrato. |

## M7 — Diferencia semántica

| ID | Escenario y aserción observable |
|---|---|
| **UAT-M7-01** | Ejecución good y bad con ruido en orden/timestamps y un bug de estado conocido: encuentra primera divergencia semántica sustentada por eventos y no confunde símbolo con invocación. |
| **UAT-M7-02** | Comparación con gaps, contexto externo ausente o datos incomparables devuelve `unknown/unsupported` y región sin cobertura, no declara igualdad falsa. |

## M8 — Reducir y reproducir

| ID | Escenario y aserción observable |
|---|---|
| **UAT-M8-01** | Input generado que viola propiedad se reduce con seed/fixture guardados; el input mínimo sigue disparando la **misma** propiedad/violación al rerun independiente. |
| **UAT-M8-02** | Reinicio/timeout/nondeterminismo durante shrinking conserva evidencia/procedencia, no etiqueta como “mínimo” sin prueba y no altera ejecución original ni fuente de datos. |

## M9 — Concurrencia

| ID | Escenario y aserción observable |
|---|---|
| **UAT-M9-01** | Dos accesos a misma dirección protegidos por lock o sincronización demostrada no se reportan como carrera confirmada. |
| **UAT-M9-02** | Fixture no sincronizado aporta evidencia happens-before insuficiente y observaciones relevantes, distingue sospechoso/confirmado/unsupported según definición explícita; pérdidas de datos nunca generan confirmación silenciosa. |

## M10 — Explorer

| ID | Escenario y aserción observable |
|---|---|
| **UAT-M10-01** | Visualizar traza grande con paginación/virtualización y memoria limitada según presupuesto preestablecido; navegación y orden de evidencia estables. |
| **UAT-M10-02** | Gap, datos redacted, estado incompleto y falta de permisos se muestran de forma comprensible, nunca como gráficos completos/falsamente exactos. |

## M11 — Nuevos runtimes

| ID | Escenario y aserción observable |
|---|---|
| **UAT-M11-XX** | Por lenguaje/runtime y versión: ejecutar programa real y verificar cada capacidad anunciada, limitación/no soporte, reinicio/error/overhead y compatibilidad de datos. Crear ID concreto por backend antes de certificarlo. |

## OPS — Calidad de producto y release

| ID | Escenario y aserción observable |
|---|---|
| **UAT-OPS-01** | Instalación limpia, configuración y primer diagnóstico real; rollback a versión anterior y migración/lectura de persistencia compatible o error explícito sin pérdida silenciosa. |
| **UAT-OPS-02** | Carga/soak con límites de CPU/RAM/disco, backpressure, retención, recuperación de fallo e indicadores y alertas utilizables; comparación contra presupuesto acordado. |
| **UAT-OPS-03** | Seguridad de host, ejecución de programa controlado, acceso a artefactos, separación de sesiones y ausencia de secretos en logs; modelo de amenazas del perfil validado. |
| **UAT-OPS-04** | Artefacto de release reproduce SHA/lockfile/SBOM, tiene checksums/firma cuando sea política del perfil, runbook de incidentes y aprobación CERT-4 trazable. |

**Regla de evolución:** antes de implementar un mecanismo nuevo, concretar al menos una UAT de éxito, una negativa, fixture y perfil en este catálogo; mantener las UAT históricas si siguen siendo invariantes, sin copiarlas ni declararlas aprobadas por duplicado.
