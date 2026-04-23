# Arquitectura: Chronos como Event Bus Semántico para LLMs

## 1. El Problema de las Abstracciones Actuales
Actualmente, `chronos-ebpf` y `chronos-native` (ptrace) emiten eventos de muy bajo nivel (`TraceEvent`). Un LLM no piensa en términos de `0x400A2B` o de llamadas a `sys_read(3, buf, 1024)`. Un LLM piensa en lenguajes de alto nivel: "Entró a la función Java `procesarPago` con el argumento `usuario_id=45`".

## 2. La Solución: Arquitectura de Adaptadores de Dos Niveles

Chronos debe dividirse claramente en dos capas que trabajan en tándem como un "Event Bus Semántico".

### Capa 1: Kernel/Hardware Observers (C++ BPF / ptrace)
- **Responsabilidad:** Atrapar los eventos de ejecución a velocidad nativa sin bloquear el runtime.
- **Herramientas:** eBPF (uprobes, kprobes) y ptrace (solo cuando eBPF no está disponible).
- **Salida:** Ring buffers con eventos binarios rápidos (`[timestamp, thread_id, instruction_pointer, arg_registers]`).

### Capa 2: Semantic Resolvers (Python, Java, Go, JS, Rust)
- **Responsabilidad:** Escuchar el Event Bus y traducir las direcciones crudas a semántica de alto nivel.
- **Mecanismo:** Cada lenguaje tiene su "resolver".
    - Cuando el eBPF envía `uprobe_hit(0xCAFE)`, el **Rust Resolver** (usando `dwarf`) o el **Java Resolver** (usando JVMTI/JDI o JMAP) intercepta el evento.
    - El Resolver enriquece el evento: "Ah, `0xCAFE` es `UserService::login`. El registro `RDI` apunta a un objeto `User`. Leo la memoria de ese objeto (vía `/proc/pid/mem` o BPF helpers) y lo decodifico a JSON".
- **Salida:** Evento Semántico JSON listo para el LLM.

## 3. Adiós a las Pipelines de CI (Simplificación del MCP)
En el diseño original, Chronos intentaba ser una herramienta de CI (comparación de regresiones de rendimiento, orquestadores monolíticos).

**Decisión:** Eliminar las herramientas CI monolíticas (`performance_regression_audit`, `debug_orchestrate`).
**Por qué:** Los Agentes IA son los mejores "orquestadores". No necesitamos hardcodear un pipeline rígido en Rust. Solo necesitamos darle al Agente las primitivas (tools) correctas y dejar que él combine y deduzca.

- **Antes:** MCP Tool `debug_orchestrate(goal="find_crash")` ejecutaba un script interno rígido.
- **Ahora:** El Agente usa `debug_run()` -> `query_events(type="SignalDelivered")` -> `get_stack_trace(thread_id)` -> `get_variables(frame=0)`.

## 4. El Motor de Consultas On-Demand (On-Demand Query Engine)
El almacenamiento CAS infinito se reduce a favor de un **Ring Buffer en memoria** para contextos en tiempo real.
El LLM consulta Chronos como si fuera una base de datos de eventos SQL:
- "Dame todas las llamadas a funciones en el thread 5 donde el argumento 'user' era null".
- Chronos filtra esto en memoria (o vía eBPF si es un tripwire inyectado) y devuelve solo los resultados relevantes.

---
**Conclusión:** Chronos ya no es una "grabadora de caja negra" pasiva, sino un **órgano sensorial activo y semántico** que el LLM usa para palpar la ejecución del programa.