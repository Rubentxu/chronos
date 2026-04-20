# Project Chronos: Arquitectura Consolidada del MCP Debugger Server

**Versión**: 1.0  
**Fecha**: 2026-04-20  
**Estado**: Arquitectura Técnica Consolidada  
**Autores**: Basado en investigación previa de MCP Debugging y análisis de debugging por lenguajes

---

## Resumen Ejecutivo

Este documento presenta la arquitectura técnica consolidada para **Project Chronos**, un servidor MCP (Model Context Protocol) que implementa **Time-Travel Debugging** accesible a agentes de IA. El enfoque central es transformar la ejecución de programas en una **base de datos temporal consultable**, donde un agente de IA puede interrogar el historial completo de ejecución sin las limitaciones del debugging tradicional interactivo.

**Visión del Proyecto**: Convertir cualquier programa en ejecución en un dataset consultable, accesible mediante herramientas MCP que permiten a agentes de IA responder preguntas como:
- "¿Qué valor tenía la variable `x` en el timestamp 5.2s?"
- "¿Quién escribió en esta dirección de memoria?"
- "¿Cuál fue el call stack completo en el momento del crash?"
- "¿En qué syscalls se usó este file descriptor?"

Este documento consolida dos líneas de investigación previas (arquitectura MCP y análisis de debugging multi-lenguaje) y añade **nueva investigación sobre mecanismos de captura por lenguaje**, expresión evaluación multi-lenguaje, seguridad y sandboxing, y persistencia histórica.

---

## PARTE I: DIAGNÓSTICO — Por qué los Debuggers Actuales Fallan para IA

### 1.1 El Modelo Mental Humano vs. Agente IA

Los debuggers modernos fueron diseñados para un flujo de trabajo fundamentalmente humano:

```
HUMANO → BREAKPOINT → PARA → INSPECCIONA → CONTINÚA → PARA → INSPECCIONA
   ↑         ↑         ↑         ↑            ↑         ↑         ↑
  decide   decide    espera   mira la        decide   espera   procesa
  dónde    cuándo     para    pantalla        siguiente  para     datos
  parar    parar              decides                   siguiente
```

Este modelo tiene supuestos fundamentales que clash con cómo una IA interactúa con código:

| Aspecto | Debugger Tradicional | Agente IA |
|---------|---------------------|-----------|
| **Temporalidad** | Síncrono, interactivo | Asíncrono, eventual |
| **Estado mental** | Atención sostenida humana | Procesamiento de trazas |
| **Interacción** | Request-response inmediato | Consulta diferida |
| **Granularidad** | Control de instrucciones | Análisis de trazas completas |
| **Memoria** | Estado actual visible | Historia completa de ejecución |
| **Reproducibilidad** | Depende de breakpoints | Completa (traza guarda todo) |
| **Queries** | No posibles (solo estado actual) | Flexibles y potentes (por tiempo, addr, tipo) |

### 1.2 El Estado del Arte de AI Debugging (2025-2026)

Las herramientas actuales de "AI debugging" operan todas bajo el mismo paradigma:

```
ERROR → LEE → ANALIZA → GENERA → APLICA → REPITE
  ↑                                         │
  └─────────────────────────────────────────┘
  Todas siguen este flujo. Ninguna inspecciona.
```

**Herramientas analizadas:**

| Herramienta | Capacidad | Limitación Fundamental |
|-------------|-----------|----------------------|
| **GitHub Copilot Debug** | Analiza errores, sugiere fixes | No tiene acceso al estado runtime |
| **Cursor AI Debug** | Lee stack traces, navega código | No establece breakpoints dinámicamente |
| **Claude Code** | Analiza errores en contexto | No puede evaluar expresiones en contexto real |
| **JetBrains AI Assistant** | Integración con debugger | No puede autonombrar breakpoints |
| **Sweep/Devin** | Regeneración de código en loop | No hace debugging real, es trial-and-error |

**La falacia del "AI Debugging":**

```
Lo que la gente dice:        "Mi IDE tiene IA que hace debugging"
Lo que eso significa:         "La IA puede leer errores y sugerirme código"
Lo que la gente espera:      "La IA encuentra y corrige bugs como un developer experto"
Lo que realmente pasa:       "La IA aplica patrones de corrección conocidos a errores
                              que reconoce de su training data"
```

### 1.3 El Gap Fundamental: Inspection vs. Guess

```
╔═══════════════════════════════════════════════════════════════════════════════╗
║                    ARQUITECTURA TRADICIONAL DE DEBUGGING                      ║
╠═══════════════════════════════════════════════════════════════════════════════╣
║                                                                               ║
║   ┌──────────┐      ┌──────────┐      ┌──────────┐      ┌──────────┐      ║
║   │  HUMAN   │ ←──→ │  EDITOR  │ ←──→ │   DAP    │ ←──→ │ DEBUGGER │      ║
║   │  ( eyes) │      │          │      │  SERVER  │      │          │      ║
║   └──────────┘      └──────────┘      └──────────┘      └──────────┘      ║
║                                                                               ║
║   El humano decide dónde parar, qué inspeccionar                              ║
║   El debugger PARA y espera instrucciones                                   ║
║   DAP: request-response síncrono, stateful sessions                         ║
║                                                                               ║
╠═══════════════════════════════════════════════════════════════════════════════╣
║                    ARQUITECTURA PROPUESTA PARA IA                            ║
╠═══════════════════════════════════════════════════════════════════════════════╣
║                                                                               ║
║   ┌──────────┐      ┌──────────┐      ┌──────────────┐      ┌──────────┐   ║
║   │    AI    │ ──── │  MCP     │ ──── │   TRACE      │ ←─── │  TARGET  │   ║
║   │  AGENT   │      │  SERVER  │      │  RECORDER    │      │ PROGRAM  │   ║
║   └──────────┘      └────┬─────┘      └──────────────┘      └──────────┘   ║
║                           │                    │                            ║
║                           │   ┌─────────────────┘                            ║
║                           │   │                                              ║
║                           ▼   ▼                                              ║
║                    ┌──────────────┐                                         ║
║                    │  QUERY       │ ←── POST-MORTEM                          ║
║                    │  ENGINE      │                                          ║
║                    └──────────────┘                                         ║
║                                                                               ║
║   El programa se EJECUTA COMPLETO (fire-and-forget)                          ║
║   TODO se graba en la traza                                                 ║
║   La IA consulta la traza después de terminar                                ║
║                                                                               ║
╚═══════════════════════════════════════════════════════════════════════════════╝
```

**La diferencia entre debugging reactivo y proactivo:**

```
DEBUGGING REACTIVO (hoy):          DEBUGGING PROACTIVO (mañana):
Error → Lee → Adivina → Fix        Hipótesis → Investiga → Inspecciona → Verifica
                                                             ↑
                                                          Con datos reales
```

### 1.4 Por qué DAP No Es Suficiente para IA

DAP (Debug Adapter Protocol) fue diseñado para bridges entre editores y debuggers, con operaciones core que reflejan esta herencia:

```typescript
// DAP: Modelo request-response síncrono
interface Request {
  command: string;       // "next", "stepIn", "stackTrace", "scopes"
  arguments: any;
  seq: number;
}

// Cada operación requiere una respuesta inmediata
// La IA debe esperar antes de poder continuar
```

**Problemas para IA:**

1. **Latencia de red**: Cada "step" es un round-trip. Para 1000 pasos, son 1000 round-trips
2. **Estado efímero**: Solo existe el estado actual; el pasado se pierde
3. **Breakpoints como única herramienta**: No hay forma de "rebobinar" o ver todos los valores que una variable tuvo
4. **Sin queries históricos**: "Dame todas las veces que `x` fue asignado" requiere instrumentación manual
5. **Stateful**: DAP mantiene estado del debugger (breakpoints, threads, stack frames)

**Comparación técnica DAP vs. MCP Debugger:**

| Aspecto | DAP | MCP Debugger (Chronos) |
|---------|-----|------------------------|
| **Paradigma** | Request-response síncrono | Tool resolution |
| **Estado** | Stateful debugger session | Stateless queries sobre traza |
| **Interacción** | Breakpoints, stepping | Traza grabada, consultas |
| **Temporalidad** | Tiempo real, interactivo | Post-mortem |
| **Para IA** | Incorrecto (síncrono) | Correcto (asíncrono) |
| **Latencia** | Bloqueante por step | No-bloqueante |
| **Reproducibilidad** | Depende de breakpoints | Exacta (traza guardada) |

---

## PARTE II: EL PARADIGMA — Record and Analyze

### 2.1 La Caja Negra de Vuelo (Flight Data Recorder)

La inspiración fundamental viene de la **aviación**. Los aviones no se detienen en el aire para revisar el motor; graban TODO durante el vuelo y los investigadores analizan los datos después de un accidente.

```
┌─────────────────────────────────────────────────────────────────────────┐
│                     FLIGHT DATA RECORDER                                  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  DURING FLIGHT: Recording 24/7                                 │   │
│   │                                                                  │   │
│   │   - Altitude, speed, heading                                     │   │
│   │   - Engine parameters                                            │   │
│   │   - Control inputs                                               │   │
│   │   - Cockpit audio                                                │   │
│   │   - All sensor readings                                          │   │
│   │                                                                  │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                              │                                           │
│                              ▼                                           │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │  AFTER INCIDENT: Analysis                                       │   │
│   │                                                                  │   │
│   │   - Reconstruct last 30 minutes                                  │   │
│   │   - Query any parameter at any timestamp                         │   │
│   │   - Find root cause by correlation                               │   │
│   │   - No need to be there when it happened                        │   │
│   │                                                                  │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.2 Time-Travel Debugging como Base de Datos Temporal

El debugging tradicional pregunta: "¿En qué estado está el programa ahora?"

El nuevo paradigma pregunta: "¿Cuál era el estado del programa en cualquier timestamp del pasado?"

**La metáfora de base de datos:**

```
┌─────────────────────────────────────────────────────────────────────────┐
│                   EJECUCIÓN = DATASET DE EVENTOS                        │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   La traza de ejecución es una tabla donde:                             │
│                                                                          │
│   ┌─────────────────────────────────────────────────────────────────┐   │
│   │ timestamp | thread_id | event_type | address | value | ...    │   │
│   │───────────┼───────────┼─────────────┼──────────┼─────────────│   │
│   │ 0.000001  |     1     │ SYSCALL     | 0x400000  │ read()      │   │
│   │ 0.000002  |     1     │ MEMORY_WRITE| 0x601000  │ 0xFF        │   │
│   │ 0.000003  |     1     │ FUNCTION_ENT| 0x401000  │ main()      │   │
│   │ ...       |    ...     │ ...         │ ...       │ ...         │   │
│   └─────────────────────────────────────────────────────────────────┘   │
│                                                                          │
│   La IA puede hacer queries SQL-like sobre esta tabla:                   │
│                                                                          │
│   SELECT * FROM trace WHERE timestamp BETWEEN 1.0 AND 2.0              │
│   SELECT * FROM trace WHERE event_type = 'MEMORY_WRITE'                  │
│   SELECT * FROM trace WHERE address = 0x601000                          │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.3 Lazy Loading: Summary → Hotspot → Detail

Para manejar trazas masivas (millones de eventos), el sistema implementa carga lazy:

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    LAZY LOADING STRATEGY                                 │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│   NIVEL 0: RESUMEN EJECUTIVO                                            │
│   ═══════════════════════════                                           │
│   - Duración total de ejecución                                          │
│   - Número de syscalls totales                                           │
│   - Lista de funciones más llamadas                                      │
│   - Memoria total asignada                                               │
│   - Posibles crash points                                                │
│                                                                          │
│                           ▼                                              │
│   NIVEL 1: HOTSPOT IDENTIFICATION                                       │
│   ═════════════════════════════════                                     │
│   - Direcciones de memoria más escritas                                  │
│   - Funciones con más asignaciones de memoria                            │
│   - Syscalls más frecuentes                                             │
│   - Timestamps de events anómalos                                       │
│                                                                          │
│                           ▼                                              │
│   NIVEL 2: DETALLE DE HOTSPOT                                           │
│   ═══════════════════════════════                                        │
│   - Todos los eventos en un rango de timestamps                          │
│   - Línea completa de call stack en cada evento                          │
│   - Valores de todas las variables en ese momento                        │
│                                                                          │
│                           ▼                                              │
│   NIVEL 3: MICROSCOPIA                                                   │
│   ══════════════════════                                                 │
│   - Contenido de memoria en dirección específica                         │
│   - Registros de CPU completos                                          │
│   - Estado de cada variable individual                                   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

### 2.4 Arquitectura de Flujo: Record → Index → Query

```
┌─────────────────────────────────────────────────────────────────────────┐
│              CICLO DE VIDA: RECORD AND ANALYZE                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  FASE 1: RECORD (Grabación)                                              │
│  ═══════════════════════════                                             │
│                                                                          │
│  ┌─────────────┐      ptrace/syscalls                                    │
│  │ TARGET      │ ←─── registers                                         │
│  │ PROGRAM     │ ←─── memory snapshots                                  │
│  │             │ ←─── breakpoints                                        │
│  └──────┬──────┘                                                        │
│         │                                                                │
│         │ Capture                                                         │
│         │ Events                                                         │
│         ▼                                                                │
│  ┌─────────────┐      ┌─────────────┐                                   │
│  │  RAW TRACE │ ───→ │  INDEXER    │                                   │
│  │  (binary)  │      │  (hotspots) │                                   │
│  └─────────────┘      └──────┬──────┘                                   │
│                               │                                           │
│  FASE 2: ANALYZE (Análisis)                                              │
│  ═══════════════════════════════                                         │
│                                                                          │
│  ┌─────────────┐      ┌─────────────┐      ┌─────────────┐             │
│  │ AI AGENT    │ ───→ │ MCP SERVER  │ ───→ │ QUERY       │             │
│  │             │      │             │      │ ENGINE      │             │
│  └─────────────┘      └──────┬──────┘      └──────┬──────┘             │
│                              │                     │                     │
│                              │    ┌────────────────┘                     │
│                              │    │                                       │
│                              ▼    ▼                                       │
│                       ┌─────────────┐                                    │
│                       │ TRACE FILE  │                                    │
│                       │ (indexed)   │                                    │
│                       └─────────────┘                                    │
│                                                                          │
│  La IA hace PREGUNTAS sobre la ejecución                                 │
│  El motor responde con DATOS de la traza                                 │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## PARTE III: MECANISMOS DE CAPTURA POR LENGUAJE

Esta es la sección más crítica del documento. Cada lenguaje tiene mecanismos únicos para la captura de trazas, y el diseño debe abstraer estas diferencias en un formato unificado.

### 3.1 Lenguajes Nativos: C, C++, Rust

**Mecanismo primario: ptrace**

`ptrace` (process trace) es el syscall central de debugging en sistemas Unix/Linux:

```c
// Indica que este proceso quiere ser trazado por su padre
ptrace(PTRACE_TRACEME, 0, NULL, NULL);

