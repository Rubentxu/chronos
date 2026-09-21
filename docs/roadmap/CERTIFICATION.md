# Sistema de certificación de Chronos — política y evidencias

**Autoridad:** [ROADMAP.md](../ROADMAP.md). **Estado vivo:** [STATE.md](STATE.md). **Casos:** [UAT_CATALOG.md](UAT_CATALOG.md). Se certifica una **capacidad + perfil de entorno + versión/SHA**; nunca “todo Chronos” por mera presencia de tests. Fecha de instauración: 2026-09-21. Ninguna certificación nueva se declara concedida por esta documentación.

## 1. Niveles acumulativos

| Nivel | Nombre | Obligaciones y prueba de salida |
|---|---|---|
| **CERT-0** | Contrato y diseño | Requisito con ID, alcance, invariantes/no-goals, tipo de evidencia, riesgos, ADR si procede, matriz de capacidades y UAT de éxito/error definidos; revisión humana trazable. |
| **CERT-1** | Implementación aislada | `fmt`, `clippy -D warnings`, build/features relevantes, unit/property/contract tests, Serde+schema roundtrip, fallos explícitos, ausencia de nuevas violaciones hexagonales; resultados vinculados al SHA. |
| **CERT-2** | Integración canónica | Tests MCP/CLI reales sobre binario **del mismo SHA**, UAT de éxito y negativos, persistence/restart/replay, cursores/gaps/provenance, CI y cobertura válidas, sin false confidence; estados soportado/no soportado explícitos. |
| **CERT-3** | Plataforma representativa | UAT real por lenguaje/backend con versión de kernel/runtime y privilegios, carga/latencia/memoria/perturbación, condiciones de pérdida, cancelación, timeout, aislamiento, trazabilidad de artefactos; sin omitir rutas privilegiadas anunciadas. |
| **CERT-4** | Perfil operable/production-ready | CERT-0..3 para todo lo incluido en ese perfil + análisis de amenazas y dependencias/SBOM, instalación/upgrade/rollback, retención y restore, límites/alertas, documentación de operación, compatibilidad y aceptación de riesgo residual firmada. Revalidar por release y cambios materiales de entorno. |

El resultado de cada nivel es uno de: `not_started`, `running`, `passed`, `failed`, `blocked`, `not_applicable` (último requiere explicación, propietario y aceptación). Una fase **no** sube de nivel por aprobar otra fase distinta. Si se modifica el contrato, binario, toolchain, backend o schema, invalidar los certificados afectados y ejecutar el subconjunto de regresión + los gates obligatorios.

## 2. Anillos de verificación (reusar las reglas T0–T5 de AGENTS.md)

- **T0 documental:** links existentes, paths, referencias al ledger, sin falsificar estado; para documentación histórica usar verificación de punteros e integridad de archivos; no gastar horas de sandbox en docs-only.
- **T1 estructura:** fmt + clippy + compile/all-features + arquitectura/legacy ratchets; en un slice de código no bastan para declarar funcionalidades.
- **T2 contrato:** unit tests, property-based, API typed schema/Serde, incompatibilidades y errores, revisión de ownership.
- **T3 integración:** cargo test workspace por los buckets documentados; no silenciar fallos o alterar el bucket para pasar; CI + Coverage + Architecture + Vault del mismo SHA.
- **T4 UAT local real:** sandbox, subprocess del binario de ese SHA, eventos/cursor/stop/replay/persistencia; fixture y log de ejecución reproducibles.
- **T5 UAT privilegiado:** host con ptrace/eBPF, permisos mínimos, fixtures reales, matriz kernel/driver/runtime, telemetría y ausencia de tests que hacen early-return silencioso.
- **T6 staging/operación:** carga/soak, aislamiento, seguridad, instalación/upgrade/rollback, caída y recuperación, retención, incident drill, firmas/SBOM y observabilidad. Necesario para CERT-4, no exigido para un PR documental.

