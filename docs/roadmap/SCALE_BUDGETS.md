# SCALE BUDGETS — R2.1

Budgets de escala del read path de `execution_log_read`, fijados **antes** de optimizar.

> **Regla que gobierna este documento** (operador, textual): *"No optimizaria a ciegas"* y
> *"Los valores concretos deben salir de lo razonable para el producto, no de adaptar el limite
> al resultado."*
>
> Un budget que se ajusta a la medición es un budget que nunca puede fallar, y por tanto no es
> un budget. Este documento separa deliberadamente lo que **ya se puede exigir** (contratos de
> complejidad, verificables sin número) de lo que **todavía no se puede exigir** (latencias
> absolutas, declaradas ABIERTAS con el motivo por el que no son derivables hoy).

---

## 0. Qué es R2.1 y en qué estado está

R2.1 es **definir los budgets de escala antes de optimizar**, para que la optimización posterior
tenga un objetivo falsable en lugar de una impresión.

Cuatro estados, y esta tabla es la única autoridad sobre ellos:

| Estado | R2.1 |
|---|---|
| `implementado` | **Sí.** Este documento existe y fija la regla, las decisiones y lo que es exigible. |
| `verificado localmente` | **Sí, y sólo como caracterización.** Las mediciones de §3 son medidas, no proyectadas, y las afirmaciones de mecanismo de §4 están verificadas leyendo el código (§A.2). |
| `integrado` | **No.** Nada de este documento ha entrado en `main` por un gate de integración. |
| `certificado` | **No.** No hay perfil de certificación ni gate que lo cubra. |

R2.1 **no está cerrada**, y este documento lo dice por sí mismo en §10: faltan piezas, entre ellas
el enforcement de la decisión 2 y la desaparición del guard de threshold que hoy no existe (§9.2).

---

## 1. Entorno de medición, y qué cifras NO son comparables

### 1.1 Host (verificado en esta sesión)

| Dato | Valor verificado |
|---|---|
| Kernel | `7.2.7-ogc1.1.fc44.x86_64` |
| CPU | Xeon E5-2682 v4 @ 2.50 GHz, 64 cores (`nproc` = 64) |
| RAM | `MemTotal: 98737112 kB` = 94,2 GiB |
| Toolchain | `rustc 1.98.1 (48a229cea 2026-09-01)` |
| Perfil | release |

### 1.2 Las cifras de este documento NO son comparables con H1.5