// Continúa la ejecución después de un stop
ptrace(PTRACE_CONT, pid, NULL, NULL);

// Ejecuta una sola instrucción y vuelve a parar
ptrace(PTRACE_SINGLESTEP, pid, NULL, NULL);

// Lee un word de memoria del proceso destino
long data = ptrace(PTRACE_PEEKDATA, pid, addr, NULL);

// Obtiene registros de CPU
ptrace(PTRACE_GETREGS, pid, NULL, &regs);

// Adjunta a un proceso existente
ptrace(PTRACE_ATTACH, pid, NULL, NULL);
```

**Mecanismo alternativo: eBPF uprobes (no intrusivo, zero-overhead)**

```bash
# Ejemplo con bpftrace
bpftrace -e 'uprobe:/lib/x86_64-linux-gnu/libc.so.6:printf { 
    printf("printf called with %s\n", str(arg2)); 
}'
```

**Arquitectura de eBPF:**

```
┌─────────────┐    ┌──────────────┐    ┌─────────────────┐
│ User Space  │    │  Kernel       │    │  eBPF Program   │
│ bpftrace    │───→│  Verifier     │───→│  (bytecode)     │
│ bcc tool    │    │  + JIT        │    │  attached to    │
└─────────────┘    └──────────────┘    │  kprobe/uprobe  │
                                        └─────────────────┘
```

**Resolución de símbolos: DWARF**

DWARF es el formato estándar para información de depuración en binarios ELF:

```
ELF File:
┌─────────────────────────────────────────────────────────────┐
│ ELF Header                                                 │
├─────────────────────────────────────────────────────────────┤
│ .debug_abbrev    — Tabla de abreviaturas                   │
│ .debug_info      — Unidades de compilación (CU)            │
│ .debug_line      — Tablas de números de línea              │
│ .debug_str       — Strings (nombres de variables)          │
│ .debug_ranges    — Rangos de direcciones                  │
│ .debug_frame     — Información de unwinding                │
├─────────────────────────────────────────────────────────────┤
│ .symtab          — Tabla de símbolos                       │
│ .strtab          — Strings de símbolos                     │
└─────────────────────────────────────────────────────────────┘
```

**Expresiones de ubicación DWARF:**

```cpp
// Variable local "i" en el stack frame
DW_AT_location: DW_OP_fbreg -20
// Significa: frame base - 20 bytes

// Parámetro en registro
DW_AT_location: DW_OP_reg0 (RAX en x86-64)

// Variable global
DW_AT_location: DW_OP_addr 0x604050

// Expresión compleja (desreferencia)
DW_AT_location: DW_OP_deref, DW_OP_reg0
```

**Evaluación de expresiones:**

1. DWARF location expression → dirección de memoria
2. Leer bytes de memoria en esa dirección
3. Cast al tipo correspondiente usando la información de tipo DWARF
4. Devolver valor tipado

**Limitaciones:**

- `ptrace` causa 2-10x de slowdown
- eBPF requiere kernel 4.x+ y root/capabilities
- Variables optimizadas-out no están disponibles
- No hay acceso a variables de lenguajes interpretados

**Herramientas existentes:** rr (Mozilla), GDB, LLDB, UndoDB

**Enfoque propuesto:** ptrace para captura completa de trace + eBPF para probes específicos

### 3.2 Java / JVM Languages: Java, Kotlin, Scala

**Mecanismo primario: JVMTI (JVM Tool Interface)**

JVMTI es una interfaz nativa que permite implementar agentes de depuración:

```java
// Agent_OnLoad: Called cuando el agente se carga
JNIEXPORT jint JNICALL
Agent_OnLoad(JavaVM *vm, char *options, void *reserved) {
    jvmtiEnv *jvmti;
    (*vm)->GetEnv(vm, (void **)&jvmti, JVMTI_VERSION_1_0);
    
    // Habilitar eventos
    jvmtiEventCallbacks callbacks = {
        .Breakpoint = &handle_breakpoint,
        .SingleStep = &handle_single_step,
        .Exception = &handle_exception,
        .FieldModification = &handle_field_mod,
        .MethodEntry = &handle_method_entry,
        .MethodExit = &handle_method_exit
    };
    (*jvmti)->SetEventCallbacks(jvmti, &callbacks, sizeof(callbacks));
    
    // Habilitar eventos específicos
    (*jvmti)->SetEventNotificationMode(jvmti, JVMTI_ENABLE, 
        JVMTI_EVENT_BREAKPOINT, NULL);
    
    return JNI_OK;
}
```

**Eventos JVMTI disponibles:**

| Evento | Descripción |
|--------|-------------|
| `Breakpoint` | Breakpoint alcanzado |
| `SingleStep` | Cada instrucción |
| `Exception` | Excepción lanzada/capturada |
| `FieldModification` | Campo modificado |
| `FieldAccess` | Campo leído |
| `MethodEntry` | Entrada a método |
| `MethodExit` | Salida de método |
| `ThreadStart/End` | Inicio/fin de thread |
| `ClassLoad/Unload` | Clase cargada/descargada |

**Alternativa: Java Agent + Instrumentation API (bytecode-level)**

```java
// Usando java.lang.instrument
public static void premain(String agentArgs, Instrumentation inst) {
    inst.addTransformer(new ClassFileTransformer() {
        @Override
        public byte[] transform(ClassLoader loader, String className,
                Class<?> classBeingRedefined,
                ProtectionDomain protectionDomain,
                byte[] classfileBuffer) {
            // Modificar bytecode: insertar logging en cada método
            return modifyBytecode(classfileBuffer);
        }
    });
}
```

**Resolución de símbolos:**

JVMTI proporciona acceso directo a metadatos de clase:

```java
// Obtener campos de una clase
jvmtiError GetClassFields(jvmtiEnv* env, jclass klass, 
                           jint* fieldCount, jfieldID** fields);

// Obtener métodos de una clase  
jvmtiError GetClassMethods(jvmtiEnv* env, jclass klass,
                           jint* methodCount, jmethodID** methods);

// Obtener nombre y firma de un campo
jvmtiError GetFieldName(jvmtiEnv* env, jclass klass, jfieldID field,
                        char** name, char** signature, char** generic);
```

**Evaluación de expresiones: JDI (Java Debug Interface)**

JDI permite evaluar expresiones en el contexto del VM debuggeado:

```java
// En el debugger (no en el target VM)
VirtualMachine vm = ... // Conexión al target VM

// Crear un stack frame reference
StackFrame frame = thread.frames().get(0);

// Evaluar expresión en ese frame
Value result = frame.thisObject().getValue(
    vm.classesByName("com.example.MyClass").get(0)
        .fieldByName("calculateTotal")
);

// O usando el evaluador de expresiones
StringExpression expr = new StringExpression("x + y * 2");
Value result = frame.evaluate(expr);
```

**Arquitectura JVMTI/JDI:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    JVM Target                                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    JVM TI (Native Agent)                  │   │
│  │  - Escucha eventos de debug                               │   │
│  │  - Controla ejecución                                     │   │
│  │  - Acceso a memoria de objetos                            │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
                              │ JDWP (Java Debug Wire Protocol)
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Debugger (Chronos Adapter)                    │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │                    JDI (Java Debug Interface)            │   │
│  │  - Evalúa expresiones en el target VM                    │   │
│  │  - Lee variables, stack frames                            │   │
│  │  - Control breakpoints                                   │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

**Limitaciones:**

- JVMTI requiere agente cargado al inicio o via Attach API
- Expression eval corre en el target VM (riesgo de seguridad)
- No hay acceso ptrace-level a memoria (heap gestionado por GC)
- GC puede mover objetos (addresses no son fijas)

**Herramientas existentes:** Chronon (record-replay), JDB, IntelliJ debugger

**Enfoque propuesto:** Agente JVMTI que graba method entry/exit, field modifications, y thread events en formato de traza. Expression eval via JDI.

### 3.3 Python

**Mecanismo primario: sys.settrace()**

Python proporciona un mecanismo de tracing integrado:

```python
import sys

def trace_func(frame, event, arg):
    """
    trace_func es llamada por Python en cada:
    - 'call': cuando se llama una función
    - 'line': cuando se ejecuta una nueva línea
    - 'return': cuando una función retorna
    - 'exception': cuando ocurre una excepción
    - 'c_call': llamada a función C
    - 'c_return': retorno de función C
    """
    if event == 'line':
        # frame.f_code.co_filename: nombre del archivo
        # frame.f_lineno: línea actual
        # frame.f_locals: variables locales
        filename = frame.f_code.co_filename
        lineno = frame.f_lineno
        locals_copy = frame.f_locals.copy()  # Copia para evitar mutaciones
        globals_copy = frame.f_globals.copy()
        
        # Grabar evento en traza
        emit_trace_event(
            event='line',
            filename=filename,
            lineno=lineno,
            locals=locals_copy,
            timestamp=get_timestamp_ns()
        )
        
    elif event == 'call':
        func_name = frame.f_code.co_name
        # frame.f_locals contiene los argumentos
        emit_trace_event(event='call', func_name=func_name, args=frame.f_locals)
        
    elif event == 'return':
        emit_trace_event(event='return', return_value=arg)
        
    return trace_func  # Important: return self to continue tracing

# Activar tracing
sys.settrace(trace_func)

# Código a trazar
def foo():
    x = 10
    return x

foo()

# Desactivar
sys.settrace(None)
```

**Alternativa: sys.monitoring (Python 3.12+)**

Python 3.12 introdujo una API de monitoring de bajo overhead:

```python
import sys.monitoring

# Obtener IDs para eventos
CALL_EVENT = sys.monitoring.events.CALL
LINE_EVENT = sys.monitoring.events.LINE

def my_callback(func, arg):
    # Handle call event
    pass

# Registrar callback
sys.monitoring.use_tool_id("my_tool")
sys.monitoring.register_callback("my_tool", CALL_EVENT, my_callback)
sys.monitoring.set_events("my_tool", CALL_EVENT)
```

**Captura de variables:**

```python
# Los frames de Python contienen toda la información
frame.f_locals    # Variables locales (dict)
frame.f_globals   # Variables globales (dict)  
frame.f_code      # Code object (nombre, bytecode, etc.)
frame.f_lineno    # Línea actual
frame.f_back      # Frame anterior (padre)

# Ejemplo: capturar todos los valores de variables
def capture_frame(frame):
    variables = {}
    for name, value in frame.f_locals.items():
        variables[name] = {
            'type': type(value).__name__,
            'value': repr(value),
            'id': id(value)  # Para tracking de objetos
        }
    return variables
```

**Evaluación de expresiones:**

Python permite evaluación nativa en el contexto de un frame:

```python
# eval() en el contexto del frame
def evaluate_in_frame(frame, expression):
    # Crear un namespace combinado
    namespace = {}
    namespace.update(frame.f_globals)
    namespace.update(frame.f_locals)
    
    result = eval(expression, namespace)
    return result

# Ejemplo
def on_breakpoint(frame):
    result = evaluate_in_frame(frame, "x + y * 2")
    print(f"x + y * 2 = {result}")
```

**Limitaciones:**

- sys.settrace añade ~2-5x overhead
- GIL limita concurrent tracing
- No hay acceso directo a memoria/registros
- Para C extensions, hay que caer a ptrace

**Herramientas existentes:** pdb, PyTrace, reptile (record-replay)

**Enfoque propuesto:** Wrapper de sys.settrace que captura frame events en formato de traza. Expression eval via eval() en contexto de frame.

### 3.4 JavaScript / Node.js

**Mecanismo primario: V8 Inspector Protocol (Chrome DevTools Protocol)**

V8 expone un protocolo de debugging basado en WebSocket:

```bash
# Iniciar Node.js con inspector
node --inspect mi_script.js

# Conectar con Chrome DevTools:
# 1. Abre chrome://inspect
# 2. Click en "Open dedicated DevTools for Node"
```

**Arquitectura CDP:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    Chrome DevTools / VS Code                     │
│                   Usa CDP (Chrome DevTools Protocol)             │
└─────────────────────────────┬───────────────────────────────────┘
                              │ WebSocket
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    V8 Inspector (en Node.js/Chrome)               │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  V8 Debugger                                              │   │
│  │  - Maneja breakpoints                                     │   │
│  │  - Collects stack traces                                  │   │
│  │  - Evalua expresiones en contexto                         │   │
│  └──────────────────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  JS Execution Engine                                      │   │
│  │  - JIT compilation                                        │   │
│  │  - Garbage collection                                    │   │
│  └──────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

**Conexión directa al protocolo:**

```javascript
const { Runtime, Debugger, Profiler } = require('inspector');
const session = new Runtime.Session();

// Conectar
session.connect();

// Habilitar debugger
await session.post('Debugger.enable');

// Establecer breakpoint por URL
await session.post('Debugger.setBreakpointByUrl', {
    lineNumber: 10,
    url: 'file:///path/to/script.js'
});

// Habilitar detección de async calls
await session.post('Debugger.enableAsyncStackTraceOperation');

