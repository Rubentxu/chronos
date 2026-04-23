# 08 — Gestión de Sesiones

Las sesiones son la unidad atómica de Chronos. Una sesión representa una captura de ejecución completa — inmutable, consultable indefinidamente, compartible entre agentes y a través del tiempo.

## El Ciclo de Vida de una Sesión

```
debug_run() ──► [sesión en memoria] ──► consultar, analizar, comparar
                      │
                      ├── save_session() ──► [persistido en disco]
                      │
                      ├── load_session() ◄── [cargar desde disco]
                      │
                      ├── drop_session() ──► [eliminado de memoria, queda en disco]
                      │
                      └── delete_session() ──► [eliminado de disco + memoria]
```

## En Memoria vs Persistido

**Solo en memoria** — creada por `debug_run` (modo síncrono). Disponible inmediatamente. Se pierde al reiniciar el servidor.

**Persistido** — guardado explícitamente con `save_session` o `auto_save: true` en `debug_run`. Sobrevive al reinicio del servidor. Cargable con `load_session`.

---

## save_session

Persiste una sesión en memoria al store persistente. Úsala después de que `debug_run` completa si quieres que el trace sobreviva reinicios del servidor o sea compartible.

**Parámetros:**
- `session_id` (string, requerido)
- `language` (string, requerido): `"python"`, `"rust"`, `"java"`, `"go"`, `"javascript"`, `"native"`
- `target` (string, requerido): ruta del programa o nombre

**Parallel-safe:** ✅ Sí

**Ejemplo:**
```json
{
  "tool": "save_session",
  "params": {
    "session_id": "sess_a1b2c3",
    "language": "rust",
    "target": "/usr/bin/my-service"
  }
}
```

---

## load_session

Carga una sesión previamente persistida del store a memoria.

**Parámetros:**
- `session_id` (string, requerido)

**Ejemplo:**
```json
{
  "tool": "load_session",
  "params": { "session_id": "sess_baseline_v2" }
}
```

---

## list_sessions

Lista todas las sesiones persistidas en el store.

**Parámetros:** Ninguno

**Ejemplo:**
```json
{ "tool": "list_sessions", "params": {} }
```

---

## delete_session

Elimina una sesión de AMBOS el store persistente Y memoria. **Operación destructiva, irreversible.**

**Parámetros:**
- `session_id` (string, requerido)

**⚠️ Warning:** No hay soft-delete. Una vez eliminada, la sesión no se puede recuperar.

---

## drop_session

Elimina una sesión de la **memoria únicamente** sin tocar el store persistente. Idempotente — seguro de llamar incluso si la sesión ya no está en memoria.

**Parámetros:**
- `session_id` (string, requerido)

**Diferencia clave:**
- `drop_session` → elimina de memoria, datos sobreviven en store
- `delete_session` → elimina de AMBOS memoria y store (permanente)

---

## compare_sessions

Realiza una diff hash entre dos sesiones. Devuelve qué eventos son únicos en cada sesión y cuáles son compartidos.

**Parámetros:**
- `session_a` (string, requerido)
- `session_b` (string, requerido)

**Ejemplo:**
```json
{
  "tool": "compare_sessions",
  "params": {
    "session_a": "sess_baseline_v2",
    "session_b": "sess_current_pr"
  }
}
```

---

## Patrón CI/CD: Regression Gate

El workflow de gestión de sesiones más poderoso para agentes IA:

```
1. debug_run()     → capturar sesión baseline → save_session("baseline_v1")
2. [cambios de código]
3. debug_run()     → capturar sesión actual
4. compare_sessions() → baseline vs actual
5. performance_regression_audit() → comparación detallada
   Si regression_score > threshold → FALLAR CI
```

---

## Patrón Multi-Agente

Las sesiones habilitan workflows sofisticados multi-agente:

```
Agente A (CI/CD runner):
  debug_run() → save_session("build_${GIT_SHA}") → store

Agente B (análisis on-call):
  load_session("build_${GIT_SHA}") → analizar trace

Agente C (comparar):
  load_session("build_main") → compare_sessions("build_main", "build_${GIT_SHA}")
```

Las sesiones sobreviven al agente que las creó. Cualquier agente puede cargar y consultar cualquier sesión persistida por ID.

---

## Auto-Save

En lugar de llamar manualmente `save_session`, pasa `auto_save: true` a `debug_run`:

```json
{
  "tool": "debug_run",
  "params": {
    "program": "/usr/bin/my-service",
    "auto_save": true,
    "program_language": "rust"
  }
}
```

La sesión se persiste automáticamente al store después de completar la captura.

---

## Ubicación de Almacenamiento

Por defecto, las sesiones se almacenan en:
```
~/.local/share/chronos/sessions.redb
```

Override con la variable de entorno `CHRONOS_DB_PATH`:

```bash
CHRONOS_DB_PATH=/var/lib/chronos/sessions.redb chronos-mcp
```

El store usa `redb`, una base de datos clave-valor embebida — no requiere servidor de base de datos externo.

---

## Tabla Resumen

| Herramienta | Objetivo | Persiste | Idempotente | Parallel-safe |
|------------|---------|----------|-------------|---------------|
| `save_session` | Memória → store | Sí | No (sobreescribe) | Sí |
| `load_session` | Store → memória | No | No (error si no existe) | Sí |
| `list_sessions` | Store | No | Sí | Sí |
| `delete_session` | Ambos | Elimina | No (error si no existe) | Sí |
| `drop_session` | Solo memória | No | Sí | Sí |
| `compare_sessions` | Ambos | No | Sí | Sí |
