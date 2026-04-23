# 01 — El Paradigma de Debugging AI-Native

## Por Qué el Debugging Tradicional Falla para Agentes IA

Los debuggers tradicionales (gdb, lldb, pdb, delve) están diseñados alrededor de un loop de interacción humana:

1. Poner un breakpoint en una ubicación sospechosa
2. Ejecutar el programa — pausa en el breakpoint
3. Inspeccionar variables, stack, registros
4. Avanzar una instrucción o una línea
5. Re-inspeccionar
6. Repetir cientos de veces

Este modelo está optimizado para un humano que:
- Solo puede mantener unas pocas variables en mente
- No puede issuing 30 consultas simultáneamente
- Necesita reducir el alcance manualmente a través de ciclos hipótesis → prueba
- Tiene tiempo ilimitado para iterar

Un agente IA es lo opuesto:
- Puede sintetizar cientos de puntos de datos a la vez
- Puede issuing todas las consultas simultáneamente en un turno de LLM
- No debe desperdiciar tokens en loops interactivos
- Tiene un contexto limitado — cada round-trip cuesta

**Chronos elimina el loop completamente.** No hay "pausar e inspeccionar." Hay "capturar una vez, consultar todo."

## El Modelo de Sesión Congelada

Cuando `debug_run` ejecuta, Chronos:

1. Lanza el programa objetivo bajo un tracer (ptrace, JDWP, DAP, CDP, Delve o eBPF según el lenguaje)
2. Registra cada entrada/salida de función, llamada al sistema, acceso a memoria, estado de registros y cambio de thread
3. Permite que el programa corra hasta completarse (o timeout)
4. Construye índices de consulta sobre el trace capturado
5. Retorna un `session_id`

La sesión es **inmutable y congelada**. Cada consulta contra ella es de solo lectura. Puedes issuing 50 consultas contra la misma sesión y obtener respuestas consistentes y reproducibles. Nada de lo que consultas modifica la sesión.

Esto habilita análisis verdaderamente paralelo.

## Análisis Paralelo — La Ventaja Central para IA

Porque la sesión está congelada y es de solo lectura, todas las herramientas de análisis son **parallel-safe**. Pueden ser llamadas simultáneamente sin dependencia de orden.

### Nivel de orientación (siempre en paralelo)

```
get_execution_summary()   ──┐
debug_get_saliency_scores() ─┼──► las tres simultáneamente
list_threads()            ──┘
```

Estas tres herramientas responden:
- "¿Cuántos eventos? ¿Algún problema obvio?" (`get_execution_summary`)
- "¿Qué funciones consumieron más CPU?" (`debug_get_saliency_scores`)
- "¿Cuántos threads? ¿Cuáles son sus IDs?" (`list_threads`)

### Nivel de análisis bulk (en paralelo, según síntomas)

```
debug_find_crash()   ──┐
debug_detect_races() ──┼──► según los síntomas que orient_tool reveló
debug_expand_hotspot() ┘
```

## Niveles de Análisis — El Patrón de Lazy Loading

El paradigma AI-native sigue una progresión de粗糙 a fino:

### Nivel 0 — Orientación (siempre primero)

**Herramientas:** `get_execution_summary`, `debug_get_saliency_scores`, `list_threads`

Estas tres herramientas responden las preguntas de mayor nivel sin requerir conocimiento previo del bug. Siempre son el primer paso.

### Nivel 1 — Hotspot (después de orientación)

**Herramientas:** `debug_expand_hotspot`, `debug_call_graph`

Después de que orientación identifica las funciones calientes, estas herramientas muestran la estructura de llamadas y permiten drill-down.

### Nivel 2 — Forense (después de que bulk identifica algo sospechoso)

**Herramientas:** `forensic_memory_audit`, `inspect_causality`, `debug_find_variable_origin`

Cuando el análisis bulk identifica una dirección de memoria o variable sospechosa, estas herramientas trazan el linaje completo.

### Nivel 3 — Drill-down (después de forense)

**Herramientas:** `query_events`, `get_call_stack`, `evaluate_expression`, `debug_get_variables`, `state_diff`, `debug_diff`, `get_event`

Inspección precisa en una ventana de eventos o tiempo específica.

### Nivel 4 — Raw (raramente)

**Herramientas:** `debug_get_memory`, `debug_get_registers`, `debug_analyze_memory`

Acceso a datos de hardware. Casi nunca necesario para languages de alto nivel.

## El Patrón "Fire Once, Analyze Forever"

Una sesión Chronos capturada puede ser interrogada infinitamente:

```
Session capturada el lunes:
  └─ Lunes: análisis de incidente de producción
  └─ Martes: agente diferente carga la misma sesión para verificar fix
  └─ Miércoles: comparada contra nueva sesión del mismo código
  └─ Jueves: ingeniero Different consulta con filtros diferentes
```

La sesión persiste en el store hasta que se elimine explícitamente.

## Por Qué Este Paradigma Es Natural para Agentes IA

El paradigma Chronos se alinea perfectamente con cómo los agentes IA trabajan:

1. **Tokens de contexto** — Capturar todo una vez es más eficiente que múltiples ejecuciones interactivas
2. **Paralelismo** — El agente puede issuing múltiples consultas en una sola respuesta de LLM
3. **Reproducibilidad** — La sesión congelada significa que la misma consulta siempre da el mismo resultado
4. **Composabilidad** — Múltiples agentes pueden trabajar en la misma sesión sin interferir
5. **Almacenamiento** — Sesiones guardadas son activos reutilizables para CI/CD, regresión, y forensics

## Anti-Patrones del Paradigma Antiguo

- **No pongas breakpoints** — No hay "pausar". Consulta el trace.
- **No iteres paso a paso** — Captura todo, luego interroga.
- **No descartes sesiones** — Guárdalas. Son activos valiosos.
- **No hagas polling** — Usa `debug_run` síncrono.
- **No llames herramientas en secuencia** — Envía todo lo paralelo en un batch.

## Resumen

El debugging tradicional es un proceso de descubrimiento secuencial guiado por humanos. El debugging con Chronos es un proceso de interrogación paralelo guiado por datos. La diferencia fundamental es que Chronos te da todo el contexto de la ejecución de una vez, y puedes hacer cualquier pregunta sobre él en cualquier momento — sin necesidad de re-ejecutar, sin breakpoints, sin loops.