// Cuando el breakpoint se dispara:
session.on('Debugger.paused', (params) => {
    // params.callFrames contiene el stack trace
    // Cada frame tiene scopeChain con variables
    for (const frame of params.callFrames) {
        console.log(`Function: ${frame.functionName}`);
        for (const scope of frame.scopeChain) {
            console.log(`  Scope: ${scope.type}`);
            console.log(`  Variables: ${JSON.stringify(scope.object)}`);
        }
    }
});
```

**Captura de scope chain:**

```javascript
// El scope chain de V8 proporciona acceso directo a variables
{
    "callFrameId": "0",
    "functionName": "mi_funcion",
    "location": { "scriptId": "1", "lineNumber": 10 },
    "scopeChain": [
        { 
            "type": "local",
            "object": {
                "type": "object",
                "className": "Object",
                "description": "Object"
            }
        },
        { 
            "type": "closure",
            "object": { ... }
        },
        { 
            "type": "global",
            "object": { ... }
        }
    ],
    "this": { "type": "undefined" }
}
```

**Evaluación de expresiones:**

```javascript
// Evaluar JS en el contexto de un call frame pausado
const response = await session.post('Debugger.evaluateOnCallFrame', {
    callFrameId: "0",  // ID del frame
    expression: "x + y * 2",
    returnByValue: true
});

console.log(response.result);  // { type: 'number', value: 42 }

// También en contexto global
const globalResult = await session.post('Runtime.evaluate', {
    expression: "process.env.NODE_ENV",
    returnByValue: true
});
```

**Streaming de eventos:**

Para grabar trazas, se pueden suscribir a eventos sin pausar:

```javascript
// Suscribirse a eventos de debugger sin pausa
await session.post('Debugger.setSkipAllPauses', { skip: true });

// Listener para function entry/exit
session.on('Debugger.functionEntry', (params) => {
    emit_trace_event({
        type: 'function_entry',
        name: params.name,
        url: params.url
    });
});

session.on('Debugger.functionExit', (params) => {
    emit_trace_event({
        type: 'function_exit', 
        returnValue: params.returnValue
    });
});
```

**Limitaciones:**

- Requiere flag --inspect
- Solo funciona cuando está pausado (no async recording)
- No hay acceso a syscall-level
- No hay acceso a memoria cruda (JS es sandboxed)

**Herramientas existentes:** Chrome DevTools, node --inspect, Replay.io

**Enfoque propuesto:** Attach via V8 Inspector Protocol, set breakpoints at function entries, capture scope snapshots. Use CDP streaming para live observation.

### 3.5 Go

**Mecanismo primario: Delve debugger API**

Delve es el debugger nativo de Go y expone API de alto nivel:

```bash
# Instalar
go install github.com/go-delve/delve/cmd/dlv@latest

# Iniciar con API
dlv debug mi_programa.go

# O adjuntar a proceso existente
dlv attach 12345
```

**Arquitectura de Delve:**

```
┌─────────────────────────────────────────────────────────────────┐
│                         Delve (dlv)                              │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐         │
│  │  Commands    │  │   RPC       │  │  Targets     │         │
│  │  (CLI/JSON)  │  │  Server     │  │  (proceso)   │         │
│  └──────────────┘  └──────────────┘  └──────────────┘         │
│                          │                                      │
│                          ▼                                      │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │              Target (Go Process)                          │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │  │
│  │  │  runtime      │  │   Go         │  │   Native     │  │  │
│  │  │  debugger     │  │   debugger   │  │   (ptrace)   │  │  │
│  │  │  hooks        │  │   logic      │  │              │  │  │
│  │  └──────────────┘  └──────────────┘  └──────────────┘  │  │
│  └──────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────┘
```

**Delve API para captura de traza:**

```go
// Conectar a Delve programáticamente
import (
    "github.com/go-delve/delve/service/rpc"
)

func createTraceSession(binary string) (*rpc.Client, error) {
    client := rpc.NewClient("localhost:12345")
    return client, nil
}

// Listar goroutines
func listGoroutines(client *rpc.Client) ([]*api.Goroutine, error) {
    return client.ListGoroutines()
}

// Obtener state de un thread
func getThreadState(client *rpc.Client, threadID int) (*api.Location, error) {
    return client.Thread(threadID)
}

// Set breakpoint
func setBreakpoint(client *rpc.Client, addr uint64) (*api.Breakpoint, error) {
    return client.CreateBreakpoint(&api.Breakpoint{
        Addr: addr,
        Tracepoint: true,  // No detiene, solo notifica
    })
}

// Evaluar expresión
func evaluate(client *rpc.Client, expr string) (*api.Variable, error) {
    return client.EvalVariable(-1, expr, api.LoadConfig{
        FollowPointers: true,
        MaxVariableRecurse: 1,
        MaxStringLen: 100,
    })
}
```

**Alternativa: runtime/trace para execution tracing**

Go tiene tracing built-in:

```go
import "runtime/trace"

// En el programa a trazar
func main() {
    // Crear archivo de trace
    f, _ := os.Create("trace.out")
    defer f.Close()
    
    // Iniciar tracing
    trace.Start(f)
    defer trace.Stop()
    
    // Código a trazar
    // ...
}
```

**Expresiones de evaluación en Go:**

```go
// Delve puede evaluar expresiones complejas de Go
// incluyendo slices, maps, channels, interfaces

result, err := client.EvalVariable(-1, "slice[i].field.nested", api.LoadConfig{
    FollowPointers: true,
    MaxVariableRecurse: 3,
})

// Tipos de Go soportados:
// - Scalars (int, float, string, bool)
// - Pointers
// - Arrays y Slices
// - Maps
// - Channels
// - Interfaces
// - Structs
// - Functions
```

**Limitaciones:**

- Delve añade overhead significativo cuando se stepping
- DWARF de Go tiene extensiones específicas
- Goroutine scheduling complica replay
- Las direcciones de memoria cambian con GC

**Herramientas existentes:** Delve, GDB (soporte limitado)

**Enfoque propuesto:** Integración con API de Delve para captura. Uso de runtime/trace para event recording ligero. Expression eval via API de Delve.

### 3.6 Tabla Comparativa de Lenguajes

| Lenguaje | Mecanismo de Capture | Overhead | Symbol Resolution | Expression Eval | Time-Travel | Thread Handling |
|----------|----------------------|----------|-------------------|-----------------|-------------|-----------------|
| **C/C++/Rust** | ptrace / eBPF | 2-10x | DWARF | DWARF loc + mem read | rr, UndoDB | Threads native |
| **Java/JVM** | JVMTI agent | 1.5-3x | JVMTI Class Metadata | JDI (in-VM) | Chronon | JVM threads/goroutines |
| **Python** | sys.settrace | 2-5x | frame objects | eval() in frame | reptile | GIL-limited |
| **JavaScript** | V8 CDP | 1.2-2x | V8 scope chain | CDP evaluateOnCallFrame | Replay.io | Event loop + workers |
| **Go** | Delve API / runtime/trace | 2-4x | DWARF + Delve types | Delve expression eval | Limited | Goroutines via scheduler |

---

## PARTE IV: ARQUITECTURA UNIFICADA — El Trace Adapter Model

### 4.1 El Patrón "Trace Adapter"

Para abstraer las diferencias entre lenguajes, cada lenguaje tiene un **Trace Adapter** que convierte eventos específicos del lenguaje en un formato unificado.

```
┌─────────────────────────────────────────────────────────────────┐
│                    MCP Server (Rust)                              │
│                                                                   │
│   ┌─────────────────────────────────────────────────────────┐    │
│   │              Unified Trace Format                         │    │
│   │           (FlatBuffers / Binary Format)                   │    │
│   │                                                           │    │
│   │   ┌─────────────────────────────────────────────────┐   │    │
│   │   │ EventType:                                       │   │    │
│   │   │   - Syscall                                      │   │    │
│   │   │   - FunctionEntry / FunctionExit                  │   │    │
│   │   │   - VariableWrite / VariableRead                  │   │    │
│   │   │   - Exception / Signal                           │   │    │
│   │   │   - ThreadCreate / ThreadExit                    │   │    │
│   │   │   - MemoryAlloc / MemoryFree                      │   │    │
│   │   │   - BreakpointHit / WatchTrigger                │   │    │
│   │   └─────────────────────────────────────────────────┘   │    │
│   └────────────────────┬──────────────────────────────────────┘    │
│                        │                                           │
│                        ▼                                           │
│   ┌───────────────────────────────────────────────────────────┐   │
│   │              Trace Adapter Interface (Rust trait)           │   │
│   └───┬─────────┬─────────┬─────────┬─────────┬─────┬────────┘   │
│       │         │         │         │         │     │              │
│       │         │         │         │         │     │              │
│   ┌───▼───┐ ┌──▼───┐ ┌──▼────┐ ┌──▼───┐ ┌──▼───┐ ┌──▼────┐    │
│   │Native  │ │  JVM  │ │Python │ │ V8/JS │ │  Go  │ │ .NET  │    │
│   │ptrace/ │ │ JVMTI │ │settrace│ │  CDP  │ │ Delve │ │ CoreClr│   │
│   │ eBPF   │ │       │ │       │ │       │ │ API   │ │        │    │
│   └────────┘ └───────┘ └────────┘ └───────┘ └───────┘ └────────┘    │
│                                                                   │
└───────────────────────────────────────────────────────────────────┘
```

### 4.2 Trace Adapter Interface (Rust trait)

```rust
use std::sync::Arc;
use tokio::sync::mpsc;

// Lenguajes soportados
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    C,
    Cpp,
    Rust,
    Java,
    Kotlin,
    Scala,
    Python,
    JavaScript,
    NodeJs,
    Go,
    CSharp,
    Unknown,
}

// Configuración de captura
#[derive(Debug, Clone)]
pub struct CaptureConfig {
    pub language: Language,
    pub capture_syscalls: bool,
    pub capture_memory: bool,
    pub capture_variables: bool,
    pub capture_stack: bool,
    pub max_memory_snapshots: usize,
    pub breakpoint_filter: Option<Vec<BreakpointFilter>>,
}

#[derive(Debug, Clone)]
pub struct BreakpointFilter {
    pub file_pattern: Option<String>,
    pub function_pattern: Option<String>,
    pub line: Option<u32>,
}

// Sesión de captura activa
pub struct CaptureSession {
    pub session_id: Uuid,
    pub pid: u32,
    pub language: Language,
    pub started_at: Instant,
    pub config: CaptureConfig,
    pub events_tx: mpsc::Sender<TraceEvent>,
}

// Evento de traza unificado
#[derive(Debug, Clone)]
pub struct TraceEvent {
    pub event_id: u64,
    pub timestamp_ns: u64,
    pub thread_id: u64,
    pub event_type: EventType,
    pub location: SourceLocation,
    pub data: EventData,
}

#[derive(Debug, Clone)]
pub enum EventType {
    // Nativos
    SyscallEnter,
    SyscallExit,
    Signal,
    // Funciones
    FunctionEntry,
    FunctionExit,
    // Variables
    VariableWrite,
    VariableRead,
    // Memoria
    MemoryAlloc,
    MemoryFree,
    MemoryWrite,
    MemoryRead,
    // Threads
    ThreadCreate,
    ThreadExit,
    ThreadSwitch,
    // Breakpoints
    BreakpointHit,
    WatchTrigger,
    // Excepciones
    ExceptionThrown,
    ExceptionCaught,
    // Especiales
    Custom(String),
}

#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub function: Option<String>,
    pub address: Option<u64>,
}

#[derive(Debug, Clone)]
pub enum EventData {
    Empty,
    Syscall {
        name: String,
        number: u64,
        args: Vec<u64>,
        return_value: i64,
    },
    Function {
        name: String,
        signature: Option<String>,
    },
    Variable {
        name: String,
        value: String,
        type_name: String,
        address: u64,
        scope: VariableScope,
    },
    Memory {
        address: u64,
        size: usize,
        data: Option<Vec<u8>>,
    },
    Thread {
        name: Option<String>,
        tid: u64,
    },
    Exception {
        message: String,
        type_name: String,
    },
    Registers {
        #[cfg(target_arch = "x86_64")]
        rax: u64, rbx: u64, rcx: u64, rdx: u64,
        rsi: u64, rdi: u64, rbp: u64, rsp: u64,
        r8: u64, r9: u64, r10: u64, r11: u64,
        r12: u64, r13: u64, r14: u64, r15: u64,
        rip: u64, rflags: u64,
        #[cfg(not(target_arch = "x86_64"))]
        #[allow(missing_docs)]
        regs: Vec<u64>,
    },
}

#[derive(Debug, Clone)]
pub enum VariableScope {
    Local,
    Global,
    Closure,
    Static,
    ThreadLocal,
}

// Trait principal del Trace Adapter
pub trait TraceAdapter: Send + Sync {
    /// Inicia la captura para un target específico
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError>;
    
    /// Detiene la captura y devuelve el archivo de traza
    fn stop_capture(&self, session: &CaptureSession) -> Result<TraceFile, TraceError>;
    
    /// Adjunta a un proceso en ejecución
    fn attach_to_process(&self, pid: u32, config: CaptureConfig) -> Result<CaptureSession, TraceError>;
    
    /// Devuelve el lenguaje soportado por este adapter
    fn get_language(&self) -> Language;
    
    /// Indica si soporta evaluación de expresiones
    fn supports_expression_eval(&self) -> bool;
    
    /// Evalúa una expresión en un contexto específico
    fn evaluate_expression(
        &self,
        session: &CaptureSession,
        expr: &str,
        frame_id: u64,
    ) -> Result<TypedValue, TraceError>;
    
    /// Obtiene información del runtime
    fn get_runtime_info(&self, session: &CaptureSession) -> Result<RuntimeInfo, TraceError>;
    
    /// Obtiene los threads activos
    fn get_threads(&self, session: &CaptureSession) -> Result<Vec<ThreadInfo>, TraceError>;
    
    /// Obtiene el stack trace en un punto específico
    fn get_stack_trace(
        &self,
        session: &CaptureSession,
        thread_id: u64,
        max_depth: usize,
    ) -> Result<Vec<StackFrame>, TraceError>;
    
    /// Obtiene las variables en un scope específico
    fn get_variables(
        &self,
        session: &CaptureSession,
        frame_id: u64,
        scope: VariableScope,
    ) -> Result<Vec<VariableInfo>, TraceError>;
}

#[derive(Debug, Clone)]
pub struct RuntimeInfo {
    pub language: Language,
    pub version: String,
    pub pid: u32,
    pub start_time: Instant,
    pub bitness: u8,
}

#[derive(Debug, Clone)]
pub struct ThreadInfo {
    pub id: u64,
    pub name: Option<String>,
    pub state: ThreadState,
    pub is_main: bool,
}

#[derive(Debug, Clone)]
pub enum ThreadState {
    Running,
    Sleeping,
    Waiting,
    Zombie,
    Stopped,
}