Los T0..T6 son **tipos de prueba**, no números de certificación ni porcentajes. Marcar cada prueba con ejecutor, SHA, fecha, comando, perfil, fixture, resultado, razón de skip, artefacto/log y enlace al run; si la prueba no se pudo ejecutar, poner `blocked` o `not_run`. Una corrida de tests de una rama o un tag anterior no acredita el HEAD actual.

## 3. Reglas de fiabilidad y anti-falsos-positivos

1. No Silent Lies: `complete` sólo con evidencia continua; `unknown/unsupported/gap/incomplete/heuristic` nunca se convierten silenciosamente en array vacío o éxito.
2. Nunca `skip`, `#[ignore]` o mocks como sustituto de la UAT real. La separación por privilegios es explícita y exige una plataforma donde ejecutar T5 antes de certificar el backend.
3. La cobertura informa por crate y rutas de negocio, con especial atención a ramas negativas; no perseguir un porcentaje ficticio ni afirmar cobertura total si Tarpaulin terminó con error.
4. Para fallos no deterministas: reproducir en baseline, registrar seed/entorno y frecuencia, aislar condiciones y corregir raíz; no etiquetar como flake sin prueba.
5. Pruebas de regresión por contrato, no sólo comparación textual de errores heredados. El esquema JSON que se publica debe ser exactamente la representación que acepta Serde.
6. Los objetivos de rendimiento se fijan **antes** de probar, en fixture/host/kernel conocido, con al menos baseline, percentiles, uso de memoria y overhead/perturbación del tracee. Sin baseline -> `not_run`, no “rendimiento adecuado”.
7. Para every high-risk boundary: prueba de éxito, fallo explícito, timeout/cancelación y recuperación; además aislamiento entre consumidores y reinicio cuando haya persistencia.
8. No versionar secretos, trazas reales sensibles, binarios temporales ni dumps no anonimizados como “evidencia”.

## 4. Registro de certificación (una ficha por capacidad/perfil)

Mantener las fichas pequeñas en `docs/roadmap/certificates/<ID>-<profile>.md` cuando se ejecute un gate. Plantilla mínima:

- ID requisito/hito/capacidad; alcance, versión, SHA exacto, perfil y limitaciones.
- Niveles CERT-0..4: estado y razón de cada uno, propietario/revisor independiente de ser posible.
- Mapa UAT IDs -> fixture, comando, duración, resultado, GitHub Actions run o ruta de recibo inmutable.
- Pruebas negativas, seguridad, perf, recovery/rollback y exclusiones justificadas.
- Incompatibilidades, deuda residual aceptada con owner/fecha de revisión y fecha/condición de recertificación.
- Historial de certificados invalidados; jamás sobreescribir un certificado anterior con un estado nuevo sin registrar la transición.

**Gate de release:** una etiqueta semver no crea una certificación. Publicación restringida al perfil que tenga CERT-4 válido, con versión/workspace/binary/ledger coherentes y sin CI obligatoria roja. Para releases experimentales declarar públicamente limitaciones y nivel alcanzado.

## 5. Protocolo de aceptación de cada ciclo

1. Antes de tocar código: identificar requisitos del ledger, UAT IDs y fallos baseline del mismo SHA; escribir criterio de éxito y qué se **no** hará.
2. TDD/caracterización del contrato con resultado observado; cambios mínimos en uno o varios commits pequeños.
3. Ejecutar T1/T2 y T3/T4/T5/T6 según impacto, preservar stdout/XML/log y recibo ligado al SHA. No cerrar un gate con pruebas del commit prefinal.
4. Revisión independiente o reproducible de resultados, seguridad/compatibilidad y evidencia; actualizar los registros de requisitos sólo para capacidades realmente probadas.
5. Escribir fila en JOURNAL, estado en STATE y archivo de certificado cuando proceda. El diario sólo apunta a evidencia; no la sustituye.
6. Si algún gate falla: `failed`/`blocked`, causa, siguiente acción y propietario. No promocionar hito o activar otro producto.
