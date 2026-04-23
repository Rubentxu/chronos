# 10 — Anti-Patrones

Estos son los errores más comunes al usar Chronos con agentes IA. Cada anti-patrón muestra el enfoque incorrecto, el enfoque correcto, y por qué importa.

---

## Anti-Patrón 1: query_events Sin Filtros

**❌ Incorrecto:**
```json
{ "tool": "query_events", "params": { "session_id": "sess_abc123" } }
```
Llamar `query_events` sin filtros devuelve los primeros 100 eventos — que pueden no ser nada útil. Un programa típico genera millones de eventos. El agente recibe ruido, desperdicia tokens, y puede perder la señal.

**✅ Correcto:**
```json
{
  "tool": "query_events",
  "params": {
    "session_id": "sess_abc123",
    "event_types": ["function_entry", "function_exit"],
    "function_pattern": "process_*",
    "timestamp_start": 5000000000,
    "timestamp_end": 6000000000,
    "limit": 50
  }
}
```
Siempre aplica filtros. Comienza con herramientas de orientación (`get_execution_summary`, `debug_get_saliency_scores`) para reducir el alcance antes de consultar eventos crudos.

**Por qué importa:** Sin filtros, el agente recibe un slice arbitrario de ejecución — no necesariamente donde está el bug. Los filtros convierten `query_events` de un dump ruidoso en una sonda dirigida.

---

## Anti-Patrón 2: Llamadas Secuenciales Cuando Paralelo Es Posible

**❌ Incorrecto:** Llamadas JSON secuenciales. El agente espera cada respuesta antes de enviar la siguiente. Cuatro round-trips de latencia.

**✅ Correcto:** Enviar todos en paralelo en un solo batch:
```json
{
  "tool_batch": [
    { "tool": "get_execution_summary", "params": { "session_id": "sess_abc123" } },
    { "tool": "debug_get_saliency_scores", "params": { "session_id": "sess_abc123" } },
    { "tool": "list_threads", "params": { "session_id": "sess_abc123" } },
    { "tool": "debug_find_crash", "params": { "session_id": "sess_abc123" } }
  ]
}
```
Un round-trip. Todos los resultados disponibles simultáneamente. El agente sintetiza todo a la vez.

**Por qué importa:** La latencia de round-trip se compounding. Cuatro llamadas secuenciales a 100ms cada una = 400ms total. Cuatro llamadas paralelas = 100ms total.

---

## Anti-Patrón 3: Usar get_event en un Loop

**❌ Incorrecto:**
```python
# Pseudo-code: agente IA hace loop sobre event IDs
for event_id in suspicious_event_ids:
    result = call_tool("get_event", session_id=session_id, event_id=event_id)
    analyze(result)
```
Fetching un evento a la vez. Si hay 100 eventos sospechosos, eso son 100 round-trips.

**✅ Correcto:** Usar `query_events` con filtro de rango o patrón para obtener todos los eventos relevantes de una vez:
```json
{
  "tool": "query_events",
  "params": {
    "session_id": "sess_abc123",
    "event_types": ["function_entry"],
    "function_pattern": "suspicious_*",
    "timestamp_start": 5000000000,
    "timestamp_end": 6000000000,
    "limit": 100
  }
}
```

**Por qué importa:** Los loops de round-trips son el patrón más caro. Un loop de 100 iteraciones a 50ms por round-trip = 5 segundos. Una `query_events` filtrada = 50ms.

---

## Anti-Patrón 4: Descartar Sesiones Sin save_session

**❌ Incorrecto:**
```json
{ "tool": "debug_run", "params": { "program": "./service" } }
// ... análisis ... luego el agente se va
// Sesión perdida cuando el servidor reinicia
```

**✅ Correcto:**
```json
{ "tool": "debug_run", "params": { "program": "./service", "auto_save": true } }
// o explícitamente:
{ "tool": "save_session", "params": { "session_id": "sess_abc123", "language": "rust", "target": "./service" } }
```
Persistir la sesión. Ya sea usar `auto_save: true` en `debug_run` o llamar `save_session` después.

**Por qué importa:** Una sesión en memória se pierde al reiniciar el servidor. Si el trace es valioso — un incidente de producción, un baseline para CI, un bug raro — persístalo. El costo es insignificante (redb es un store embebido rápido). El valor es enorme.

---

## Anti-Patrón 5: Llamar Drill-Down Antes de Orientación

**❌ Incorrecto:**
```json
[
  { "tool": "get_call_stack", "params": { "session_id": "sess_abc123", "event_id": 12345 } },
  { "tool": "debug_get_variables", "params": { "session_id": "sess_abc123", "event_id": 12345 } },
  { "tool": "inspect_causality", "params": { "session_id": "sess_abc123", "address": 140734193800032 } }
]
```
El agente está llamando herramientas de inspección profunda en event IDs y direcciones arbitrarias que no ha validado como relevantes.

**✅ Correcto:** Siempre empezar con orientación:
```json
[
  { "tool": "get_execution_summary", "params": { "session_id": "sess_abc123" } },
  { "tool": "debug_get_saliency_scores", "params": { "session_id": "sess_abc123", "limit": 10 } },
  { "tool": "list_threads", "params": { "session_id": "sess_abc123" } }
]
```