#[derive(Debug, Clone)]
pub struct StackFrame {
    pub id: u64,
    pub depth: u32,
    pub function: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub address: u64,
}

#[derive(Debug, Clone)]
pub struct VariableInfo {
    pub name: String,
    pub value: String,
    pub type_name: String,
    pub address: u64,
    pub scope: VariableScope,
}

#[derive(Debug, Clone)]
pub struct TypedValue {
    pub value: String,
    pub type_name: String,
    pub is_null: bool,
    pub members: Option<Vec<VariableInfo>>,
}

// Errores de trace
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("Proceso no encontrado: {0}")]
    ProcessNotFound(u32),
    
    #[error("Permiso denegado: {0}")]
    PermissionDenied(String),
    
    #[error("Lenguaje no soportado: {0}")]
    UnsupportedLanguage(String),
    
    #[error("Sesión no válida: {0}")]
    InvalidSession(String),
    
    #[error("Expresión inválida: {0}")]
    InvalidExpression(String),
    
    #[error("Error de comunicación: {0}")]
    CommunicationError(String),
    
    #[error("Error interno: {0}")]
    InternalError(String),
}

// Archivo de traza
pub struct TraceFile {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub event_count: u64,
    pub duration_ns: u64,
    pub language: Language,
}
```

### 4.3 El Formato Unificado de Traza (FlatBuffers)

El formato de traza usa FlatBuffers para zero-copy parsing:

```capnp
// chronos_trace.fbs - Schema para el formato de traza unificado

namespace Chronos.Trace;

enum EventType : byte {
    // Syscalls
    SyscallEnter,
    SyscallExit,
    
    // Funciones
    FunctionEntry,
    FunctionExit,
    
    // Variables
    VariableWrite,
    VariableRead,
    
    // Memoria
    MemoryAlloc,
    MemoryFree,
    MemoryWrite,
    MemoryRead,
    
    // Threads
    ThreadCreate,
    ThreadExit,
    ThreadSwitch,
    
    // Breakpoints
    BreakpointHit,
    WatchTrigger,
    
    // Excepciones
    ExceptionThrown,
    ExceptionCaught,
    
    // Especial
    Custom
}

enum VariableScope : byte {
    Local,
    Global,
    Closure,
    Static,
    ThreadLocal
}

// Location en código fuente
table SourceLocation {
    file: string (key);
    line: uint32;
    column: uint32 (key);
    function: string (key);
    address: uint64;
}

// Valor de variable
table Variable {
    name: string (key);
    value: string;
    type_name: string;
    address: uint64;
    scope: VariableScope;
    is_null: bool = false;
}

// Estado de registros
table RegisterState {
    // x86-64 registers (principal arquitectura)
    rax: uint64;
    rbx: uint64;
    rcx: uint64;
    rdx: uint64;
    rsi: uint64;
    rdi: uint64;
    rbp: uint64;
    rsp: uint64;
    r8: uint64;
    r9: uint64;
    r10: uint64;
    r11: uint64;
    r12: uint64;
    r13: uint64;
    r14: uint64;
    r15: uint64;
    rip: uint64;
    rflags: uint64;
}

// Información de syscall
table SyscallInfo {
    name: string;
    number: uint64;
    args: [uint64];
    return_value: int64;
}

// Información de función
table FunctionInfo {
    name: string;
    signature: string;
}

// Información de memoria
table MemoryInfo {
    address: uint64;
    size: uint64;
    data: [ubyte] (force_align: 16);
}

// Información de thread
table ThreadInfo {
    tid: uint64;
    name: string;
    is_main: bool = false;
}

// Información de excepción
table ExceptionInfo {
    type_name: string;
    message: string;
}

// Evento de traza individual
table TraceEvent {
    event_id: uint64;
    timestamp_ns: uint64;
    thread_id: uint64;
    event_type: EventType;
    
    // Location
    location: SourceLocation;
    
    // Datos específicos por tipo (solo uno debe estar presente)
    syscall: SyscallInfo (key);
    function: FunctionInfo (key);
    variable: Variable (key);
    registers: RegisterState (key);
    memory: MemoryInfo (key);
    thread: ThreadInfo (key);
    exception: ExceptionInfo (key);
}

// Metadata de la traza
table TraceMetadata {
    trace_id: string (key);
    session_id: string;
    language: string;
    version: string;
    start_time_ns: uint64;
    end_time_ns: uint64;
    event_count: uint64;
    pid: uint64;
}

// Traza completa
table ExecutionTrace {
    metadata: TraceMetadata;
    events: [TraceEvent];
}

root_type ExecutionTrace;
```

### 4.4 Implementación de Cada Trace Adapter

#### 4.4.1 NativeTraceAdapter (C, C++, Rust)

```rust
pub struct NativeTraceAdapter {
    symbol_resolver: Arc<dyn SymbolResolver>,
    memory_reader: Arc<dyn MemoryReader>,
}

impl NativeTraceAdapter {
    pub fn new() -> Self {
        Self {
            symbol_resolver: Arc::new(DwarfSymbolResolver::new()),
            memory_reader: Arc::new(PtraceMemoryReader::new()),
        }
    }
}

impl TraceAdapter for NativeTraceAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        // 1. Fork + exec el target
        // 2. En el hijo: ptrace(PTRACE_TRACEME)
        // 3. En el padre: esperar eventos y grabarlos
        
        let (events_tx, events_rx) = mpsc::channel(10000);
        
        // Spawn worker para capturar eventos
        let worker = NativeCaptureWorker {
            pid,
            events_tx,
            config: config.clone(),
            symbol_resolver: self.symbol_resolver.clone(),
        };
        
        tokio::spawn(worker.run());
        
        Ok(CaptureSession {
            session_id: Uuid::new_v4(),
            pid,
            language: Language::C,
            started_at: Instant::now(),
            config,
            events_tx,
        })
    }
    
    fn get_variables(
        &self,
        session: &CaptureSession,
        frame_id: u64,
        scope: VariableScope,
    ) -> Result<Vec<VariableInfo>, TraceError> {
        // 1. Obtener location DWARF para el frame
        // 2. Para cada variable en el frame:
        //    a. Obtener location expression
        //    b. Evaluar expresión (memoria/registro)
        //    c. Leer bytes
        //    d. Interpretar según tipo DWARF
        let variables = self.symbol_resolver
            .get_frame_variables(session.pid, frame_id, scope)?;
        
        Ok(variables)
    }
}
```

#### 4.4.2 JVMTI Trace Adapter (Java, Kotlin, Scala)

```rust
pub struct JvmTraceAdapter {
    jvm_path: PathBuf,
    agent_jar: PathBuf,
}

impl JvmTraceAdapter {
    pub fn new(jvm_path: PathBuf, agent_jar: PathBuf) -> Self {
        Self { jvm_path, agent_jar }
    }
}

impl TraceAdapter for JvmTraceAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        // Iniciar JVM con agente JVMTI
        let mut cmd = Command::new(&self.jvm_path);
        cmd.arg(format!("-agentpath:{}", self.agent_jar.display()))
           .arg("-jar")
           .arg(config.target.clone());
        
        let child = cmd.spawn()
            .map_err(|e| TraceError::InternalError(e.to_string()))?;
        
        // Conectar al agente via JDWP
        let session = self.connect_to_agent(child.id()).await?;
        
        Ok(CaptureSession {
            session_id: Uuid::new_v4(),
            pid: child.id(),
            language: Language::Java,
            started_at: Instant::now(),
            config,
            events_tx,
        })
    }
    
    fn supports_expression_eval(&self) -> bool {
        true  // JDI soporta evaluación
    }
    
    fn evaluate_expression(
        &self,
        session: &CaptureSession,
        expr: &str,
        frame_id: u64,
    ) -> Result<TypedValue, TraceError> {
        // Usar JDI para evaluar expresión en el contexto del frame
        self.jdi_client
            .evaluate(expr, frame_id)
            .map_err(|e| TraceError::InvalidExpression(e.to_string()))
    }
}
```

#### 4.4.3 PythonTraceAdapter

```rust
pub struct PythonTraceAdapter {
    python_path: PathBuf,
}

impl PythonTraceAdapter {
    pub fn new() -> Self {
        Self {
            python_path: PathBuf::from("python"),
        }
    }
}

impl TraceAdapter for PythonTraceAdapter {
    fn start_capture(&self, config: CaptureConfig) -> Result<CaptureSession, TraceError> {
        // Crear wrapper que habilita sys.settrace
        let wrapper_code = format!(r#"
import sys
import json

# Trace function que emite eventos
def chronos_trace(frame, event, arg):
    event_data = {{
        'event': event,
        'filename': frame.f_code.co_filename,
        'lineno': frame.f_lineno,
        'function': frame.f_code.co_name,
        'locals': {{k: repr(v) for k, v in frame.f_locals.items()}},
    }}
    print(json.dumps(event_data), flush=True)
    return chronos_trace

# Habilitar tracing
sys.settrace(chronos_trace)

# Ejecutar el script target
exec(open('{}').read())
"#, config.target.display());
        
        // Spawn python con el wrapper
        let mut cmd = Command::new(&self.python_path);
        cmd.arg("-c").arg(wrapper_code);
        
        let child = cmd.spawn()
            .map_err(|e| TraceError::InternalError(e.to_string()))?;
        
        Ok(CaptureSession {
            session_id: Uuid::new_v4(),
            pid: child.id(),
            language: Language::Python,
            started_at: Instant::now(),
            config,
            events_tx,
        })
    }
    
    fn evaluate_expression(
        &self,
        session: &CaptureSession,
        expr: &str,
        frame_id: u64,
    ) -> Result<TypedValue, TraceError> {
        // Enviar expresión al proceso para evaluación
        // El trace wrapper evalua en contexto de frame
        self.send_eval_request(session, frame_id, expr)
    }
}
```

---

## PARTE V: MOTOR DE ÍNDICES Y CONSULTAS

### 5.1 Shadow Index (ART + Sharding)

El Shadow Index es el índice principal para buscar eventos por dirección de memoria.

```rust
use art::AdaptiveRadixTree;
use std::sync::Arc;

// Dirección de memoria → Lista de eventos
pub struct ShadowIndex {
    tree: Arc<AdaptiveRadixTree<u64, Vec<u64>>>,  // addr → event_ids
    lock_free_shards: Vec<Arc<LockFreeVector<(u64, u64)>>>,  // Shards para escritura concurrent
}

impl ShadowIndex {
    pub fn new(num_shards: usize) -> Self {
        let shards = (0..num_shards)
            .map(|_| Arc::new(LockFreeVector::new()))
            .collect();
        
        Self {
            tree: Arc::new(AdaptiveRadixTree::new()),
            lock_free_shards: shards,
        }
    }
    
    // Bit-packing: EventID + DeltaTS en un solo u64
    // Formato: [event_id: 32 bits][delta_ts: 32 bits]
    fn pack(event_id: u64, delta_ts: u64) -> u64 {
        (event_id << 32) | (delta_ts & 0xFFFFFFFF)
    }
    
    pub fn insert(&self, address: u64, event_id: u64, timestamp: u64) {
        // Calcular shard basado en address
        let shard_idx = (address >> 16) as usize % self.lock_free_shards.len();
        let shard = &self.lock_free_shards[shard_idx];
        
        // Obtener timestamp base del árbol para calcular delta
        let base_ts = self.tree.get(&address)
            .map(|entries| entries.last().map(|e| e >> 32).unwrap_or(0))
            .unwrap_or(0);
        
        let delta_ts = timestamp - base_ts;
        let packed = Self::pack(event_id, delta_ts);
        
        shard.push((address, packed));
    }
    
    // Busca todos los eventos para una dirección
    pub fn get_events_for_address(&self, address: u64) -> Vec<(u64, u64)> {
        // Buscar en shards
        let mut results = Vec::new();
        for shard in &self.lock_free_shards {
            for (addr, packed) in shard.iter() {
                if *addr == address {
                    results.push(*packed);
                }
            }
        }
        results
    }
}

// Target: 5-10M eventos/segundo por core
pub struct IndexConfig {
    pub num_shards: usize,
    pub chunk_size: usize,
    pub flush_interval_ms: u64,
}
```

### 5.2 Temporal Index

Índice para consultas por rango de tiempo.

```rust
use std::collections::BTreeMap;

pub struct TemporalIndex {
    // BTreeMap<timestamp, EventID>
    // Chunked para fast seeking
    tree: BTreeMap<u64, u64>,
    chunks: Vec<TemporalChunk>,
}

pub struct TemporalChunk {
    pub start_ts: u64,
    pub end_ts: u64,
    pub first_event_id: u64,
    pub last_event_id: u64,
    pub event_count: u64,
}

impl TemporalIndex {
    pub fn insert(&mut self, timestamp: u64, event_id: u64) {
        self.tree.insert(timestamp, event_id);
    }
    
    // Busca el event_id más cercano a un timestamp dado
    pub fn seek_nearest(&self, target_ts: u64) -> Option<(u64, u64)> {
        // Upper bound del timestamp
        let ts = self.tree.upper_bound(&target_ts);
        // Lower bound del timestamp
        let ts_lower = self.tree.lower_bound(&target_ts);
        
        // Devolver el más cercano
        match (ts, ts_lower) {
            (Some((ts1, e1)), Some((ts2, e2))) => {
                if (target_ts - ts2) < (ts1 - target_ts) {
                    Some((ts2, e2))
                } else {
                    Some((ts1, e1))
                }
            }
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        }
    }
    
    // Busca todos los eventos en un rango de tiempo
    pub fn range(&self, start: u64, end: u64) -> Vec<(u64, u64)> {
        self.tree.range(start..end)
            .map(|(&ts, &eid)| (ts, eid))
            .collect()
    }
    