El baseline H1.5 registró su fingerprint de host con **kernel 7.2.4**
(`docs/milestones/H1-CLOSE.md:33` y `:100` — *"Host fingerprint: kernel 7.2.4, Xeon E5-2682 v4
@ 2.50GHz, 64 cores, 94 GiB RAM, rustc 1.98.1"*). Misma CPU, mismos cores, misma RAM, mismo
rustc; **kernel distinto**.

**Consecuencia:** las cifras de §3 y los budgets de H1.5 §4
(`docs/architecture/H1.5-runtimes-capabilities-benchmarks.md:188-196`) no son comparables entre sí
y no deben contrastarse como si lo fueran. La razón no es cosmética: el kernel 7.2.4 → 7.2.7 es un
cambio de kernel del host, que es exactamente la clase de variable que un fingerprint de host
existe para congelar. Cuando H1.5 se certifice, lo que se reutilizará son sus **benchmarks**
(código y selección), no sus **cifras**.

Por el mismo motivo, los budgets H1.5 no pueden anclar un budget de R2.1: son de `events_read`
(otra tool), a 10k sesiones (no 1M) y en **debug build**.

### 1.3 Los números de 1M están MEDIDOS, no proyectados

Una proyección previa del worker que caracterizó la escala **subestimó el tiempo real de 1M entre
un 13 % y un 69 %**. Por eso este documento no extrapola en ninguna dirección: las cifras de §3 son
medidas, y lo que no está medido está marcado como no medido, nunca rellenado por estimación.

### 1.4 Lo que NO está medido, y no se debe presupuestar

- **`export` no está en el read path.** Vive en otro servicio
  (`crates/chronos-services/src/session_export.rs`) y **no fue medido**. Ningún budget de este
  documento le aplica.
- **El p95 de `summarize`/`rollup` a 1M no es obtenible hoy** (§8). No es un budget: es un
  requisito de diseño pendiente de resolverse antes de que ese budget pueda existir.

---

## 2. Superficie real del read path

`ReadPathService` (`crates/chronos-services/src/read_path.rs`, 460 líneas) expone **exactamente
cuatro** operaciones públicas de lectura:

| Método | Línea | Qué hace |
|---|---|---|
| `poll_batch` | `read_path.rs:155` | Lote acotado de eventos nuevos más allá del cursor, y el cursor siguiente. |
| `summarize` | `read_path.rs:179` | Recorre el log **entero** y devuelve conteos por cubos de tiempo. |
| `rollup` | `read_path.rs:190` | Recorre el log **entero** y devuelve conteos por hilo. |
| `causality_status` | `read_path.rs:205` | Si hay pistas de causalidad disponibles. |

Las cuatro se exponen al agente por **una sola tool MCP**, `execution_log_read`
(`server.rs:3625`), con cuatro modos que se enrutan **por el parámetro `mode` que elige el
agente** (`server.rs:3655` en adelante): `Poll`, `Summarize`, `Rollup`, `Causality`.

Duas consecuencias que importan para budgutar:

1. **`summarize` y `rollup` son, por contrato, recorrido total.** No son lecturas de ventana. La
   doc de `summarize` lo dice: *"Reads every page (per the `summarize_log` contract)"*
   (`read_path.rs:177`), y la de `summarize_log` lo repite: *"this function reads ALL events"*
   (`virtualization.rs:199-200`).
2. **El modo lo elige el agente, no un guard.** El handler enruta sobre `params.mode` sin comparar
   ningún `event_count` contra ningún umbral. Ver §9.2: el switch por threshold que ADR-0029
   documenta **no está cableado**.

---

## 3. Baseline medido a 1M de eventos

> **ADVERTENCIA — estas filas describen código que ya no existe.** La tabla se midió en
> `814325a8`, **antes** de R2.2 (`3c3bb256`), que sustituyó el `read_from_seq` que clonaba el log
> entero por página por una ventana en *slice*. La reconciliación está medida, no supuesta, en
> [§3.1](#31-la-tabla-de-3-mide-codigo-ya-sustituido). Las filas de `poll` y RSS **no** son
> comparables con las de `summarize`/`rollup`: R2.2 no cambió las primeras y sí las segundas.

Con backend segmentado real y registros decodificables.

| Operación | Medido a 1M (código de `814325a8`) |
|---|---|
| `poll(limit=100)` | p50 **622,5 ms** / p95 **642,5 ms** (7 reps) |
| `summarize` (cubos 1 s) | **1.222,7 s** (1 rep) |
| `rollup` (por hilo) | **1.231,8 s** (1 rep) |
| RSS residente | **~424 MB** (442,4 B/evento) |
| RSS pico (`VmHWM`) | **~843 MB** |
| First response (replay + 1er poll) | **~1.784 ms** (replay 1.451 ms + poll 333 ms) |

### 3.1 La tabla de §3 mide código ya sustituido

**La discrepancia está resuelta, y por medición directa.** Un worker que implementó D2
midió `summarize` a 1M en **11,2 s** frente a los 1.222,7 s de esta tabla, y la declaró sin
reconciliar. La hipótesis era que la tabla era anterior al arreglo del clon; se comprobó
contra el historial y luego **se midió**:

| Árbol | `read_from_seq` | `summarize` a 1M |
|---|---|---|
| `814325a8` (§3) | clonaba el log entero por página | **no completó en 600 s** |
| `c16f9913` (actual) | ventana en *slice* | **11,6 s** |

La fila de arriba se ejecutó en un worktree del mismo commit que escribió esta tabla
(`git worktree add /tmp/chronos-preR22 814325a8`), con el mismo test y la misma máquina.
Falló por timeout a los 600 s, que es el comportamiento que el propio test ya describía en su
cabecera antes de que nadie lo ejecutara.

**La aritmética explica el mecanismo y no sólo lo corrobora.** `poll(100)` son 622,5 ms:
*una* lectura de página clona 1M de registros. `summarize` recorre ~977 páginas, y cada
`read_page` hace 2 llamadas de `SCAN_CHUNK=512`; cada una clonaba el log entero. ~977 × 2 ×
622,5 ms ≈ **1.217 s**, que es el orden de magnitud exacto de los 1.222,7 s registrados. El
coste cuadrático de la agregación **era** el clon de R2.2, no la carga de la máquina.

**Consecuencias, y son las que importan:**

1. **La mejora de R2.2 es mucho mayor de lo registrado.** El cambio de `poll` se documentó como
   0,92×/0,94× de cociente cabeza/cola; para la agregación es de **>51×** frente al peor caso
   medido, y superior si se toma la cifra de §3. El arreglo no quitó una constante: quitó un
   término cuadrático.
2. **La tabla de §3 no puede seguir usándose para argumentar nada sobre el código actual.** Las
   filas de `summarize` y `rollup` miden un defecto ya resuelto. Las de `poll` y RSS no cambiaron
   con R2.2 y siguen siendo válidas como referencia — pero miden `poll`, no la agregación.
3. **El techo de 60 s de D2 no muerde a 1M en esta máquina.** No porque el techo sea excesivo,
   sino porque el recorrido cabe holgadamente. La decisión se sostiene por lo que garantiza
   (un tope con ancla de reanudación), no por una cifra que ya no aplica.

*Pendiente y **no** medido: volver a levantar el p95 de `poll` y el RSS con el arreglo en
sitio, para que las filas no-agregación tengan una cifra que sí corresponda al código actual.
No se hace aquí porque §8 ya registra que 20 muestras a 1M cuestan ~6,8 h por operación, y esa
medida pertenece a la lane de certificación, no a un cierre documental.*

### 3.2 Dos lecturas honestas de la tabla de §3

**(a) El `poll` de 333 ms de "first response" y el `poll` de 622,5 ms no son la misma medición.**
La cifra de 622,5 ms es a 1M. La de 333 ms pertenece a la cadena de first response, cuyo tamaño
de sesión **no consta** en el encargo del que proceden las cifras. No son comparables entre sí y
este documento no las usa como si lo fueran. Se registra como inconsistencia abierta (§10), no
como contradicción resuelta.

**(b) Coherencia interna que sí se puede comprobar.** El coste transitorio por llamada de `poll`
reportado (~422 MB) es aproximadamente igual al RSS residente de la sesión entera (~424 MB). Esa
es justamente la firma aritmética de un **clon completo de la sesión**: el transitorio es ~1× la
sesión, no ~1× la ventana. §4 lo confirma en el código.

---

## 4. Mecanismo del cuello de botella (`verificado localmente` por lectura de código)

### 4.1 La cadena de llamadas, extremo a extremo

Para `mode=poll`, la llamada recorre:

```
execution_log_read(mode=poll)              server.rs:3628
  └─ ReadPathService::poll_batch           read_path.rs:155
      └─ LiveStreamHandle::poll_batch_real live_streaming.rs:365
          └─ read_page                    events_log_read.rs:346
              └─ read_page_with            events_log_read.rs:381
                  └─ read(position, 512)  events_log_read.rs:405   (SCAN_CHUNK = 512, :336)
                      └─ handle.read_from_seq(position, chunk)
                          └─ SegmentedExecutionLog::read_from_seq   segmented.rs:916
                              └─ inner.backend.read_from_seq(…)    segmented.rs:932-934
                                  └─ InMemoryExecutionLog::read_from_seq   memory.rs:550
```

### 4.2 El defecto

`InMemoryExecutionLog::read_from_seq` (`crates/chronos-log/src/memory.rs:550`) **clona el `Vec`
completo de la sesión ANTES de mirar el `limit`**:

```rust
// memory.rs:558-564
let entries = {
    let records = self.records.lock().expect("records lock poisoned");
    match records.get(session_id).cloned() {   // <-- clona la sesión entera
        Some(e) => e,
        None => return Ok(LogPage::empty_at(from_seq)),
    }
};
```

y sólo después recorre ese clon aplicando el límite:

```rust
// memory.rs:571-579
for entry in entries {
    match entry {
        RecordEntry::Record(r) => {
            if r.seq < from_seq { continue; }   // filtra desde el principio del Vec
            if out_records.len() >= limit { break; }
```

Un matiz que **corrige** la descripción del encargo y que refuerza la tesis: el recorrido no
arranca en `from_seq` mediante búsqueda, sino **en el índice 0**, descartando con
`if r.seq < from_seq { continue; }` todo lo anterior. El `break` por `limit` sólo se alcanza una
vez collected `limit` registros. Por tanto el coste es **O(N de la sesión) en ambos casos**, y el
clon es el término dominante.

### 4.3 Consecuencias medidas

| Consecuencia | Evidencia |
|---|---|
| El coste es O(N del log total), no O(limit) | El clon precede al `limit` (`memory.rs:560` vs `:577`) |
| Pedir 1 evento cuesta lo mismo que pedir 100 | Mismo clon, mismo recorrido, distinto `limit` |
| El coste **no** depende de la posición del cursor | A 1M, cabeza p50 567-580 ms vs cola 581-596 ms, dentro del ruido |
| Cada llamada asigna ~422 MB transitorios | ≈ 1× el RSS de la sesión (§3.2b) |

Que el coste no dependa de la posición **no es una buena noticia hoy**: es la firma de que *todo*
es O(N). Una lectura benar acotada también sería insensible a la posición, pero por una razón
distinta y barata. La medición actual no puede distinguir las dos; por eso el contrato de §7.1
afirma la independencia de posición **junto con** la independencia de N, y no la primera sola.

### 4.4 Corrección de atribución: "backend segmentado" **no** significa "lectura por ventana"

Este punto merece quedar escrito porque es la clase de suposición que hace que un budget proteja
nada. La única implementación de `ExecutionLogFactory` en producción es
`SegmentedExecutionLogFactory` (`crates/chronos-log/src/factory.rs:42`), y
`SegmentedExecutionLog` **envuelve un `InMemoryExecutionLog`**:

```
segmented.rs:2    //! backend. It wraps an `InMemoryExecutionLog` and periodically flushes
segmented.rs:109      backend: InMemoryExecutionLog,
```

Es decir: el segmentado resuelve **durabilidad y replay** (qué hay en disco tras un reinicio), no
**coste de lectura**. La sesión completa está materializada en memoria de todas formas, y toda
lectura pasa por ella. Por eso el defecto de §4.2 está en el read path de producción y no es un
artefacto de un backend de pruebas.

### 4.5 Por qué `summarize` y `rollup` son O(N²/512)

Ambos recorren el log entero en páginas de 1024 y cada página se lee en chunks de
`SCAN_CHUNK = 512`:

- `summarize_log`: `page_limit = 1024` (`virtualization.rs:212`), bucle `while let Ok(page) =
  read_page(...)` (`virtualization.rs:214`).
- `rollup_log`: `page_limit = 1024` (`virtualization.rs:261`), bucle `read_page`
  (`virtualization.rs:263`).
- `read_page_with` itera `while matched.len() < limit` llamando `read(position, SCAN_CHUNK)`
  (`events_log_read.rs:403-405`).

La aritmética para N = 10⁶:

```
páginas           = N / 1024                    =    977
llamadas a read   = páginas × ceil(1024/512)   =  1.954
coste por llamada = O(N)                        (el clon, §4.2)
coste total                                          O(N²/512)
```

Volumen de copia derivado, con el RSS de sesión de §3 (~424 MB) y el tamaño de entrada en disco
registrado en el propio test in-tree (26.971.843 B para 10⁶ eventos = 27,0 B/evento,
`chronos-sandbox/tests/scale_execution_log_1m.rs:137-138`):

```
1.954 llamadas × ~424 MB ≈ 828 GB copiados  →  coherente con 1.222,7 s
```

La curva de crecimiento medida de 1k a 128k (p50 ms) muestra un exponente local creciente de
**1,3 a 2,5**, que es la firma de esa forma cuadrática. **El término dominante es el clon**, no el
recorrido de páginas.

### 4.6 La palanca

**Un solo arreglo cierra los dos problemas.** Si `read_from_seq` deja de clonar la sesión
completa y resuelve la ventana por índice, entonces:

- `poll` pasa de O(N) a O(ventana) — y con ello `summarize`/`rollup` pasan de O(N²/512) a
  O(N) (≈977 ventanas × 512, lineal).
- El recorrido cuadrático no necesita un arreglo propio: desaparece como consecuencia.

Es la mayor palanca de rendimiento por línea tocada de todo el read path, y es la razón por la
que §7 propone un contrato de complejidad y no una lista de optimizaciones.

---

## 5. Las TRES decisiones del operador

Se registran como **decisiones tomadas**, no como proposals. No admiten discusión en este ciclo.

### D1 — Alcance de lectura: VENTANA ACOTADA

El agente lee franjas. **NO** se promete drenar la sesión entera de forma secuencial.

Consecuencia directa: el número de páginas que el agente recorrerá **no es una constante del
producto**, sino una elección suya. Todo budget derivado de "las 10.000 páginas de un drenaje
completo" queda sin anclas (§6.1).

### D2 — El techo de 60 s se extiende de captura a lecturas

`ResourceLimits { max_events: 1_000_000, timeout_secs: 60 }` (`server.rs:123-138`) queda como
**techo duro de cualquier operación del read path**, no sólo de captura.

**Estado de esta decisión: decidida, sin mecanismo.** `ResourceLimits` no se instancia en ningún
punto de producción: sus únicas apariciones fuera de su propia definición son dos funciones
`#[test]` (`server.rs:6744` y `server.rs:6751`). Nada en el read path consulta `timeout_secs`.
Ver §9.3 — se registra como pendiente de implementación, **no** como afirmación falsa, porque
nadie ha documentado que esté aplicado.

### D3 — Bucketing RELATIVO al inicio de la sesión

Hoy `summarize_log` indexa `bucket_counts` por el **número absoluto de cubo**:

```rust
// virtualization.rs:220-223
let bucket = ts / bucket_size_ns;   // ts en ns epoch
let idx = bucket as usize;
if idx >= bucket_counts.len() {
    bucket_counts.resize(idx + 1, 0);
```

La memoria es por tanto **O(índice absoluto)**, no O(cubos distintos). Como `bucket_counts` es un
`Vec<u64>` (8 B por entrada) y `bucket_size_ns` por defecto es 1 s
(`tools_params.rs:394-396` y `virtualization.rs:189`):

```
entradas = ts_max / 1 s        → ≈ 1,76 × 10⁹ para una captura con timestamps epoch
memoria  = 8 B × 1,76 × 10⁹   → ≈ 14 GB
```

(Las ~13,6 GB del encargo corresponden a un epoch algo anterior; la cifra es aritmética
dependiente del epoch de captura, **no una medición**, y por eso se deriva aquí en forma de
fórmula y no como número fijo.)

Con D3 el bucketing pasa a ser relativo a la sesión y la memoria depende de los **cubos
realmente poblados**.

**Aviso de cobertura:** el test in-tree de 1M **no puede** detectar este fallo. Siembra
`monotonic_ns: i * 1_000` y `timestamp_ns: MonotonicNs::from(event_id * 1_000)`
(`scale_execution_log_1m.rs:124` y `:86`), es decir timestamps que llegan hasta 10⁹ ns ≈ **1
segundo**, luego el índice absoluto máximo es 1. D3 responde a un modo de fallo que la
caracterización actual no ejercita. Ver §9.4.

---

## 6. Re-derivación: qué budgets sobreviven a D1

### 6.1 B1 — "6 ms por página": RETIRADO

El worker previo propuso un budget de **6 ms por página**, derivado de `60 s / 10.000 páginas`.

**Ese budget ya no se sostiene y no debe repetirse como si siguiera vigente.** Su derivación
asumía el drenaje secuencial completo de la sesión, que D1 acaba de descartar. Peor: bajo D1 es
**innecesario**, no sólo injustificado. El techo de 60 s de D2 acota una *operación*, no una
página; si el techo se aplica por llamada, el agente no puede quedar atrapado por el coste de una
página individual, y un budget por página no añade garantía. Se retira sin sustituto.

### 6.2 B2 — Latencia absoluta de `poll` por ventana: **ABIERTO**

Un presupuesto en milisegundos para la lectura por ventana **no es derivable hoy**, y se declara
abierto en vez de inventarse. El motivo es concreto y no es pereza:

> Todas las mediciones de `poll` disponibles están contaminadas por el clon O(N) de §4.2. No
> existe ninguna medición del coste de una lectura **ventaneada**, porque esa lectura todavía no
> existe en el código. Fijar un número ahora sería inventar un ancla; ajustarlo después sería
> exactamente el "adaptar el limite al resultado" que la regla de §0 prohíbe.

Anclas que existen y **no** sirven aquí, y por qué:

| Ancla candidata | Por qué no ancla B2 |
|---|---|
| `poll` p50 622,5 ms a 1M | Es el coste **con** el clon. Es una cota superior que se derrumbará al arreglarlo; budgetizarla hoy sería presupuestar un defecto. |
| `poll` 333 ms de first response | Sesión de tamaño no consta (§3.2a) y también está contaminada por el clon. |
| H1.5 `events_read` ≤ 50 ms a 10k | Otra tool, otro tamaño, **debug build**, y kernel 7.2.4 (§1.2). |
| Techo de 60 s de D2 | Es un **techo**, no un objetivo de rendimiento. Acota el daño; no dice cuál es el coste razonable de una ventana. |

**Cómo se cierra B2:** medir el coste de `poll` una vez eliminado el clon (§4.6), con el
contrato C1 de §7.1 GREEN, y entonces derivar el número desde la latencia observada **más** el
margen de producto, no al revés.

### 6.3 Lo que sí se deriva: contratos de complejidad (§7)

Un contrato de complejidad es más fuerte y más testeable que un número en milisegundos: no
depende del host, no se ajusta a la medición, y no puede pasar mientras el defecto siga presente.

---

## 7. Los contratos de complejidad exigibles hoy

Cada uno está redactado para convertirse en un test sin decisiones adicionales. Los tres atacan
la misma causa raíz (§4.6).

### 7.1 C1 — `poll` debe costar O(ventana), no O(N de la sesión) — **CONTRATO PRINCIPAL**

> **Enunciado.** Sea `L` un límite de ventana fijo y `C` un cursor. El coste de una llamada a
> `execution_log_read(mode=poll, limit=L, cursor=C)` **no crece con N**, el número total de
> eventos de la sesión, para ninguna posición de `C`.
>
> **Forma de test.** Escalar, no umbral.
>
> 1. Sembrar la **misma sesión lógica** en tres tamaños N₁ = 10⁴, N₂ = 10⁵, N₃ = 10⁶, con `L`
>    fijo y `C` en la cabeza en los tres casos. Medir `c(N)` como mediana de ≥ 7 llamadas (misma
>    tolerancia de reps que el `poll` de §3).
> 2. **Afirmar** `c(N₃) / c(N₁) ≤ K`, donde `K` es una constante **declarada e independiente de
>    N**.
> 3. La aserción que no depende de `K` y es la que porta el contrato: el cociente
>    `c(N₃)/c(N₁)` **no debe ser proporcional a** `N₃/N₁` (= 100). Con el clon de §4.2 lo es: el
>    coste crece linealmente en N, luego el cociente es ≈ 100. Satisfecho el contrato, la curva
>    es plana en N.
>
> **Requisito de validez del test (descubierto al verificar §4.4).** El log debe construirse con
> el backend que el **composition root de producción** construye, es decir
> `SegmentedExecutionLogFactory` (`factory.rs:42`). Un test que mida el proveedor puramente
> in-memory probaría un camino que producción no usa y el budget no protegería nada.
>
> **Estado.** Hoy FAIL por construcción: el cociente es ≈ 100.

### 7.2 C2 — `poll` debe ser sensible a `L`, no ciego a `L`

> **Enunciado.** A N fijo, el coste de `poll` debe crecer con el límite `L`, porque lo que se
> decodifica es la ventana.
>
> **Forma de test.** A N fijo, medir `cost(L=1)` y `cost(L=100)`. La firma **defectuosa** es
> `cost(1) ≈ cost(100)` (el clon precede al `limit`, así que el `limit` es irrelevante). La firma
> **correcta** es `cost(100)` claramente mayor que `cost(1)`.
>
> Es más barato que C1 (no necesita 10⁶), y discrimina el mismo defecto. Se propone como test
> rápido de regresión; C1 es el que cierra el gate.

### 7.3 C3 — Los agregados deben ser O(N), no O(N²/512)

> **Enunciado.** El coste total de `summarize` (y de `rollup`) debe ser lineal en N.
>
> **Forma de test.** Con N y 2N sembrados, afirmar que `coste(2N)/coste(N)` está acotado por una
> constante declarada. El exponente local observado hoy crece hasta **2,5** (§4.5); conforme al
> contrato, la curva debe tender a exponente 1.

### 7.4 C4 — `read_after` debe costar O(nuevo), no O(N de la sesión) — **CERRADO 2026-10-03**

> **Enunciado.** El coste de `read_after` es proporcional a lo que **devuelve**, no al tamaño de la
> sesión. Un consumidor que ha alcanzado la cola no devuelve nada y no debe pagar nada.
>
> **Por qué es un contrato y no una optimisation.** El estado que importa no es la primera lectura
> sino la de **reposo**: un consumidor atrapado en la cola pregunta "qué hay nuevo" en cada tick.
> Con el clon, esa llamada asignaba el log entero para descubrir que no había pasado nada.

**Medido antes y después**, con el allocator contando, mismo consumidor atrapado en la cola:

| N | antes (B por llamada) | después (B por llamada) |
|---|---|---|
| 20 | 4.910 | 30 |
| 200.000 | **48.800.030** | **30** |
| cociente N-grande/N-pequeño | 9.939x (sesión 10.000x mayor) | **1,0x** |

Y en **tiempo**, que es la capa que un contador de asignaciones no ve:

| N | antes | solo sin el clon | después |
|---|---|---|---|
| 20 | 0,002 ms | 0,002 ms | 0,002 ms |
| 200.000 | 9,455 ms | 3,832 ms | **0,002 ms** |
| 1.000.000 | 51,236 ms | 23,353 ms | **0,003 ms** |

La columna intermedia importa: **quitar el clon no basta**. C1/C2 ya lo advertían —"invisible to
allocations, which is why the cost-is-flat-in-N assertion here is a necessary but not sufficient
guard"— y la medición lo confirma. Quedaban dos recorridos O(N): el mínimo de `oldest_seq` y el
bucle de filtrado. El primero se cierra cortocircuitando con el invariante `oldest_seq <= tail_seq`;
el segundo, con el mismo `first_reachable(last_seq + 1)` que `read_from_seq` ya usaba, porque ambas
ramas del filtro reducen a la misma predicción `reach() >= last_seq + 1`.

**El coste ya no sigue a la sesión**, que es literalmente el enunciado del contrato. A 1M son
0,003 ms frente a 51,2 ms: ~17.000x, y plano en vez de lineal.

**Vive en:** `crates/chronos-log/tests/c4_read_after_cost_contract.rs` (un solo `#[test]`, porque el
allocator que cuenta es process-global) y
`crates/chronos-log/tests/read_after_seek_equivalence.rs` + los dos tests in-crate del módulo
`memory::c1_c2_read_from_seq`. La afirmación de que el **tiempo** es plano no es un test: es una
medición, y por eso vive en `crates/chronos-log/examples/timing_probe.rs`. Una aserción de reloj en
el lazo rápido sería un gate flaky, que es exactamente lo que C1 se niega a ser.

### 7.5 Dónde viven estos tests

En el target ya existente y **ya fuera del hot path**:
`chronos-sandbox/tests/scale_execution_log_1m.rs`, que es su propio target y por tanto no alarga
el gate (`cargo test -p chronos-sandbox --test execution_log_read_e2e` es lo que corre el gate),
con `#[ignore]` y su coste registrado. Es el hogar coherente con la convención del repo.

> **Punto de coordinación.** Ese fichero está implicado en la lista de skips de CI, que se deriva
> de `reconstruction-contracts.toml` para tests diferidos que **fallan**. Añadir tests C1/C2/C3
> puede rozar ese contrato. No he tocado `reconstruction-contracts.toml` (propiedad de otro
> worker); lo dejo señalado para que la coordinación decida antes de escribir los tests.

---

## 8. Un budget que hoy no puede existir: el p95 de `summarize`/`rollup` a 1M

No es un budget pendiente; es un **requisito de diseño** que debe resolverse antes de que ese
budget pueda fijarse.

Una sola llamada a `summarize` a 1M cuesta ~1.223 s. Una distribución p95 exige 20 muestras como
mínimo:

```
20 × 1.222,7 s ≈ 24.454 s ≈ 6,8 h   por operación
20 × 1.231,8 s ≈ 24.636 s ≈ 6,8 h   por operación
```

~6,8 h por operación, ~13,6 h para ambas, **en un host ocioso y exclusivo**. Budgetizar un p95
que costaría medio día de cómputo por medición es un budget que nadie va a medir, y por tanto un
budget que nunca va a fallar: es la forma exacta del defecto que la regla de §0 prohíbe.

**Opciones para el gate de R2.1** (a decidir por el operador; este documento no las elige):
reducir el p95 a un tamaño representativo y declarar la extrapolación como no medida; sustituir
p95 por una aserción de coste total sobre un único caso de 1M; o arreglar §4.6 primero y
re-medir. En las tres, la cifra de 1.223 s sigue siendo el dato que hay que tener delante.

---

## 9. Afirmaciones documentadas que no se sostienen

Registradas porque un budget construido sobre una premisa falsa no es un budget.

### 9.1 `CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD` no existe

Se documenta como override por despliegue en:

- `docs/chronos-agentic-reconstruction/docs/adr/0029-...md:72` (fila M10.4): *"threshold switch
  page↔summary (default 100_000, env `CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD`)"*.
- `docs/milestones/M10-SCOPING.md:89` (§4.4 D4): *"configurable via env
  `CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD`"*.
- Dos comentarios de código: `virtualization.rs:22` y `:64`, y `tools_params.rs:391`.

**Verificado: no se lee en ninguna parte.** El nombre sólo aparece en prosa. No hay ningún
`env::var` del repositorio que mencione un threshold o una virtualización; los `env::var` reales son
`XDG_DATA_HOME`, `HOME`, `CHRONOS_EXECUTION_LOG_DIR`, `CHROME_PATH_ENV`, `CHRONOS_ACTIVE_TOOLSET`,
`CHRONOS_DB_PATH`, `CHRONOS_ALLOW_IN_MEMORY_FALLBACK`, `CHRONOS_AUTO_COMPACT_INTERVAL_SECS`,
`CHRONOS_E2E` y `CARGO_MANIFEST_DIR`.

### 9.2 El switch por threshold tampoco está cableado — más grave que 9.1

`should_summarize` es una función pura que **recibe `threshold` como parámetro**
(`virtualization.rs:71`), y `DEFAULT_VIRTUALIZATION_THRESHOLD` (`virtualization.rs:65`) sólo se
referencia en su propia definición y en **una** aserción de test (`virtualization.rs:405`).
`should_summarize` no tiene **ningún** call site de producción: sus referencias están en su
definición y en los tests del mismo fichero, más una mención en prosa en `read_path.rs:178` que
justamente **difiere la decisión al llamador** (*"the caller owns the `should_summarize` threshold
decision"*).

El handler `execution_log_read` enruta sobre `params.mode` sin comparar `event_count` contra nada.

**Consecuencia para R2.1, y es la relevante:** el guard que debería impedir que un agente caiga
en `mode=summarize` sobre una sesión de 1M **no existe**. Por eso la llamada de 1.222,7 s es
alcanzable en producción, y por eso un budget de 60 s sin enforcement (§9.3) deja al agente sin
ninguna red. Es la misma familia que el invariante falso ya retirado de `server.rs`: una propiedad
documentada que el código no cumple.

### 9.3 `ResourceLimits` no tiene mecanismo (esto NO es una afirmación falsa)

Distingo esto de 9.1 y 9.2 a propósito. Nadie ha documentado que el techo se aplique: al
contrario, el propio doc de `ResourceLimits` (`server.rs:120-122`) lo describe como *"limits for
capture operations"*, que es exactamente lo que es hoy. La decisión D2 **extiende** su alcance, y
esa extensión es trabajo pendiente, no una mentira documental.

Estado: `ResourceLimits` se instancia sólo en `server.rs:6744` y `server.rs:6751`, ambos `#[test]`
dentro de `mod tests`. Cero enforcement en el read path.

### 9.4 La caracterización in-tree de 1M no ejercita el fallo de D3

`scale_execution_log_1m.rs` siembra timestamps monotónicos desde ~0 hasta ~1 s
(`:86`, `:124`), luego el índice absoluto de cubo nunca pasa de 1. El crecimiento O(índice
absoluto) de D3 **no puede aparecer** en ese test. Además el test está `#[ignore]` (§`147-148`),
por lo que **no está en el gate**.

Consecuencia sobre sus propias aserciones: su comentario registra que `summarize` *"ran past
600s"* (`scale_execution_log_1m.rs:139-141`), pero el timeout del cliente es
`AGGREGATE_TIMEOUT_SECS = 600` (`:61`) y el resultado se desenvuelve con
`unwrap_or_else(… panic!)` (`:250`). Un `summarize` que pasa de 600 s hace fallar el test. Por
tanto **las aserciones de total de 1.000.000 para `summarize` y `rollup` no están demostradas en
verde a 1M**; las de `poll` sí se describen como correctas.

### 9.5 Discrepancia entre las dos grabaciones de 1M — abierta, no resuelta

| Fuente | Fecha | Host | `summarize` a 1M | RSS |
|---|---|---|---|---|
| Test in-tree (`scale_execution_log_1m.rs:137-141`) | 2026-10-02 | **con carga ~17** | **pasó de 600 s** (abortado por timeout) | **~1,5 GB** |
| Medición de §3 | posterior | ocioso | **1.222,7 s** (completado) | ~424 MB / pico ~843 MB |

No son necesariamente contradictorias: la grabación in-tree fue abortada a los 600 s, así que su
duración es una **cota inferior** y su RSS una **muestra a mitad de recorrido**, no un pico. Pero
el RSS es la única de las dos cifras que **no** se explica por esa asimetría: 1,5 GB a mitad de
recorrido es mayor que el pico de 843 MB de la ejecución completa, lo que no encaja con un perfil
de crecimiento monótono. No elijo ganador. Queda registrado como discrepancia abierta; no afecta
a ningún contrato de §7, que son todos relativos.

---

## 10. Qué falta para cerrar R2.1

R2.1 **no** puede declararse cerrada con este documento. Estado al 2026-10-03, sobre
`main @ 3aa99c0b`:

**Cerrados desde la redaccion de este documento:**

- **(2) y (3) — el switch por threshold y las afirmaciones falsas.** Cerrados por correccion
  documental con supersede-not-remove: `ADR-0029` §10 y `M10-SCOPING.md` §9 anaden la seccion
  correctiva (121 y 118 lineas insertadas, **0 borradas**), con evidencia `fichero:linea` y el
  comando de busqueda. No se implementa el guard: **queda SUPERADO por D1 + D2**, no pendiente.
  La decision es de lectura por ventana, no de politica por tamano, asi que el guard por umbral
  no es lo que falta: es lo que dejo de necesitar.
- **(8) en parte — los tests de contrato C1 y C2.** Escritos y verdes. C1 a 1M queda en target
  propio con `#[ignore]` y fuera del lazo rapido; C2 y el RED discriminante de forma estan en el
  lazo normal y cuestan menos de un segundo entre los dos. C3 (agregados) sigue pendiente.

**Abiertos:**

1. **Mecanismo para D2**: enforcement de `timeout_secs = 60` en las operaciones del read path
   (§5 D2, §9.3). Hoy no existe. Sigue siendo el primer pendiente.
2. ~~**Anclar B2** una vez observado el coste de `poll` sin el clon (§6.2).~~ **CERRADO por
   medición (2026-10-03).** B2 era el ancla del presupuesto de `poll` esperando el coste sin el
   clon, y ahora hay dos: el de `read_from_seq` (R2.2) y el de `read_after` (C4, §7.4). La cifra
   que faltaba era "qué paga un poll cuando no hay nada nuevo", y esa ya no depende de N.
3. **Resolver la estrategia de p95** de los agregados a 1M (§8). Bloqueante por coste, no por
   dificultad: 20 muestras a ~1.223 s son ~6,8 h por operacion. **Sin cambio de estado**: la
   medicion de una sola muestra que hay ahora abajo no es un p95 y no se presenta como tal.
4. **Caracterizar D3**: una medicion que use timestamps epoch, que la actual no ejercita (§9.4).
5. **Resolver la discrepancia de RSS** entre las dos grabaciones (§9.5). Se deja sin elegir
   ganador a proposito. **Tercera grabación el 2026-10-03** sobre el lane de 1M con C4 en sitio:
   pico de RSS **603.292 KB (~589 MB)**, pared con los ~843 MB de la grabación anterior y los
   ~1,5 GB del test in-tree con el host cargado. Las tres siguen sin un ganador, y la diferencia
   entre elles es probablemente de ruta (proceso que siembra + servidor + cliente en un caso,
   solo lectura en otro), no de código. **No se elige ganador aqui**; lo que se anota es que la
   cifra mas baja pertenece a la medicion que pasa por el camino corregido.
6. **C3** para los agregados, si se decide que los agregados son parte de la superficie
   certificable y no solo de la exploracion.

### Medición de una sola muestra, 2026-10-03, con C4 en sitio

Lane `scale_execution_log_1m` ejecutado entero de verdad (`--ignored --nocapture`):

```
seeded 1000000 events in 32.5s
segment bytes on disk: 26971843
summarize answered in 11.8s   total_events: 1000000
rollup answered in 11.8s
read path served 1000000 events: PASS   (82.32s)
Maximum resident set size: 603.292 KB
```

Dos cosas dice esto y una no:

- **Dice que el camino de lectura sirve 1M de punta a punta con C4 en sitio**, que es lo que el
  arreglo tenía que preservar. `summarize` 11,8 s y `rollup` 11,8 s son consistentes con los
  11,6 s que R2.3 registro, o sea **neutro**: C4 no toca la agregacion, que va por
  `read_from_seq`. Lo que arregla es el camino de **consumidor**, el de `read_after`.
- **No dice nada del p95.** Una muestra no es una distribucion, y §8 sigue_blocked por coste. La
  cifra de 11,8 s es un punto, no un percentil, y no se usa como si lo fuera.

**Hallazgos posteriores que este documento no recogia:**

- `record_gap` acepta un gap que **solapa** registros ya escritos. El log resultante es evidencia
  contraditoria y `build_replay_plan` lo rechaza, o sea que **el log deja de poder reabrirse**.
  Un gap *retroactivo* (empieza antes del allocator pero termina antes del ultimo registro) si es
  legitimo y no debe rechazarse: es justo el caso que `reach_disordered` maneja. Confundir las
  dos cosas dejaria el flag permanentemente activo y C1 volveria a fallar.
- `should_summarize` **no tiene ningun campo `oversized`** y ningun call site de produccion. Se
  verifica que `EventBatch` tiene exactamente dos campos (`next_cursor`, `events`); el unico
  `oversized` del workspace es un comentario de test no relacionado. El discriminante `mode` si
  esta cableado y las cuatro vistas tienen implementacion real: lo que falta es la *decision
  automatica* de conmutar por tamano.
- `M10-SCOPING.md:3` sigue afirmando que no se ha ejecutado ningun sub-cycle de M10. Era exacto
  cuando se escribio (2026-09-22) y hoy es **obsoleto por fecha**, no falso en su momento.

Mientras 1..8 no estén cerrados, este documento es un **contrato de entrada**, y R2.1 está
`implementado` y `verificado localmente` como caracterización. Nada `integrado`, nada
`certificado`.

---

## Anexo A. Verificación de este documento

### A.1 Comandos ejecutados en esta sesión

Todos desde `/var/mnt/DiscoChino2-fast/Proyectos/rust/chronos`, sobre `main` @ `3d05d175`, árbol
de trabajo limpio al empezar. Ninguno modificó el repositorio.

| # | Comando | Exit |
|---|---|---|
| 1 | `git status --porcelain` | 0 (sin salida: limpio) |
| 2 | `git rev-parse --abbrev-ref HEAD` | 0 (`main`) |
| 3 | `git log -1 --oneline` | 0 (`3d05d175`) |
| 4 | `ls docs/roadmap/` + test de existencia de `SCALE_BUDGETS.md` | 0 (confirmado nuevo) |
| 5 | `grep -rn "CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD"` (workspace) | 0 (§9.1) |
| 6 | `grep -rn "env::var\\|env(" --include=*.rs crates/ \\| grep -i "virt\\|thresh"` | 1 (sin coincidencias) |
| 7 | `grep -rn "env::var" --include=*.rs crates/` | 0 (inventario completo, §9.1) |
| 8 | `sed -n '110,150p' server.rs` + `sed -n '380,400p' tools_params.rs` + `sed -n '55,75p' virtualization.rs` | 0 |
| 9 | `grep -n "pub struct ResourceLimits" -A 12 server.rs` | 0 (`123`-`135`) |
| 10 | `grep -n "pub fn default_limit\\|pub fn default_bucket_size_ns\\|DEFAULT_VIRTUALIZATION_THRESHOLD" …` | 0 |
| 11 | `grep -n "SCAN_CHUNK" events_log_read.rs` | 0 (`336`) |
| 12 | `grep -n "pub async fn poll_batch\\|… summarize\\|… rollup\\|… causality_status\\|… export" read_path_service.rs` | 2 (fichero no existe) → se localizó `read_path.rs` |
| 13 | `find . -name "session_export.rs"` | 0 |
| 14 | `grep -rn "execution_log_read" --include=*.rs crates/chronos-mcp/src/` | 0 |
| 15 | `grep -n "page_limit = 1024\\|while let Ok(page) = read_page\\|let bucket = ts\\|bucket_counts.resize" virtualization.rs` | 0 |
| 16 | `grep -rn "VIRT_THRESHOLD" -B 6 -A 3 …0029-…md` | 2 (ruta mal indicada) → re-ejecutado en `docs/` |
| 17 | `grep -n "VIRT_THRESHOLD" -B 6 -A 3 docs/…/0029-…md` | 0 (fila M10.4, línea 72) |
| 18 | `sed -n '85,95p' docs/milestones/M10-SCOPING.md` | 0 (línea 89) |
| 19 | `grep -rn "should_summarize" --include=*.rs .` | 0 (§9.2) |
| 20 | `grep -rn "DEFAULT_VIRTUALIZATION_THRESHOLD" --include=*.rs .` | 0 (§9.2) |
| 21 | `sed -n '3620,3720p' server.rs` | 0 (enrutado de los 4 modos) |
| 22 | `grep -rn "ResourceLimits" --include=*.rs crates/` | 0 (§9.3) |
| 23 | `grep -rn "fn poll_batch_real" -A 30 crates/` | 0 (`live_streaming.rs:365`) |
| 24 | `sed -n '6735,6762p' server.rs` + `grep -n "#\[cfg(test)\]" server.rs` | 0 (tests confirmados) |
| 25 | `awk` sobre `server.rs` para el `mod tests` envolvente | 0 (`mod tests` en `4341`) |
| 26 | `sed -n '330,420p' events_log_read.rs` | 0 (`SCAN_CHUNK`, cadena del closure) |
| 27 | `awk 'NR>=400 && NR<=410' events_log_read.rs` | 0 |
| 28 | `find . -name "0029-*"` | 0 (ADR bajo `docs/`, no `crates/`) |
| 29 | `grep -rn "should_summarize(" --include=*.rs . \\| grep -v "fn should_summarize\\|assert"` | 1 (sin call sites de producción) |
| 30 | `grep -rn "fn read_from_seq" --include=*.rs crates/chronos-log/src/` | 0 (5 implementaciones) |
| 31 | `grep -n "fn handle" -A 12 session_log.rs` | 0 (`Arc<dyn ExecutionLogProvider>`) |
| 32 | `sed -n '200,240p'` y `sed -n '330,365p' provider.rs` | 0 (delegaciones) |
| 33 | `sed -n '90,140p' execution_log_bootstrap.rs` + `grep -n "fn reopen_existing" -A 25 session_log.rs` | 0 |
| 34 | `grep -rn "impl ExecutionLogFactory" --include=*.rs crates/` | 0 (**una** impl) |
| 35 | `sed -n '905,990p' segmented.rs` | 0 (`read_from_seq` → `inner.backend`) |
| 36 | `grep -n "backend" segmented.rs` | 0 (`backend: InMemoryExecutionLog`, línea 109) |
| 37 | `awk` sobre `tools_params.rs:385-397` y `virtualization.rs` | 0 |
| 38 | `find . -name "scale_execution_log_1m.rs"` + lectura completa | 0 |
| 39 | `uname -r` / `rustc --version` / `nproc` / `grep MemTotal /proc/meminfo` | 0 |
| 40 | `grep -rn "7\.2\.4\\|7\.2\.7" --include=*.md docs/` | 0 (§1.2) |
| 41 | `find . -name "*H1.5*"` + `grep -n -i "budget"` | 0 |
| 42 | `sed -n '184,215p' docs/architecture/H1.5-…-benchmarks.md` | 0 (convención de budgets) |
| 43 | `grep -n "^\| \*\*" docs/roadmap/STATE.md \\| cut -c1-120` | 0 (filas: R1, R0; **no hay fila R2**) |

**No ejecutado, deliberadamente:** el test de escala de 1M
(`cargo test -p chronos-sandbox --test scale_execution_log_1m -- --ignored`). Cuesta 32,5 s de
siembra más >600 s de agregado. Este encargo es documental. **Por tanto las cifras de §3 son
citadas, no reproducidas por mí**, y así se declaran.

### A.2 Qué verifiqué personalmente leyendo el código, y qué no

**Verificado por mí, leyendo el código:**

- El clon completo antes del `limit` en `memory.rs:550` (`:558-564` clone, `:571-579` recorrido y
  `break`), incluido el matiz de que el recorrido arranca en el índice 0 y filtra (§4.2).
- La cadena de llamadas completa de `poll` hasta ese punto (§4.1).
- Que el backend de producción **envuelve un `InMemoryExecutionLog`** y por tanto el defecto está
  en el read path real (§4.4) — la afirmación más fácil de dar por cierta y la que más habría
  falseado este documento.
- `page_limit = 1024` y los bucles de `summarize_log`/`rollup_log`, y `SCAN_CHUNK = 512` (§4.5).
- El indexado absoluto de `bucket_counts` y la fórmula de D3 (§5 D3).
- La superficie de 4 métodos de `ReadPathService` y el enrutado por `mode` de los 4 modos MCP
  (§2), y que `export` no está ahí.
- Los cuatro anchors de producto: `ResourceLimits` `123-138`, `default_limit()` `385-387` (= 100),
  `default_bucket_size_ns()` `394-396` (= 10⁹), `DEFAULT_VIRTUALIZATION_THRESHOLD` `65`
  (= 100.000).
- Que `CHRONOS_EXEC_EXPLORER_VIRT_THRESHOLD` no se lee, y que `should_summarize` no tiene call
  site de producción (§9.1, §9.2).
- Que `ResourceLimits` sólo se instancia en dos `#[test]` (§9.3).
- El contenido del test in-tree de 1M: `#[ignore]`, timeout de 600 s, y que siembra timestamps
  monotónicos desde ~0 (§9.4).
- El fingerprint de H1.5 con kernel 7.2.4 y el actual con 7.2.7 (§1.2).

**NO verificado por mí:**

- Las cifras de §3 (proceden de un worker anterior; citadas, no reproducidas).
- La curva de crecimiento 1k→128k y sus exponentes 1,3→2,5 (§4.5).
- La proyección previa que subestimó 13 %-69 % (§1.3): la registré como advertencia metodológica,
  sin cifra que comprobar.
- El tamaño de sesión del `poll` de 333 ms de first response (§3.2a).

### A.3 Correcciones al encargo de partida

Tres, y las tres verificadas:

1. **Atribución del clon.** El encargo señalaba `memory.rs:550` sin advertir que el read path de
   producción pasa por él a través de un backend segmentado que *contiene* un
   `InMemoryExecutionLog` (`segmented.rs:109`). La atribución es correcta, pero sin esa cadena
   alguien podría "corregirla" creyendo que el defecto es de un backend de pruebas. Queda escrito
   en §4.4.
2. **Matiz del recorrido.** El recorrido no arranca "desde `from_seq`": arranca en el índice 0 y
   descarta con `if r.seq < from_seq { continue; }` (`memory.rs:574-576`). Refuerza la tesis
   O(N) en ambos casos de cursor.
3. **Affirmación adicional no presente en el encargo.** El switch por threshold **no está
   cableado en absoluto** (§9.2), lo cual es más fuerte que el env var muerto que el encargo ya
   anticipaba, y es la razón por la que la llamada de 1.222,7 s es alcanzable.

---

## Anexo B. Nota sobre `STATE.md`

**No he tocado `docs/roadmap/STATE.md`.** Razón: `STATE.md` no tiene fila de R2 (sus únicas filas
de estado son `**Estado de R1 (truth closure)**` en la línea 14 y `**Estado de R0**` en la 15, más
un bloque de entradas de journal). La fila de R0 es la fila de **certificación** —"6/6 gates en
verde sobre `5fce05fe`", la única certificación del proyecto— y R2.1 no es R0 ni debe escribir en
esa fila. Añadir el enlace de este documento requiere **crear una fila de R2**, que es una decisión
de estructura de `STATE.md` que corresponde al agente principal y no a este encargo. La sección
Anexo A.3 y §10 quedan como el contenido enlazable.
