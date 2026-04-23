# Project Chronos: Motor de Observabilidad Dinámica para IA

## 1. El Problema del Paradigma Actual ("Record Everything")

Los debuggers de viaje en el tiempo (Time-Travel Debuggers) tradicionales intentan grabar **todo el estado** de un programa desde el inicio hasta el final. 

**Problemas para Agentes IA:**
1. **Saturación de Contexto (Context Choking):** Un LLM no puede procesar un volcado de memoria de 2GB. Ahogar al agente en datos inútiles reduce drásticamente su capacidad de razonamiento.
2. **Heisenberg Effect:** Capturar cada escritura de variable (`sys.settrace`, `ptrace` step-by-step) introduce un overhead inaceptable (2x-10x). Esto altera race conditions, rompe timeouts y oculta los bugs no deterministas en producción.
3. **Mantenimiento Insostenible:** Mantener adaptadores pesados (JDWP para Java, DAP para Python, Delve para Go) atados a los internals de cada runtime es un pozo sin fondo de deuda técnica.

---

## 2. La Nueva Visión: "Pesca con Arpón Láser"

Chronos debe evolucionar de ser una "Cámara de Seguridad Omnipresente" a un **"Microscopio Táctico Inteligente"**. En lugar de grabar el océano de datos pasivamente, le damos al LLM herramientas para inyectar observabilidad dinámica a velocidad nativa.

### Pilar A: "Tripwires" & Black Box (Cables Trampa y Caja Negra)
El programa corre a velocidad nativa (99% performance). Chronos mantiene un **Ring Buffer** continuo en memoria (ej. los últimos 5 segundos). 
El LLM inyecta condiciones (Tripwires) en el kernel vía eBPF. 
- **Ejemplo:** `"Si la variable 'total' es negativa, CONGELA el buffer y dímelo"`.
- Cuando salta la trampa, el LLM recibe exactamente el contexto de los 5 segundos previos al crash o anomalía, sin basura adicional.

### Pilar B: Observabilidad Dinámica (El Agente como Ciborg)
El LLM lee el código fuente estático y decide qué necesita saber.
- En lugar de pedir "Dame todas las variables", el LLM inyecta una sonda quirúrgica: `inject_probe(func="procesar_pago", expr="args[0]->monto")`.
- Chronos compila esto a eBPF al vuelo, lo inyecta en el kernel, extrae solo ese dato durante la ejecución, y devuelve el array de valores. Cero overhead en el resto del código.

### Pilar C: eBPF Semántico (El Lector de Mentes)
En lugar de usar `sys.settrace` en Python, eBPF lee la memoria del proceso desde el kernel.
- Conoce el layout de `PyObject` o de los `JVM Objects`.
- Puede leer diccionarios, strings y objetos del espacio de usuario directamente desde el kernel, de forma asíncrona y sin bloquear el runtime de la aplicación.

### Pilar D: "The Divergence Engine" (El Superpoder de la IA)
La técnica definitiva de debugging para un LLM no es analizar 10,000 líneas de un volcado. Es el juego de **"Encuentra las Diferencias"**.
- El Agente ejecuta el programa con un input exitoso (Golden Trace).
- El Agente ejecuta el programa con el input que falla.
- Invoca `compare_and_find_divergence()`.
- Chronos, usando su motor CAS (Content-Addressable Storage) con hashes BLAKE3, encuentra en O(1) la instrucción exacta o el bloque de memoria exacto donde las dos ejecuciones tomaron caminos distintos o generaron valores diferentes.
- Al LLM solo se le entrega el punto de divergencia. Resolución de bugs no deterministas en segundos.