    // Chunking: divide en ventanas de 10ms para búsqueda rápida
    pub fn chunk(&mut self, window_ms: u64) {
        let window_ns = window_ms * 1_000_000;
        let mut current_chunk_start = 0;
        let mut current_chunk_end = window_ns;
        
        for (&ts, &eid) in &self.tree {
            if ts > current_chunk_end {
                // Guardar chunk actual
                self.chunks.push(TemporalChunk {
                    start_ts: current_chunk_start,
                    end_ts: current_chunk_end,
                    first_event_id: eid,
                    last_event_id: eid,
                    event_count: (current_chunk_end - current_chunk_start) / window_ns,
                });
                
                current_chunk_start = current_chunk_end;
                current_chunk_end += window_ns;
            }
        }
    }
}
```

### 5.3 Causality Index

Índice para tracking de mutaciones de variables.

```rust
// Maps: variable_address → lista de eventos de escritura
pub struct CausalityIndex {
    // Para cada dirección, guarda los eventos que la escribieron
    write_events: HashMap<u64, Vec<CausalityEntry>>,
    // Índice invertido: variable_name → addresses
    name_to_addr: HashMap<String, Vec<u64>>,
}

#[derive(Debug, Clone)]
pub struct CausalityEntry {
    pub event_id: u64,
    pub timestamp: u64,
    pub thread_id: u64,
    pub value_before: Option<String>,
    pub value_after: String,
    pub instruction: String,
    pub function: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

impl CausalityIndex {
    pub fn record_write(
        &mut self,
        address: u64,
        var_name: Option<&str>,
        event_id: u64,
        timestamp: u64,
        thread_id: u64,
        value_before: Option<String>,
        value_after: String,
        instruction: String,
        function: String,
    ) {
        let entry = CausalityEntry {
            event_id,
            timestamp,
            thread_id,
            value_before,
            value_after,
            instruction,
            function,
            file: None,
            line: None,
        };
        
        self.write_events
            .entry(address)
            .or_default()
            .push(entry);
        
        // Actualizar índice de nombres si está disponible
        if let Some(name) = var_name {
            self.name_to_addr
                .entry(name.to_string())
                .or_default()
                .push(address);
        }
    }
    
    // Encuentra la última mutación a una dirección antes de un timestamp
    pub fn find_last_mutation(
        &self,
        address: u64,
        before_ts: u64,
    ) -> Option<&CausalityEntry> {
        self.write_events.get(&address)
            .and_then(|entries| {
                entries.iter()
                    .filter(|e| e.timestamp < before_ts)
                    .max_by_key(|e| e.timestamp)
            })
    }
    
    // Trace la historia completa de una variable
    pub fn trace_lineage(
        &self,
        name: &str,
    ) -> Vec<(u64, CausalityEntry)> {
        let mut result = Vec::new();
        
        if let Some(addresses) = self.name_to_addr.get(name) {
            for &addr in addresses {
                if let Some(entries) = self.write_events.get(&addr) {
                    for entry in entries {
                        result.push((addr, entry.clone()));
                    }
                }
            }
        }
        
        // Ordenar por timestamp
        result.sort_by_key(|(_, e)| e.timestamp);
        result
    }
}
```

### 5.4 Performance Index

Índice para hardware counters y detección de regresiones.

```rust
use perf_event::{Perf, PerfEventId};

// Integración con perf_event_open
pub struct PerformanceIndex {
    cycles_counter: Option<Perf>,
    instructions_counter: Option<Perf>,
    cache_misses_counter: Option<Perf>,
    function_counts: HashMap<u64, u64>,  // addr → count
}

impl PerformanceIndex {
    pub fn new() -> Self {
        let cycles = Perf::builder()
            .kind(perf_event_open::perf_type::Hardware::CPU_CYCLES)
            .build()
            .ok();
            
        let instructions = Perf::builder()
            .kind(perf_event_open::perf_type::Hardware::INSTRUCTIONS)
            .build()
            .ok();
            
        let cache_misses = Perf::builder()
            .kind(perf_event_open::perf_type::Hardware::CACHE_MISSES)
            .build()
            .ok();
        
        Self {
            cycles_counter: cycles,
            instructions_counter: instructions,
            cache_misses_counter: cache_misses,
            function_counts: HashMap::new(),
        }
    }
    
    // Lee counters para un evento específico
    pub fn read_counters(&self) -> Option<PerfCounters> {
        Some(PerfCounters {
            cycles: self.cycles_counter.as_ref()?.read(),
            instructions: self.instructions_counter.as_ref()?.read(),
            cache_misses: self.cache_misses_counter.as_ref()?.read(),
        })
    }
    
    // Incrementa contador para una función
    pub fn record_function_call(&mut self, func_addr: u64) {
        *self.function_counts.entry(func_addr).or_insert(0) += 1;
    }
}

#[derive(Debug, Clone)]
pub struct PerfCounters {
    pub cycles: u64,
    pub instructions: u64,
    pub cache_misses: u64,
}
```

---

## PARTE VI: HERRAMIENTAS MCP COMPLETAS

### 6.1 Herramientas de Control

#### debug_run

```json
{
  "name": "debug_run",
  "description": "Ejecuta el programa target bajo captura de traza, grabando toda la ejecución para análisis posterior",
  "inputSchema": {
    "type": "object",
    "properties": {
      "target": {
        "type": "string",
        "description": "Ruta al ejecutable o script"
      },
      "args": {
        "type": "array",
        "items": {"type": "string"},
        "description": "Argumentos de línea de comando"
      },
      "env": {
        "type": "object",
        "description": "Variables de entorno (hereda si no se especifican)"
      },
      "cwd": {
        "type": "string",
        "description": "Directorio de trabajo"
      },
      "language": {
        "type": "string",
        "enum": ["c", "cpp", "rust", "java", "python", "javascript", "go", "csharp"],
        "description": "Lenguaje del target (auto-detecta si no se especifica)"
      },
      "capture_config": {
        "type": "object",
        "properties": {
          "capture_syscalls": {"type": "boolean", "default": true},
          "capture_memory": {"type": "boolean", "default": false},
          "capture_variables": {"type": "boolean", "default": true},
          "capture_stack": {"type": "boolean", "default": true},
          "breakpoints": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "type": {"enum": ["function", "file", "address"]},
                "target": {"type": "string"},
                "condition": {"type": "string"}
              }
            }
          }
        }
      },
      "max_duration_ms": {
        "type": "integer",
        "description": "Timeout máximo de ejecución"
      }
    },
    "required": ["target"]
  }
}
```

**Respuesta:**

```json
{
  "session_id": "trace-abc123",
  "pid": 12345,
  "status": "running",
  "trace_file": "/tmp/chronos/trace-abc123.bin",
  "started_at": "2024-01-15T10:30:00Z",
  "language": "rust",
  "capture_config": {
    "capture_syscalls": true,
    "capture_variables": true
  }
}
```

#### debug_attach

```json
{
  "name": "debug_attach",
  "description": "Adjunta a un proceso en ejecución para captura de traza",
  "inputSchema": {
    "type": "object",
    "properties": {
      "pid": {"type": "integer", "description": "Process ID"},
      "language": {"type": "string"},
      "capture_config": {"$ref": "#/definitions/capture_config"}
    },
    "required": ["pid"]
  }
}
```

#### debug_stop

```json
{
  "name": "debug_stop",
  "description": "Detiene la captura de traza y cierra la sesión",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"}
    },
    "required": ["session_id"]
  }
}
```

### 6.2 Herramientas de Query

#### debug_query_trace

```json
{
  "name": "debug_query_trace",
  "description": "Consulta la traza grabada por tipo de evento, rango de tiempo, o patrón",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "filter": {
        "type": "object",
        "properties": {
          "event_type": {
            "type": "array",
            "items": {
              "enum": [
                "syscall_enter", "syscall_exit", "function_entry", "function_exit",
                "variable_write", "variable_read", "memory_alloc", "memory_free",
                "thread_create", "thread_exit", "exception_thrown", "breakpoint_hit"
              ]
            }
          },
          "thread_id": {"type": "integer"},
          "timestamp_start": {"type": "integer"},
          "timestamp_end": {"type": "integer"},
          "address_range": {
            "type": "object",
            "properties": {
              "start": {"type": "integer"},
              "end": {"type": "integer"}
            }
          },
          "function_pattern": {"type": "string"},
          "file_pattern": {"type": "string"}
        }
      },
      "limit": {"type": "integer", "default": 100},
      "offset": {"type": "integer", "default": 0},
      "sort_by": {"enum": ["timestamp", "event_id"], "default": "timestamp"}
    },
    "required": ["session_id"]
  }
}
```

**Respuesta:**

```json
{
  "session_id": "trace-abc123",
  "total_matching": 1543,
  "events": [
    {
      "event_id": 5001,
      "timestamp_ns": 4512303456,
      "thread_id": 12345,
      "event_type": "function_exit",
      "location": {
        "function": "process_data",
        "file": "main.rs",
        "line": 42
      },
      "data": {
        "return_value": "Ok(42)"
      }
    }
  ],
  "next_offset": 100
}
```

#### debug_get_memory

```json
{
  "name": "debug_get_memory",
  "description": "Lee el contenido de memoria en una dirección y timestamp específicos",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "address": {"type": "integer"},
      "size": {"type": "integer"},
      "timestamp": {"type": "integer"},
      "nearest_frame": {"type": "boolean", "default": true}
    },
    "required": ["session_id", "address", "size", "timestamp"]
  }
}
```

#### debug_get_registers

```json
{
  "name": "debug_get_registers",
  "description": "Obtiene el estado de todos los registros de CPU en un timestamp específico (solo lenguajes nativos)",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "timestamp": {"type": "integer"},
      "frame_id": {"type": "integer"},
      "thread_id": {"type": "integer"}
    },
    "required": ["session_id"]
  }
}
```

#### debug_get_stack

```json
{
  "name": "debug_get_stack",
  "description": "Reconstruye el call stack desde la traza en un timestamp específico",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "timestamp": {"type": "integer"},
      "depth": {"type": "integer", "default": 32},
      "thread_id": {"type": "integer"}
    },
    "required": ["session_id", "timestamp"]
  }
}
```

#### debug_get_variables

```json
{
  "name": "debug_get_variables",
  "description": "Obtiene las variables visibles en un scope específico",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "timestamp": {"type": "integer"},
      "frame_id": {"type": "integer"},
      "scope": {"enum": ["local", "global", "closure", "all"], "default": "all"},
      "thread_id": {"type": "integer"}
    },
    "required": ["session_id"]
  }
}
```

#### debug_diff

```json
{
  "name": "debug_diff",
  "description": "Compara el estado del proceso entre dos timestamps",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "timestamp_a": {"type": "integer"},
      "timestamp_b": {"type": "integer"},
      "diff_type": {"enum": ["registers", "memory", "variables", "all"], "default": "all"},
      "memory_regions": {"type": "array", "items": {"type": "integer"}}
    },
    "required": ["session_id", "timestamp_a", "timestamp_b"]
  }
}
```

### 6.3 Herramientas de Análisis

#### debug_call_graph

```json
{
  "name": "debug_call_graph",
  "description": "Construye un grafo de llamadas a funciones desde la traza",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "filter": {
        "type": "object",
        "properties": {
          "module": {"type": "string"},
          "function_prefix": {"type": "string"},
          "min_calls": {"type": "integer"}
        }
      },
      "max_depth": {"type": "integer", "default": 10}
    },
    "required": ["session_id"]
  }
}
```

#### debug_find_variable_origin

```json
{
  "name": "debug_find_variable_origin",
  "description": "Encuentra todas las escrituras a una dirección de memoria/variable",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "variable_name": {"type": "string"},
      "variable_address": {"type": "integer"},
      "time_range": {
        "type": "object",
        "properties": {
          "start": {"type": "integer"},
          "end": {"type": "integer"}
        }
      },
      "include_reads": {"type": "boolean", "default": true}
    },
    "required": ["session_id"]
  }
}
```

#### debug_analyze_memory

```json
{
  "name": "debug_analyze_memory",
  "description": "Analiza una región de memoria, mostrando allocations y frees",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "address": {"type": "integer"},
      "size": {"type": "integer"},
      "time_range": {
        "type": "object",
        "properties": {
          "start": {"type": "integer"},
          "end": {"type": "integer"}
        }
      }
    },
    "required": ["session_id"]
  }
}
```

#### debug_detect_races

```json
{
  "name": "debug_detect_races",
  "description": "Detecta race conditions en la traza",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "time_range": {
        "type": "object",
        "properties": {
          "start": {"type": "integer"},
          "end": {"type": "integer"}
        }
      }
    },
    "required": ["session_id"]
  }
}
```

#### debug_find_crash

```json
{
  "name": "debug_find_crash",
  "description": "Encuentra el punto exacto del crash y su causa",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"}
    },
    "required": ["session_id"]
  }
}
```

### 6.4 Herramientas Avanzadas

#### inspect_causality

```json
{
  "name": "inspect_causality",
  "description": "Determina qué escribió un valor específico en una dirección de memoria",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "symbol": {"type": "string", "description": "Nombre de variable o dirección"},
      "timestamp": {"type": "integer"},
      "value": {"type": "string", "description": "Valor específico a buscar (opcional)"}
    },
    "required": ["session_id", "symbol"]
  }
}
```

#### forensic_memory_audit

```json
{
  "name": "forensic_memory_audit",
  "description": "Audita si una dirección de memoria fue escrita de forma legítima",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "address": {"type": "integer"},
      "time_range": {
        "type": "object",
        "properties": {
          "start": {"type": "integer"},
          "end": {"type": "integer"}
        }
      }
    },
    "required": ["session_id", "address"]
  }
}
```

#### performance_regression_audit

```json
{
  "name": "performance_regression_audit",
  "description": "Detecta anomalías de rendimiento en la traza",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "baseline_session_id": {"type": "string"},
      "threshold_percent": {"type": "number", "default": 10.0}
    },
    "required": ["session_id"]
  }
}
```

#### compare_historical_executions

```json
{
  "name": "compare_historical_executions",
  "description": "Compara dos trazas de ejecuciones diferentes",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id_a": {"type": "string"},
      "session_id_b": {"type": "string"},
      "compare_type": {"enum": ["call_graph", "memory", "syscalls", "all"], "default": "all"}
    },
    "required": ["session_id_a", "session_id_b"]
  }
}
```

#### evaluate_expression

```json
{
  "name": "evaluate_expression",
  "description": "Evalúa una expresión en el contexto de un frame específico",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "expression": {"type": "string"},
      "frame_id": {"type": "integer"},
      "timestamp": {"type": "integer"},
      "thread_id": {"type": "integer"}
    },
    "required": ["session_id", "expression"]
  }
}
```

#### subscribe_to_symbol

```json
{
  "name": "subscribe_to_symbol",
  "description": "Crea un watch en vivo para un símbolo (requiere eBPF o modo debug activo)",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "symbol": {"type": "string"},
      "watch_type": {"enum": ["read", "write", "both"], "default": "both"},
      "callback_url": {"type": "string", "description": "Webhook o endpoint para notificaciones"}
    },
    "required": ["session_id", "symbol"]
  }
}
```

### 6.5 Herramientas de Abstracción (Semantic Compression)

#### debug_execution_summary

```json
{
  "name": "debug_execution_summary",
  "description": "Resumen ejecutivo de la ejecución (nivel 0)",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"}
    },
    "required": ["session_id"]
  }
}
```

**Respuesta:**

```json
{
  "session_id": "trace-abc123",
  "summary": {
    "duration_ns": 5000000000,
    "total_events": 125000,
    "total_syscalls": 3500,
    "functions_called": 142,
    "threads_created": 4,
    "memory_allocated_bytes": 52428800,
    "hotspots": [
      {"function": "process_data", "call_count": 5000},
      {"function": "validate_input", "call_count": 3500}
    ],
    "potential_issues": [
      {"type": "memory_leak", "confidence": 0.8},
      {"type": "null_dereference", "confidence": 0.6}
    ]
  }
}
```

#### debug_expand_hotspot

```json
{
  "name": "debug_expand_hotspot",
  "description": "Expande un hotspot para ver detalle (nivel 1)",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "hotspot_id": {"type": "string"}
    },
    "required": ["session_id", "hotspot_id"]
  }
}
```

#### debug_get_saliency_scores

```json
{
  "name": "debug_get_saliency_scores",
  "description": "Lista de eventos/anomalías rankeados por importancia",
  "inputSchema": {
    "type": "object",
    "properties": {
      "session_id": {"type": "string"},
      "limit": {"type": "integer", "default": 20}
    },
    "required": ["session_id"]
  }
}
```

---

## PARTE VII: RESOLUCIÓN DE EXPRESIONES MULTI-LENGUAJE

### 7.1 El Problema

Cada lenguaje tiene diferentes:
- Sistemas de tipos
- Semánticas de evaluación
- Scoping rules
- Manejo de memoria

La misma expresión `x + y * 2` tiene significados completamente diferentes en cada lenguaje.

### 7.2 Estrategia por Lenguaje

| Lenguaje | Estrategia | Notas |
|----------|------------|-------|
| **C/C++/Rust** | DWARF location expressions → memory read → type cast | Requiere parsing de tipos DWARF |
| **Java/JVM** | JDI evaluation in target VM | Expression eval corre en el VM destino |
| **Python** | eval() en contexto de frame | frame.f_globals + frame.f_locals |
| **JavaScript** | CDP Debugger.evaluateOnCallFrame | Sandbox de V8 |
| **Go** | Delve expression evaluator | Maneja slices, maps, channels |

### 7.3 Expression Adapter Interface

```rust
pub trait ExpressionEvaluator: Send + Sync {
    /// Evalúa una expresión en un contexto específico
    fn evaluate(&self, expr: &str, context: &FrameContext) -> Result<TypedValue, EvalError>;
    