**Por qué importa:** Sin orientación, el agente no tiene base para elegir qué event IDs y direcciones inspeccionar. Está adivinando. Las herramientas de orientación le dan al agente un mapa — funciones calientes, ubicación del crash, lista de threads — para dirigir el drill-down con precisión.

---

## Anti-Patrón 6: Usar state_diff / debug_diff Sin Saber Qué Comparar

**❌ Incorrecto:**
```json
{ "tool": "state_diff", "params": { "session_id": "sess_abc123", "timestamp_a": 1000000000, "timestamp_b": 2000000000 } }
```
El agente eligió dos timestamps aleatorios. La diff podría mostrar un cambio sin significado (ej. un incremento de stack pointer) y perder el bug real.

**✅ Correcto:** Usar `state_diff` solo después de que `debug_find_crash` o `query_events` reduce la ventana relevante:
```json
// Primero: encontrar el punto de crash
{ "tool": "debug_find_crash", "params": { "session_id": "sess_abc123" } }
// Respuesta: crash en event_id=45123, timestamp=5892341002

// Luego: diff del estado justo antes del crash
{ "tool": "debug_diff", "params": { "session_id": "sess_abc123", "event_id_a": 45120, "event_id_b": 45123 } }
```

**Por qué importa:** `state_diff` y `debug_diff` son herramientas de precisión. Usarlas en ventanas no validadas produce ruido y desperdicia contexto. Pertenecen al final de una cadena de investigación, no al principio.

---

## Anti-Patrón 7: Usar Modo Background Para Polling

**❌ Incorrecto:**
```json
{ "tool": "debug_run", "params": { "program": "./long-running-service", "background": true } }
// Luego polling:
while (true) {
    status = call_tool("get_session_status", session_id=session_id)
    if (status == "finalized") break
    sleep(1)
}
```
Esto es debugging interactivo disfrazado. El agente está haciendo polling, esperando que la captura termine.

**✅ Correcto:** Usar modo síncrono — espera automáticamente hasta completarse:
```json
{ "tool": "debug_run", "params": { "program": "./long-running-service" } }
// La respuesta llega cuando la captura está completa
```

**Cuando background SÍ es apropiado:** Servicios de larga duración donde quieres iniciar captura y hacer otro trabajo, pero solo si el agente tiene genuinamente otro trabajo que hacer. No para polling.

**Por qué importa:** Los loops de polling son antitéticos al modelo AI-native. El agente debe issuing un comando y esperar el resultado. `debug_run` síncrono es un round-trip. Polling es N round-trips hasta que se cumple una condición.

---

## Anti-Patrón 8: Usar debug_attach Cuando debug_run Funcionaría

**❌ Incorrecto:**
```json
{ "tool": "debug_attach", "params": { "pid": 12345 } }
```

**✅ Correcto:**
```json
{ "tool": "debug_run", "params": { "program": "./my-service", "args": ["--config", "prod.toml"] } }
```
Usar `debug_run` para capturar una ejecución completa desde el inicio. Trace completo, contexto completo, todos los eventos desde timestamp 0.

**Cuando debug_attach SÍ es apropiado:** Inspeccionar un proceso que ya está corriendo y no puede ser reiniciado. Debugging de producción donde no puedes reiniciar el servicio.

**Por qué importa:** `debug_attach` captura desde el momento del attach hacia adelante — no hay eventos de entrada para funciones ya en el stack, no hay contexto sobre qué pasó antes del attach.

---

## Anti-Patrón 9: Ignorar program_language Cuando Importa

**❌ Incorrecto:**
```json
{ "tool": "debug_run", "params": { "program": "my_script.py" } }
// Omite program_language
// Python auto-detectado pero faltan parámetros DAP
```

**✅ Correcto:**
```json
{ "tool": "debug_run", "params": {
    "program": "my_script.py",
    "program_language": "python",
    "debug_host": "127.0.0.1",
    "debug_port": 5678,
    "wait_for_connection": true
  }
}
```

**Por qué importa:** Sin `debug_port`, Chronos no sabe dónde conectar para Python/JavaScript. La captura retornará "pending" pero no producirá una sesión queryable.

---

## Referencia Rápida: Anti-Patrón → Patrón Correcto

| Anti-patrón | Patrón correcto |
|------------|----------------|
| `query_events` sin filtros | Siempre filtrar por event_types, function_pattern, o rango de tiempo |
| Llamadas secuenciales | Batch de todas las llamadas parallel-safe en un round-trip |
| `get_event` en loop | `query_events` con filtros para obtener N eventos de una vez |
| Sesión no persistida | Usar `auto_save: true` o llamar `save_session` |
| Drill-down antes de orientación | Siempre ejecutar herramientas de orientación primero |
| `state_diff` con timestamps aleatorios | Solo después de que `debug_find_crash` o `query_events` reduce el alcance |
| background + polling | Usar `debug_run` síncrono — bloquea hasta completar |
| `debug_attach` para procesos nuevos | Usar `debug_run` para traces completos |
| Python/JS sin `debug_port` | Siempre incluir `debug_port` + `wait_for_connection` para lenguajes interpretados |