    /// Obtiene las variables disponibles en un contexto
    fn get_available_variables(&self, context: &FrameContext) -> Vec<VariableInfo>;
    
    /// Devuelve el lenguaje soportado
    fn get_language(&self) -> Language;
    
    /// Verifica si puede evaluar una expresión específica
    fn can_evaluate(&self, expr: &str) -> bool;
}

#[derive(Debug, Clone)]
pub struct FrameContext {
    pub session_id: Uuid,
    pub frame_id: u64,
    pub thread_id: u64,
    pub timestamp: u64,
    pub language: Language,
    // Datos específicos del lenguaje
    pub lang_specific: LangSpecificContext,
}

#[derive(Debug, Clone)]
pub enum LangSpecificContext {
    Native {
        registers: RegisterState,
        stack_addr: u64,
        cfa: u64,
    },
   Jvm {
        classloader: String,
        method_signature: String,
    },
    Python {
        frame_object_id: u64,
        locals: HashMap<String, PyValue>,
        globals: HashMap<String, PyValue>,
    },
    V8 {
        scope_chain: Vec<V8Scope>,
        call_frame_id: String,
    },
    Go {
        goroutine_id: u64,
        delimiters: Vec<String>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("Variable no encontrada: {0}")]
    VariableNotFound(String),
    
    #[error("Tipo incompatible: {0}")]
    TypeMismatch(String),
    
    #[error("Dirección de memoria inválida: {0}")]
    InvalidAddress(u64),
    
    #[error("Expresión demasiado compleja: {0}")]
    ExpressionTooComplex(String),
    
    #[error("Lenguaje no soportado para evaluación: {0}")]
    UnsupportedLanguage(String),
    
    #[error("Error interno: {0}")]
    InternalError(String),
}
```

### 7.4 Implementaciones Específicas

#### NativeExpressionEvaluator (C, C++, Rust)

```rust
impl ExpressionEvaluator for NativeExpressionEvaluator {
    fn evaluate(&self, expr: &str, context: &FrameContext) -> Result<TypedValue, EvalError> {
        let ctx = match &context.lang_specific {
            LangSpecificContext::Native { registers, stack_addr, cfa } => {
                (registers, *stack_addr, *cfa)
            }
            _ => return Err(EvalError::UnsupportedLanguage("native".to_string())),
        };
        
        // 1. Parsear expresión con un parser simple
        let ast = self.parser.parse(expr)?;
        
        // 2. Para cada identificador, resolver su dirección via DWARF
        let resolved = self.resolve_identifiers(&ast, context)?;
        
        // 3. Evaluar en el contexto de registros/memoria
        self.eval_ast(&resolved, ctx)
    }
    
    fn resolve_identifiers(&self, ast: &Expr, context: &FrameContext) -> Result<Expr, EvalError> {
        match ast {
            Expr::Ident(name) => {
                // Buscar en variables del frame
                let var_info = self.get_variable_info(context, name)?;
                Ok(Expr::Deref(var_info.address, var_info.type_info))
            }
            Expr::BinaryOp(op, left, right) => {
                let resolved_left = self.resolve_identifiers(left, context)?;
                let resolved_right = self.resolve_identifiers(right, context)?;
                Ok(Expr::BinaryOp(*op, Box::new(resolved_left), Box::new(resolved_right)))
            }
            // ... otros casos
        }
    }
}
```

#### PythonExpressionEvaluator

```rust
impl ExpressionEvaluator for PythonExpressionEvaluator {
    fn evaluate(&self, expr: &str, context: &FrameContext) -> Result<TypedValue, EvalError> {
        let ctx = match &context.lang_specific {
            LangSpecificContext::Python { locals, globals, .. } => {
                (locals, globals)
            }
            _ => return Err(EvalError::UnsupportedLanguage("python".to_string())),
        };
        
        // Crear namespace combinado
        let mut namespace = globals.clone();
        namespace.extend(locals.clone());
        
        // Evaluar expresión Python
        let result = eval(expr, globals!(), locals.clone())
            .map_err(|e| EvalError::InternalError(e.to_string()))?;
        
        // Convertir resultado a TypedValue
        Ok(TypedValue {
            value: repr(result),
            type_name: type(result).__name__,
            is_null: result.is_none(),
            members: None,
        })
    }
}
```

### 7.5 Resolución Post-Mortem

Para queries post-mortem, la traza debe contener suficiente información:

**Native (C/C++/Rust):**
- Snapshots de memoria en cada evento
- DWARF info del binario
- Valores de registros en cada frame

**Java:**
- Valores de variables capturados en cada MethodEntry/Exit
- Class metadata via JVMTI
- Heap snapshots si es necesario

**Python:**
- frame.f_locals y frame.f_globals en cada evento
- Bytecode para resolver closures

**JavaScript:**
- Scope chain snapshots en cada frame
- Objetos serializados

**Go:**
- Variables locales capturadas via Delve
- Goroutine state

---

## PARTE VIII: PERSISTENCIA Y COMPARACIÓN HISTÓRICA

### 8.1 Almacenamiento (RocksDB + CAS)

```
┌─────────────────────────────────────────────────────────────────┐
│              ARQUITECTURA DE ALMACENAMIENTO                      │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │                    CAS (Content Addressable Store)       │   │
│   │                                                          │   │
│   │   Key: hash(trace_data)                                  │   │
│   │   Value: compressed_trace_data                           │   │
│   │                                                          │   │
│   │   Beneficios:                                            │   │
│   │   - Deduplicación automática                             │   │
│   │   - Shared library traces solo se almacenan una vez      │   │
│   │   - Integridad verificada por hash                       │   │
│   │                                                          │   │
│   └─────────────────────────────────────────────────────────┘   │
│                              │                                   │
│                              ▼                                   │
│   ┌─────────────────────────────────────────────────────────┐   │
│   │                    RocksDB Index                          │   │
│   │                                                          │   │
│   │   trace_id:session_id:timestamp:address → CAS_key        │   │
│   │                                                          │   │
│   │   Meta:                                                  │   │
│   │   - trace:{trace_id} → metadata                          │   │
│   │   - session:{session_id} → trace_id                      │   │
│   │   - symbol:{name} → [addresses]                          │   │
│   │                                                          │   │
│   └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

```rust
use rocksdb::{DB, Options, ColumnFamilyDescriptor};

pub struct TraceStore {
    db: DB,
}

impl TraceStore {
    pub fn new(path: &Path) -> Result<Self, TraceError> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(rocksdb::CompressionType::Lz4);
        
        // Column families para diferentes tipos de datos
        let cfs = vec![
            ColumnFamilyDescriptor::new("traces", Options::default()),
            ColumnFamilyDescriptor::new("sessions", Options::default()),
            ColumnFamilyDescriptor::new("symbols", Options::default()),
            ColumnFamilyDescriptor::new("causality", Options::default()),
        ];
        
        let db = DB::open_cfs_descriptors(&opts, path, cfs)
            .map_err(|e| TraceError::InternalError(e.to_string()))?;
        
        Ok(Self { db })
    }
    
    // Guardar traza con CAS
    pub fn store_trace(&self, trace: &[u8]) -> Result<String, TraceError> {
        // Calcular hash para CAS
        let hash = sha256(trace);
        let key = format!("trace:{}", hex(hash));
        
        // Comprimir y guardar
        let compressed = lz4_compress(trace);
        self.db.put(&key, &compressed)
            .map_err(|e| TraceError::InternalError(e.to_string()))?;
        
        Ok(key)
    }
    
    // Guardar metadata de sesión
    pub fn store_session(&self, session: &SessionMetadata) -> Result<(), TraceError> {
        let key = format!("session:{}", session.id);
        let value = serde_json::to_vec(session)
            .map_err(|e| TraceError::InternalError(e.to_string()))?;
        
        self.db.put_cf("sessions", &key, &value)
            .map_err(|e| TraceError::InternalError(e.to_string()))?;
        
        Ok(())
    }
}
```

### 8.2 Golden Trace Comparison

```rust
pub struct GoldenComparator {
    baseline_trace: Arc<TraceFile>,
    current_trace: Arc<TraceFile>,
    normalizer: AddressNormalizer,
}

pub struct DiffResult {
    pub call_graph_diff: Vec<CallGraphDiff>,
    pub memory_diff: Vec<MemoryRegionDiff>,
    pub syscall_diff: Vec<SyscallDiff>,
    pub divergence_point: Option<EventLocation>,
}

impl GoldenComparator {
    /// Compara traza actual contra baseline
    pub fn compare(&self, current: &TraceFile) -> DiffResult {
        // 1. Normalizar addresses (ASLR-aware)
        let normalized_current = self.normalizer.normalize(current);
        let normalized_baseline = self.normalizer.normalize(&self.baseline_trace);
        
        // 2. Encontrar punto de divergencia
        let divergence = self.find_divergence(
            &normalized_baseline,
            &normalized_current,
        );
        
        // 3. Generar diff completo
        DiffResult {
            call_graph_diff: self.diff_call_graphs(),
            memory_diff: self.diff_memory(),
            syscall_diff: self.diff_syscalls(),
            divergence_point: divergence,
        }
    }
    
    /// Normaliza direcciones para comparación (ignora ASLR)
    fn normalize_addresses(&self, trace: &TraceFile) -> Vec<NormalizedEvent> {
        // Para cada dirección, mapear a offset dentro del módulo
        // Esto permite comparar ejecuciones con ASLR activo
    }
}
```

### 8.3 Regression Detection

```rust
pub struct RegressionDetector {
    baseline_metrics: BaselineMetrics,
    thresholds: RegressionThresholds,
}

#[derive(Debug)]
pub struct BaselineMetrics {
    pub avg_cycles_per_function: HashMap<String, f64>,
    pub avg_duration_ms: f64,
    pub memory_allocation_rate: f64,
    pub syscall_frequency: HashMap<String, f64>,
}

#[derive(Debug)]
pub struct RegressionThresholds {
    pub cycle_threshold_percent: f64,  // e.g., 10.0 = 10% más lento es regressión
    pub memory_threshold_percent: f64,
    pub crash_tolerance: u32,  // número de crashes aceptables
}

impl RegressionDetector {
    /// Detecta regressiones vs baseline
    pub fn detect(&self, current: &TraceFile) -> Vec<Regression> {
        let current_metrics = self.compute_metrics(current);
        let mut regressions = Vec::new();
        
        // Performance regression
        for (func, &baseline_cycles) in &self.baseline_metrics.avg_cycles_per_function {
            if let Some(&current_cycles) = current_metrics.avg_cycles_per_function.get(func) {
                let change_percent = ((current_cycles - baseline_cycles) / baseline_cycles) * 100.0;
                if change_percent > self.thresholds.cycle_threshold_percent {
                    regressions.push(Regression {
                        kind: RegressionKind::Performance,
                        function: func.clone(),
                        baseline_value: baseline_cycles,
                        current_value: current_cycles,
                        change_percent,
                    });
                }
            }
        }
        
        // Memory leak detection
        if current_metrics.memory_allocation_rate 
            > self.baseline_metrics.memory_allocation_rate * 1.5 {
            regressions.push(Regression {
                kind: RegressionKind::MemoryLeak,
                // ...
            });
        }
        
        regressions
    }
}
```

---

## PARTE IX: SEGURIDAD Y SANDBOXING

### 9.1 Amenazas

| Amenaza | Descripción | Impacto |
|---------|-------------|---------|
| **Malicious trace data** | Datos de traza manipulados para exploit | Execution hijacking |
| **Expression eval injection** | Inyección de código via expresiones | Arbitrary code execution |
| **eBPF program safety** | Programas eBPF maliciosos | Kernel panic, privilege escalation |
| **Memory access** | Acceso a memoria sensible fuera del target | Information disclosure |
| **Session hijacking** | Robo de sesión de debug | Unauthorized debugging |

### 9.2 Mitigaciones

#### Sandboxed Expression Evaluation

```rust
use wasmer::{Store, Instance, Module, Imports};
use wasmer_wasi::WasiState;

pub struct SandboxedEvaluator {
    store: Store,
    module: Module,
}

impl SandboxedEvaluator {
    pub fn new() -> Result<Self, TraceError> {
        // Crear store con límites de memoria
        let store = Store::default();
        
        // Cargar módulo WASM con expresión evaluador
        let module = Module::from_file(&store, "eval_sandbox.wasm")
            .map_err(|e| TraceError::InternalError(e.to_string()))?;
        
        Ok(Self { store, module })
    }
    
    pub fn evaluate(&self, expr: &str, context: &VariableBindings) 
        -> Result<String, EvalError> {
        
        // Crear WASI con filesystem limitado
        let wasi = WasiState::new("eval")
            .env("PATH", "")  // Sin filesystem
            .memory_limit(64 * 1024)  // 64KB limit
            .build()
            .map_err(|e| EvalError::InternalError(e.to_string()))?;
        
        let mut imports = Imports::new();
        imports.define("env", "print", Function::new_native(&self.store, |s: String| {
            println!("{}", s);
        }));
        
        let instance = Instance::new(&self.store, &self.module, &imports)
            .map_err(|e| EvalError::InternalError(e.to_string()))?;
        
        // Ejecutar evaluación en sandbox
        let evaluate = instance.exports.get_function("evaluate")
            .map_err(|e| EvalError::InternalError(e.to_string()))?;
        
        let result = evaluate.call(&[
            Value::I32(context.as_wasm_ptr()),
        ]);
        
        // El resultado nunca ejecuta código fuera del sandbox
    }
}
```

#### Read-Only Trace Access

```rust
// El MCP server solo tiene acceso de lectura a la traza
pub struct ReadOnlyTraceHandle {
    trace_path: PathBuf,
    // No hay forma de modificar datos
}

impl ReadOnlyTraceHandle {
    pub fn query(&self, query: &TraceQuery) -> Result<Vec<TraceEvent>, TraceError> {
        // Solo lectura, no hay write/delete/update
    }
}
```

#### eBPF Verifier Integration

```rust
// Antes de cargar un programa eBPF, verificar con el kernel verifier
pub async fn load_ebpf_program(
    &self,
    code: &[u8],
) -> Result<ebpf::Program, TraceError> {
    // 1. Enviar al kernel verifier
    let verifier_result = self.ebpf_loader
        .load(code)
        .map_err(|e| {
            match e {
                ebpf::Error::VerificationFailed(log) => {
                    // Log del verifier para debugging
                    TraceError::InvalidEbpFProgram(log)
                }
                _ => TraceError::InternalError(e.to_string())
            }
        })?;
    
    // 2. Solo cargar si el verifier approve
    Ok(verifier_result)
}
```

#### Audit Logging

```rust
pub struct AuditLogger {
    log_path: PathBuf,
}

impl AuditLogger {
    pub fn log_query(
        &self,
        session_id: &str,
        tool: &str,
        query: &serde_json::Value,
        result: &Result<(), TraceError>,
    ) {
        let entry = AuditEntry {
            timestamp: Utc::now(),
            session_id: session_id.to_string(),
            tool: tool.to_string(),
            query: query.clone(),
            success: result.is_ok(),
            error: result.err().map(|e| e.to_string()),
            client_ip: get_client_ip(),
        };
        
        // Escribir a log inmutable
        let json = serde_json::to_string(&entry).unwrap();
        append_to_file(&self.log_path, json);
    }
}
```

---

## PARTE X: INTEGRACIÓN CI/CD

### 10.1 Automated Debugging en Pipeline

```
┌─────────────────────────────────────────────────────────────────────────┐
│                    CI/CD PIPELINE CON CHRONOS                           │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                          │
│  ┌─────────┐     ┌─────────┐     ┌─────────┐     ┌─────────┐         │
│  │  Build  │────→│  Test   │────→│ Capture │────→│  AI     │         │
│  │         │     │         │     │  Trace  │     │ Analyze │         │
│  └─────────┘     └─────────┘     └─────────┘     └────┬────┘         │
│                                                       │               │
│       Test Failure ───────────────────────────────────┘               │
│              │                                                              │
│              ▼                                                              │
│  ┌─────────────────────────────────────────────────────────┐            │
│  │  Chronos MCP Server                                     │            │
│  │                                                          │            │
│  │  1. Graba traza de test fallido                         │            │
│  │  2. AI Agent consulta traza                              │            │
│  │  3. Identifica root cause                                │            │
│  │  4. Genera bug report con fix suggestion                 │            │
│  │                                                          │            │
│  └─────────────────────────────────────────────────────────┘            │
│                          │                                               │
│                          ▼                                               │
│  ┌─────────────────────────────────────────────────────────┐            │
│  │  GitHub Issue / PR con:                                  │            │
│  │  - Root cause analysis                                   │            │
│  │  - Stack trace relevante                                  │            │
│  │  - Fix suggestion                                         │            │
│  │  - Trace ID para referencia                               │            │
│  └─────────────────────────────────────────────────────────┘            │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

```yaml
# .github/workflows/debug-ai.yml
name: AI Debug Analysis

on:
  workflow_run:
    workflows: ["Tests"]
    types: [completed]

jobs:
  analyze-failures:
    if: github.event.workflow_run.conclusion == 'failure'
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v4
      
      - name: Start Chronos Server
        run: |
          docker run -d --name chronos \
            -p 8080:8080 \
            chronos-mcp-server
      
      - name: Run tests with trace capture
        run: |
          cargo test 2>&1 | tee test_output.log
          
          # Si hay falla, capturar trace
          if grep -q "test result: FAILED" test_output.log; then
            # Grabar trace del test fallido
            curl -X POST http://localhost:8080/debug_run \
              -d '{"target": "cargo", "args": ["test"], "capture_config": {...}}'
          fi
      
      - name: AI Analysis
        run: |
          # Enviar traza a AI agent para análisis
          curl -X POST http://localhost:8080/debug_analyze_memory \
            -d '{"session_id": "...", "address": "..."}'
      
      - name: Create GitHub Issue
        if: failure()
        uses: actions/github-script@v7
        with:
          script: |
            github.issues.create({
              title: "Bug: Test failure detected",
              body: "Chronos trace analysis: ..."
            })
```

### 10.2 Trace Regression Testing

```rust
pub struct GoldenTraceManager {
    store: TraceStore,
    baseline_path: PathBuf,
}

impl GoldenTraceManager {
    /// Registra una traza como baseline para un test
    pub fn register_golden_trace(
        &self,
        test_name: &str,
        trace: &TraceFile,
    ) -> Result<(), TraceError> {
        let key = format!("golden:{}", test_name);
        self.store.store_trace_with_key(trace, &key)?;
        
        // Guardar metrics del baseline
        let metrics = self.compute_metrics(trace);
        self.store.save_metrics(&key, &metrics)?;
        
        Ok(())
    }
    
    /// Verifica que una nueva ejecución sea equivalente al baseline
    pub fn verify_equivalence(
        &self,
        test_name: &str,
        current: &TraceFile,
    ) -> Result<VerificationResult, TraceError> {
        let golden = self.store.load_trace(&format!("golden:{}", test_name))?;
        
        let comparator = GoldenComparator::new(&golden, current);
        let diff = comparator.compare(current);
        
        Ok(VerificationResult {
            equivalent: diff.divergence_point.is_none(),
            diff,
            can_auto_merge: diff.divergence_point.is_none(),
        })
    }
}
```

```yaml
# En el proyecto, CI verifica:
test_regression:
  script:
    - cargo test --no-run
    - chronos capture --output=current.trace -- cargo test
    - chronos verify --golden=tests/golden/my_test.trace current.trace
    
  artifacts:
    when: always
    paths:
      - "*.trace"
      - "test_results/"
```

---

## PARTE XI: ROADMAP DE IMPLEMENTACIÓN

### Fase 1 (4 semanas): MVP — Native Languages

**Objetivo:** Captura básica de trazas para C/C++/Rust via ptrace

| Task | Descripción | Entregable |
|------|-------------|------------|
| 1.1 | Implementar NativeTraceAdapter básico | ptrace capture working |
| 1.2 | Diseño e implementación de FlatBuffers schema | Schema compilable |
| 1.3 | MCP server básico con debug_run | Server responde a commands |
| 1.4 | Shadow Index básico (BTreeMap) | Index para addresses |
| 1.5 | Expression eval simple (DWARF) | `print x` funciona |
| 1.6 | Tests de integración | Tests pasando |

**Dependencias Rust:**
```toml
[dependencies]
# MCP
mcp = "0.1"
tokio = { version = "1", features = ["full"] }

# FlatBuffers
flatbuffers = "24"

# DWARF parsing
gimli = "0.29"
object = "0.32"

# eBPF (para fase 2)
aya = "0.11"

# Storage
rocksdb = "0.21"

# Serialization
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Async
futures = "0.3"
```

### Fase 2 (4 semanas): Query Engine + eBPF

**Objetivo:** Consulta avanzada y probes no-intrusivos

| Task | Descripción | Entregable |
|------|-------------|------------|
| 2.1 | Temporal Index con chunking | Búsqueda por tiempo rápida |
| 2.2 | Causality Index | `find_last_mutation` funciona |
| 2.3 | Performance Index (perf_event_open) | HW counters disponibles |
| 2.4 | eBPF integration via Aya | uprobes sin ptrace |
| 2.5 | Semantic compression layer | Resumen → hotspot → detail |
| 2.6 | MCP tools avanzados | todas las tools de query |

### Fase 3 (4 semanas): Multi-Language — Python + JavaScript

**Objetivo:** Soporte para lenguajes interpretados

| Task | Descripción | Entregable |
|------|-------------|------------|
| 3.1 | PythonTraceAdapter (sys.settrace) | Captura de frames |
| 3.2 | Python expression eval (eval in frame) | `eval("x + y")` funciona |
| 3.3 | Node.js Trace Adapter (CDP) | Attach a Node process |
| 3.4 | V8 expression eval | `evaluateOnCallFrame` funciona |
| 3.5 | Language auto-detection | Auto-detecta lenguaje |

### Fase 4 (4 semanas): Multi-Language — Java + Go

**Objetivo:** Soporte para JVM y Go

| Task | Descripción | Entregable |
|------|-------------|------------|
| 4.1 | JVMTI agent development | Java agent JAR |
| 4.2 | JVMTraceAdapter + JDI | Captura Java |
| 4.3 | GoTraceAdapter (Delve API) | Captura Go |
| 4.4 | Expression adapters JVM/Go | Full expression eval |
| 4.5 | Goroutine tracking | Goroutines visibles |

### Fase 5 (4 semanas): Persistence + Historical

**Objetivo:** Almacenamiento y comparación de trazas

| Task | Descripción | Entregable |
|------|-------------|------------|
| 5.1 | RocksDB integration | Trazas persistidas |
| 5.2 | CAS implementation | Deduplicación |
| 5.3 | Golden trace comparison | Diff entre trazas |
| 5.4 | Historical diff tool | compare_historical_executions |
| 5.5 | CI/CD integration examples | GitHub Actions workflow |

### Fase 6 (4 semanas): Production Hardening

**Objetivo:** Lista para producción

| Task | Descripción | Entregable |
|------|-------------|------------|
| 6.1 | Security audit | Reporte de seguridad |
| 6.2 | Performance optimization | ART index, SIMD ops |
| 6.3 | Docker/Kubernetes deployment | Helm chart |
| 6.4 | SDK documentation | docs.rs + examples |
| 6.5 | Load testing | Benchmarks públicos |

---

## PARTE XII: PREGUNTAS ABIERTAS

### 12.1 Formato de Traza Unificado

**Pregunta:** ¿Puede un único formato de traza cubrir eficientemente lenguajes nativos y gestionados?

**Análisis:** Los lenguajes nativos (C/C++/Rust) tienen acceso a memoria cruda, registros, syscalls. Los lenguajes gestionados (Java, Python, JS) tienen objetos, GC, y abstracciones de más alto nivel.

**Posibles soluciones:**
1. **Formato genérico con extensiones**: El schema base cubre lo común, cada adapter añade sus campos específicos via FlatBuffers union
2. **Multi-format**: Cada lenguaje tiene su propio schema, el MCP server traduce al consultar
3. **Formato "any"**: Almacenar eventos como JSON crudo para máximo flexibility, con index pre-computado para queries

**Decisión pendiente:** Evaluar trade-off entre performance (FlatBuffers binario) y flexibilidad (JSON).

### 12.2 Memoria en Lenguajes con GC

**Pregunta:** ¿Cómo manejar lenguajes con GC (Java, Python, Go) donde las direcciones de memoria cambian?

**Análisis:** El GC puede mover objetos, invalidando addresses capturadas en la traza.

**Posibles soluciones:**
1. **Handles en vez de addresses**: Usar identificadores lógicos en vez de direcciones físicas
2. **Snapshot Isolation**: Capturar heap snapshots atómicos junto con la traza
3. **Object IDs persistentes**: El runtime asigna IDs que sobreviven al GC

**Decisión pendiente:** Estudiar cómo Chronon (Java) maneja esto.

### 12.3 Sandboxed Expression Eval

**Pregunta:** ¿Vale la pena compilarse a WASM para sandboxing de expression eval?

**Análisis:** WASM proporciona isolation fuerte pero añade overhead de serialización.

**Posibles soluciones:**
1. **WASM sandbox**: Máximo isolation, ~2-5x overhead
2. **Restricted eval nativo**: Timeout + restrictedbuiltins, ~1.1x overhead
3. **Interpreter embebido**: Usar un interpreter simple (e.g., muforth) sin acceso al sistema

**Decisión pendiente:** Probar ambas approaches con benchmarks reales.

### 12.4 Deployment Model

**Pregunta:** ¿Debería el MCP server ser un proceso separado o embebible como library?

**Análisis:** Los agentes pueden querer:
- Ejecutar como servicio standalone (multiple agents comparten)
- Embeberse en el agente (para casos offline)
- Deployment en containers vs in-process

**Posibles soluciones:**
1. **Dual-mode**: Librería con API syn/async + server wrapper
2. **RPC-based**: Siempre servidor, comunicación via gRPC/Unix socket
3. **Plugin system**: Server cargable como plugin del agente

**Decisión pendiente:** Definir modelo inicial basado en casos de uso prioritarios.

---

## APÉNDICE A: Dependencias Rust (Cargo.toml completo)

```toml
[package]
name = "chronos-mcp"
version = "0.1.0"
edition = "2021"
authors = ["Chronos Team"]
description = "MCP Debugger Server with Time-Travel Debugging"

[dependencies]
# ============== CORE ==============
tokio = { version = "1.36", features = ["full"] }
futures = "0.3"
anyhow = "1.0"
thiserror = "1.0"

# ============== MCP ==============
mcp = "0.1"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
async-trait = "0.1"

# ============== SERIALIZATION ==============
flatbuffers = "24"
capnp = "0.20"
rmp-ser = "0.15"  # MessagePack for Rust

# ============== DEBUGGING SYMBOLS ==============
gimli = "0.29"
object = "0.32"
addr2line = "0.21"
dwarf = "0.10"

# ============== eBPF ==============
aya = "0.11"
aya-ebpf = "0.1"
aya-log-ebpf = "0.1"

# ============== STORAGE ==============
rocksdb = "0.21"
sled = "0.34"  # alternativa a rocksdb

# ============== PROCESS CONTROL ==============
nix = "0.27"
procfs = "0.16"
libc = "0.2"

# ============== PERFORMANCE ==============
perf-event = "0.1"
rayon = "1.8"  # Parallel iteration
parking_lot = "0.12"

# ============== NETWORK ==============
tonic = "0.10"  # gRPC
hyper = "1.2"
axum = "0.7"

# ============== SANDBOXING ==============
wasmer = "4.3"
# wasmer-wasi = "4.3"
caps = "0.5"  # Linux capabilities

# ============== LOGGING ==============
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
tracing-appender = "0.2"

# ============== TESTING ==============
#[dev-dependencies]
mockall = "0.12"
tempfile = "3.9"
criterion = "0.5"

[[bin]]
name = "chronos-server"
path = "src/bin/server.rs"

[[bin]]
name = "chronos-cli"
path = "src/bin/cli.rs"

[profile.release]
opt-level = 3
lto = true
codegen-units = 1

[profile.dev]
opt-level = 0
debug = true

[build-dependencies]
capnpc = "0.20"
```

---

## APÉNDICE B: FlatBuffers Schema completo

```capnp
// chronos_trace.fbs - Complete FlatBuffers schema
// Versión: 1.0

namespace Chronos.Trace;

enum EventType : byte {
    // Syscalls
    SyscallEnter,
    SyscallExit,
    
    // Funciones
    FunctionEntry,
    FunctionExit,
    FunctionCall,
    
    // Variables
    VariableWrite,
    VariableRead,
    VariableInit,
    
    // Memoria
    MemoryAlloc,
    MemoryFree,
    MemoryWrite,
    MemoryRead,
    MemoryMap,
    MemoryUnmap,
    
    // Threads
    ThreadCreate,
    ThreadExit,
    ThreadSwitch,
    ThreadSuspend,
    ThreadResume,
    
    // Breakpoints
    BreakpointHit,
    WatchTrigger,
    StepComplete,
    
    // Excepciones
    ExceptionThrown,
    ExceptionCaught,
    ExceptionUnhandled,
    
    // Señales
    SignalDelivered,
    SignalCaught,
    
    // JIT / Dynamic code
    CodeGenerated,
    CodeUnloaded,
    
    // Custom
    Custom,
    Unknown = 255
}

enum VariableScope : byte {
    Unknown = 0,
    Local = 1,
    Global = 2,
    Static = 3,
    ThreadLocal = 4,
    Closure = 5,
    Parameter = 6
}

enum ThreadState : byte {
    Unknown = 0,
    Running = 1,
    Sleeping = 2,
    Waiting = 3,
    Zombie = 4,
    Stopped = 5,
    New = 6
}

// Location en código fuente
table SourceLocation {
    file: string (key);
    line: uint32;
    column: uint32 (key);
    function: string (key);
    function_signature: string (key);
    address: uint64;
    module: string (key);
    compilation_unit: string (key);
}

// Valor de variable
table Variable {
    name: string (key);
    value: string;
    type_name: string;
    type_size: uint64;
    address: uint64;
    scope: VariableScope = Unknown;
    is_null: bool = false;
    is_optimized_out: bool = false;
    is_constant: bool = false;
}

// Array de variables
table Variables {
    variables: [Variable];
}

// Estado de registros
table RegisterState {
    // x86-64 registers
    rax: uint64 = 0;
    rbx: uint64 = 0;
    rcx: uint64 = 0;
    rdx: uint64 = 0;
    rsi: uint64 = 0;
    rdi: uint64 = 0;
    rbp: uint64 = 0;
    rsp: uint64 = 0;
    r8: uint64 = 0;
    r9: uint64 = 0;
    r10: uint64 = 0;
    r11: uint64 = 0;
    r12: uint64 = 0;
    r13: uint64 = 0;
    r14: uint64 = 0;
    r15: uint64 = 0;
    rip: uint64 = 0;
    rflags: uint64 = 0;
    
    // Segment registers
    cs: uint64 = 0;
    ds: uint64 = 0;
    es: uint64 = 0;
    fs: uint64 = 0;
    gs: uint64 = 0;
    ss: uint64 = 0;
    
    // FP/SIMD
    st0: string = "";
    st1: string = "";
    st2: string = "";
    st3: string = "";
    
    // xmm registers (128-bit)
    xmm0: string = "";
    xmm1: string = "";
    xmm2: string = "";
    xmm3: string = "";
    xmm4: string = "";
    xmm5: string = "";
    xmm6: string = "";
    xmm7: string = "";
    xmm8: string = "";
    xmm9: string = "";
    xmm10: string = "";
    xmm11: string = "";
    xmm12: string = "";
    xmm13: string = "";
    xmm14: string = "";
    xmm15: string = "";
}

// Flags de CPU
table CpuFlags {
    carry: bool = false;
    zero: bool = false;
    sign: bool = false;
    overflow: bool = false;
    parity: bool = false;
    adjust: bool = false;
    interrupt: bool = false;
    direction: bool = false;
    trap: bool = false;
}

// Información de syscall
table SyscallInfo {
    name: string (key);
    number: uint64;
    args: [uint64];
    return_value: int64;
    error: int32 = 0;
    duration_ns: uint64 = 0;
}

// raw args como strings para debugging
table SyscallArgs {
    raw_args: [string];
}

// raw return value
table SyscallReturn {
    raw_value: int64;
    interpreted: string (key);
}

// Información de función
table FunctionInfo {
    name: string (key);
    signature: string;
    entry_address: uint64;
    exit_address: uint64;
    inline: bool = false;
    inlined_frames: [SourceLocation];
}

// Información de memoria
table MemoryInfo {
    address: uint64;
    size: uint64;
    data: [ubyte] (force_align: 16);
    
    // Para memory writes
    offset: uint64 = 0;
    prev_data: [ubyte] (force_align: 16);
    
    // Metadata
    allocation_type: string (key);
    source: string (key);  // "mmap", "malloc", "stack", etc.
}

// Para memory allocation sites
table AllocationSite {
    address: uint64;
    size: uint64;
    stack_trace: [SourceLocation];
    timestamp_ns: uint64;
}

// Información de thread
table ThreadInfo {
    tid: uint64;
    pid: uint64;
    name: string (key);
    is_main: bool = false;
    state: ThreadState = Unknown;
    
    // Scheduling
    priority: int8 = 0;
    cpu_id: int32 = -1;
    
    // Stack
    stack_base: uint64 = 0;
    stack_limit: uint64 = 0;
}

// Información de excepción
table ExceptionInfo {
    type_name: string (key);
    message: string;
    
    // Stack trace
    stack_trace: [SourceLocation];
    
    // Para excepciones encadenadas
    cause: ExceptionInfo (nested);
    
    // Handled vs unhandled
    is_uncaught: bool = true;
}

// Para signals
table SignalInfo {
    signal_number: int32;
    signal_name: string (key);
    sender_pid: uint64 = 0;
    sender_tid: uint64 = 0;
    address: uint64 = 0;
    code: int32 = 0;
}

// Breakpoint information
table BreakpointInfo {
    id: uint64;
    address: uint64;
    condition: string (key);
    hit_count: uint64 = 0;
    
    // Location
    location: SourceLocation;
}

// Watchpoint information  
table WatchpointInfo {
    address: uint64;
    size: uint64;
    access_type: string (key);  // "read", "write", "read_write"
    hit_count: uint64 = 0;
    
    // value que trigger
    value: string (key);
}

// Custom event data
table CustomEvent {
    name: string (key);
    data_json: string;
}

// Evento de traza individual
table TraceEvent {
    event_id: uint64;
    timestamp_ns: uint64;
    thread_id: uint64;
    event_type: EventType;
    
    // Location
    location: SourceLocation;
    
    // Thread state
    thread_state: ThreadState = Unknown;
    
    // Datos específicos por tipo (solo uno debe estar presente)
    syscall: SyscallInfo (key);
    function: FunctionInfo (key);
    variable: Variable (key);
    variables: Variables (key);
    registers: RegisterState (key);
    memory: MemoryInfo (key);
    thread: ThreadInfo (key);
    exception: ExceptionInfo (key);
    signal: SignalInfo (key);
    breakpoint: BreakpointInfo (key);
    watchpoint: WatchpointInfo (key);
    custom: CustomEvent (key);
    
    // Índices para búsqueda rápida
    _index: uint64 = 0;  // Index en el archivo
}

// Stack frame
table StackFrame {
    id: uint64;
    depth: uint32;
    
    // Location
    location: SourceLocation;
    
    // Frame-specific
    frame_base: uint64 = 0;
    locals: [Variable];
    params: [Variable];
    
    // Para return address
    return_address: uint64 = 0;
    return_location: SourceLocation (key);
}

// Stack trace completa
table StackTrace {
    thread_id: uint64;
    timestamp_ns: uint64;
    
    frames: [StackFrame];
    truncated: bool = false;
    max_depth_reached: bool = false;
}

// Metadata de la traza
table TraceMetadata {
    trace_id: string (key);
    session_id: string (key);
    
    // Runtime info
    language: string;
    version: string;
    runtime_name: string (key);
    runtime_version: string (key);
    
    // Binary info
    binary_path: string;
    binary_md5: string (key);
    binary_build_id: string (key);
    
    // Execution info
    pid: uint64;
    ppid: uint64;
    uid: uint64;
    gid: uint64;
    
    // Timestamps
    start_time_ns: uint64;
    end_time_ns: uint64;
    duration_ns: uint64;
    
    // Counts
    event_count: uint64;
    thread_count: uint64;
    function_count: uint64;
    syscall_count: uint64;
    
    // Configuration
    capture_config: CaptureConfig;
    
    // Platform
    os: string;
    arch: string;
    hostname: string (key);
}

// Capture configuration
table CaptureConfig {
    capture_syscalls: bool = true;
    capture_memory: bool = false;
    capture_variables: bool = true;
    capture_stack: bool = true;
    capture_registers: bool = true;
    
    max_memory_snapshots: uint32 = 0;
    max_trace_size_mb: uint32 = 1024;
    
    breakpoint_filter: [BreakpointFilter];
    syscall_filter: [string];
    function_filter: [FunctionFilter];
}

table BreakpointFilter {
    type: string (key);  // "function", "file", "address"
    pattern: string (key);
    condition: string (key);
}

table FunctionFilter {
    module: string (key);
    name_pattern: string (key);
    include: bool = true;
}

// Summary para queries rápidas
table TraceSummary {
    trace_id: string (key);
    
    duration_ns: uint64;
    event_count: uint64;
    
    // Hotspots
    top_functions: [FunctionStats];
    top_syscalls: [SyscallStats];
    
    // Memory
    total_memory_allocated: uint64;
    peak_memory_usage: uint64;
    
    // Threads
    thread_count: uint64;
    max_concurrent_threads: uint64;
    
    // Potential issues
    potential_issues: [PotentialIssue];
}

table FunctionStats {
    name: string (key);
    call_count: uint64;
    total_time_ns: uint64;
    avg_time_ns: uint64;
}

table SyscallStats {
    name: string (key);
    call_count: uint64;
    total_time_ns: uint64;
}

table PotentialIssue {
    type: string (key);
    confidence: float;
    location: SourceLocation;
    description: string;
    evidence: [string];
}

// Traza completa
table ExecutionTrace {
    metadata: TraceMetadata;
    summary: TraceSummary;
    
    // Eventos - pueden estar en archivos separados
    events: [TraceEvent];
    
    // Indices
    shadow_index: ShadowIndexData (key);
    temporal_index: TemporalIndexData (key);
    causality_index: CausalityIndexData (key);
}

// Shadow index data (para reconstruir en query)
table ShadowIndexData {
    entries: [ShadowIndexEntry];
}

table ShadowIndexEntry {
    address: uint64;
    event_ids: [uint64];
}

// Temporal index data
table TemporalIndexData {
    chunks: [TemporalChunk];
}

table TemporalChunk {
    start_ts: uint64;
    end_ts: uint64;
    first_event_id: uint64;
    event_count: uint64;
}

// Causality index data
table CausalityIndexData {
    entries: [CausalityEntry];
}

table CausalityEntry {
    variable_name: string (key);
    address: uint64;
    mutations: [MutationRecord];
}

table MutationRecord {
    event_id: uint64;
    timestamp_ns: uint64;
    thread_id: uint64;
    value_before: string (key);
    value_after: string (key);
}

root_type ExecutionTrace;

file_identifier "CHR1";
file_extension "chronos";
