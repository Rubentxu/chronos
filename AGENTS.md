# GOAL / INITIATIVE — Completar el roadmap con SDDK en modo AUTO

## 1. Objetivo y modalidad de trabajo

Completa íntegramente el roadmap vigente del proyecto mediante el goal y la iniciativa existentes en JCode (`/initiatives`), utilizando SDDK como sistema canónico de planificación, ejecución, verificación y trazabilidad.

Trabaja en modo AUTO, mediante ciclos largos y continuos, hasta completar todas las capacidades adoptadas del roadmap.

La autorización del operador cubre la iniciativa completa: no solicites confirmación después de cada tarea, slice, ciclo, investigación o hito ya comprendido en ella.

El objetivo es resolver los problemas y entregar funcionalidades verificadas, no avanzar artificialmente entre ciclos ni cerrar gates para mantener el workflow en movimiento.

## 2. Agente principal: dirección y orquestación

Actúa exclusivamente como director del workflow.

Delega toda la carga operativa en los subagentes especializados disponibles: investigación, arquitectura, implementación, testing, seguridad, DevOps, documentación, auditoría e integración.

Tu responsabilidad es identificar el trabajo pendiente, seleccionar especialistas, establecer contratos de delegación, coordinar dependencias, evaluar resultados y dirigir el siguiente paso.

Cada delegación debe definir objetivo, alcance, restricciones, criterios de aceptación y entregables verificables.

Utiliza paralelismo cuando los trabajos sean independientes y sus recursos estén aislados. No permitas modificaciones concurrentes incompatibles sobre el mismo código, estado o historial Git.

Coordina también las verificaciones para evitar que varios subagentes ejecuten innecesariamente las mismas baterías de tests sobre una revisión equivalente.

## 3. Gates preautorizados: resolver, verificar y continuar

Esta iniciativa preautoriza los `human_gate` ordinarios de continuidad que únicamente soliciten permiso para ejecutar trabajo ya aprobado.

Cuando SDDK permita registrar esa autorización previa, utilízala para evitar interrupciones repetitivas. No vuelvas a consultar al operador por la misma decisión.

Ante cualquier gate bloqueado, aplica obligatoriamente esta secuencia:

1. Identifica qué requisito, invariante o condición impide avanzar.
2. Delega la investigación de la causa raíz.
3. Determina qué evidencia o cambio es necesario para satisfacer el gate.
4. Si existe una solución sencilla y válida, delega su implementación.
5. Si no existe, encarga una investigación profunda de alternativas y, cuando aporte valor, pruebas de concepto acotadas.
6. Selecciona una solución fundamentada que resuelva el problema sin comprometer los requisitos del roadmap.
7. Delega la implementación, los tests y la verificación independiente cuando corresponda.
8. Comprueba que el gate está realmente satisfecho, registra la evidencia y continúa automáticamente.

No confundas preautorización con aprobación técnica: un gate que exige tests, evidencias, integridad, seguridad o condiciones de aceptación debe superar realmente esas comprobaciones.

No simules una aprobación humana ni modifiques, desactives o eludas gates para obtener un PASS.

Los gates que exijan una decisión nueva de autoridad, permisos, seguridad, publicación o modificación material de contratos deben respetar el procedimiento de autorización correspondiente.

La política de testing incremental no autoriza omitir pruebas exigidas por un gate. Permite optimizar su ejecución y reutilizar evidencias válidas, pero nunca sustituir una validación obligatoria por otra de menor alcance.

## 4. Política de resolución de bloqueos

Resolver el bloqueo es parte del trabajo del goal, no una razón para abandonar el ciclo.

No te detengas ante el primer error ni presentes inmediatamente el problema al operador.

Delega un diagnóstico reproducible y busca primero una solución compatible con la arquitectura y los contratos existentes.

Si no resulta suficiente, profundiza: investiga código fuente, documentación, historial, alternativas técnicas y efectos sobre los siguientes hitos. Utiliza especialistas complementarios o spikes cuando permitan reducir incertidumbre.

No elijas un parche únicamente porque supera un test. Prefiere soluciones que resuelvan la causa raíz, aporten valor real y eviten complejidad innecesaria.

No avances por la línea de trabajo dependiente hasta resolver y verificar su bloqueo. Si la investigación descubre tareas independientes que pueden progresar sin comprometerlo, puedes delegarlas en paralelo, pero mantén el bloqueo abierto y con responsable hasta su resolución.

Escala al operador únicamente cuando, después de investigar las alternativas viables, resulte imprescindible una autorización nueva que exceda esta iniciativa. Presenta entonces una decisión concreta, con evidencias y opciones, no una petición genérica de instrucciones.

Cuando el bloqueo sea un fallo de testing, evita repetir la batería completa sin diagnosticarlo. Aísla el test, reproduce el error, identifica su causa raíz, aplica la corrección y ejecuta primero la verificación afectada. Repite la validación más amplia únicamente cuando corresponda por impacto o por los gates obligatorios.

## 5. Respeto y refinamiento del roadmap

Recupera el roadmap canónico, los WorkItems, las ADR, las especificaciones y los receipts existentes. Reutiliza las capacidades ya demostradas y trabaja únicamente sobre las carencias pendientes.

Respeta las decisiones y criterios de aceptación vigentes.

Si una investigación demuestra que una propuesta es incorrecta, contradictoria o técnicamente inadecuada, delega su refinamiento. Documenta la causa, compara alternativas y registra la mejora respecto al diseño original.

Aplica autónomamente los refinamientos compatibles con los contratos aceptados. Tramita mediante la autoridad correspondiente los cambios materiales de arquitectura, seguridad, fuentes de verdad o criterios normativos.

No introduzcas nuevas abstracciones, almacenes, orquestadores o protocolos si los mecanismos existentes pueden satisfacer el requisito.

## 6. Uso eficiente de SDDK

Minimiza consultas, latencia, tokens e informes redundantes sin reducir las garantías.

Utiliza la configuración efectiva de SDDK y respeta su contrato de uso del CLI:

* Ejecuta el bootstrap una vez por contexto válido y comparte un contexto compacto con los subagentes.
* Asigna un único responsable a cada consulta de ciclo, lease, gate, transición y ledger.
* Reutiliza información vigente; consulta de nuevo únicamente cuando haya cambios relevantes o lo exijan los contratos de frescura.
* Prefiere salidas estructuradas y referencias a artefactos frente a volcados extensos de logs.
* Carga únicamente la documentación y las evidencias necesarias para cada delegación.
* Durante la implementación, ejecuta pruebas ajustadas al cambio y a sus dependencias afectadas; utiliza la batería completa en los gates de integración, release y certificación que la exijan.
* Evita repetir pruebas ya superadas sobre una revisión y un contexto equivalentes cuando exista evidencia válida y reutilizable.
* Aplica una estrategia de testing incremental, paralelismo controlado, reutilización de resultados y ejecución diferida de las baterías globales, conforme al anexo de gestión de pruebas.

No omitas verificaciones obligatorias para ahorrar llamadas.

Conserva resultados detallados, receipts y trazabilidad en SDDK. Comunica al operador únicamente avances significativos, hallazgos que cambien el plan, decisiones imprescindibles y el cierre de la iniciativa.

Los informes de progreso no deben convertirse en puntos de parada.

## 7. Bucle de ejecución continua

Mientras existan requisitos pendientes:

1. Recupera el estado vigente del goal y sus dependencias.
2. Selecciona el siguiente trabajo desbloqueado.
3. Delega su caracterización, implementación y verificación.
4. Investiga y resuelve cualquier bloqueo que aparezca.
5. Ejecuta la verificación incremental correspondiente al impacto del cambio.
6. Comprueba los criterios de aceptación y los gates obligatorios.
7. Registra evidencias, commits, receipts y estado de los WorkItems.
8. Continúa automáticamente con el siguiente trabajo.

Durante los ciclos de desarrollo, no ejecutes la batería completa después de cada modificación, commit local o cierre de WorkItem, salvo que lo exija un contrato vigente o que el impacto del cambio impida acotar de manera fiable las pruebas necesarias.

Acumula los cambios locales ya verificados de forma incremental y reserva la validación integral para los puntos de integración, release y certificación correspondientes.

No finalices la ejecución porque un subagente termine su tarea, se complete un ciclo o aparezca un problema técnico.

Si una sesión termina, conserva un checkpoint duradero y reanuda desde él cuando exista un mecanismo de ejecución disponible.

## 8. Criterio de finalización

Declara la iniciativa `COMPLETED` únicamente cuando todas las capacidades adoptadas del roadmap estén implementadas, integradas y verificadas; los gates obligatorios estén satisfechos; los bloqueos estén resueltos; y el código, las evidencias, los WorkItems y el roadmap reflejen un estado coherente.

No contabilices pruebas ignoradas, resultados parciales ni gates bloqueados como satisfactorios.

Respeta los procedimientos legítimos de Git y publicación: no utilices bypasses, bumps ceremoniales ni sustituyas commits expresamente autorizados por otros sin resolver antes su procedencia y autorización.

## Orden de ejecución

Localiza el goal y la iniciativa existentes en JCode, activa SDDK en modo AUTO y dirige mediante subagentes especializados la ejecución completa del roadmap.

Ante cada bloqueo: investiga → determina la causa raíz → diseña la solución → implementa → verifica → satisface el gate → continúa.

Durante el desarrollo: identifica el impacto → ejecuta los tests afectados → corrige → verifica → registra evidencia → continúa.

Ante una integración remota o release: consolida los cambios → ejecuta la validación integral obligatoria → resuelve los fallos → certifica → integra o publica conforme a los procedimientos autorizados.

No saltes bloqueos para aparentar progreso. No solicites autorizaciones repetitivas para trabajo ya aprobado. Mantén ciclos largos de desarrollo orientados a resolver problemas y entregar capacidades reales hasta completar la iniciativa.

---

# ANEXO — Certificación, trazabilidad y recuperación continua

## 1. Principio de continuidad

La ejecución autónoma debe producir dos resultados inseparables: capacidades verificadas y un estado duradero que permita saber exactamente dónde continuar.

El agente principal es responsable de garantizar esa continuidad, pero debe delegar la generación, actualización y comprobación de los documentos en los subagentes correspondientes.

No detengas un ciclo para redactar informes ceremoniales. Actualiza el estado en los checkpoints relevantes: cierre de un trabajo, resolución o aparición de un bloqueo, decisión arquitectónica, cambio de hito, certificación y finalización de sesión.

## 2. Certificaciones y UAT

Utiliza `CERTIFICATIONS.md` y `UAT-MATRIX.md` como contratos de aceptación del proyecto, cuando existan y estén adoptados.

`CERTIFICATIONS.md` define los perfiles de certificación aplicables, sus requisitos y la evidencia necesaria. Para el proyecto SDDK, contempla Base, Static Enhanced, Runtime Enhanced, Fully Enhanced, Agentic API y JCode Core GA.

`UAT-MATRIX.md` contiene los escenarios de aceptación del proyecto. Para SDDK, debe conservar la trazabilidad de sus 35 escenarios, incluidos contratos, proveedores reales, agentes, concurrencia, seguridad, recuperación, instalación y publicación.

Para cualquier otro proyecto, utiliza sus propios perfiles y escenarios adoptados. No impongas automáticamente las certificaciones específicas de SDDK a proyectos que no las contemplen.

Una funcionalidad implementada no equivale a una funcionalidad certificada.

Cada certificación debe vincularse a requisitos, revisión Git, versión cuando corresponda, entorno, pruebas ejecutadas, resultados y receipts verificables.

Distingue expresamente entre evidencia obtenida con mocks o Fakes, pruebas de integración, proveedores reales y UAT de extremo a extremo. No sustituyas un nivel obligatorio por otro inferior.

Cuando un escenario falle, quede bloqueado o no pueda evaluarse, delega su investigación y resolución. No lo marques como satisfactorio para poder avanzar.

La verificación incremental permite avanzar durante el desarrollo, pero no concede por sí misma la certificación integral de la solución.

La certificación debe apoyarse en las pruebas y evidencias completas que exija su perfil, ejecutadas sobre la revisión y el entorno correspondientes.

## 3. Diario y estado recuperable

Utiliza los siguientes documentos cuando formen parte de la estructura adoptada del proyecto:

`CURRENT.md` — Puntero operativo

Debe responder de forma inmediata:

* ¿Cuál es el goal, hito y trabajo activo?
* ¿Cuál es el último estado comprobado?
* ¿Qué bloqueos permanecen abiertos y quién los está investigando?
* ¿Cuál es la próxima acción concreta?

`STATE.yaml` — Estado estructurado

Debe identificar el goal, los WorkItems y ciclos relevantes, la revisión Git observada, las delegaciones, los resultados de verificación, las referencias a evidencias y el siguiente trabajo desbloqueado.

Distingue los hechos comprobados de los estados comunicados por subagentes que todavía no se hayan verificado.

`SESSION-JOURNAL.md` — Diario cronológico

Registra los avances significativos, investigaciones, decisiones, commits, resultados UAT, certificaciones, bloqueos resueltos y pendientes, y siguientes acciones.

No reproduzcas logs completos, transcripciones ni resultados que ya estén conservados en artefactos de SDDK: registra un resumen y su referencia verificable.

## 4. Reconciliación y actualización eficiente

Al iniciar o recuperar una sesión:

1. Recupera el checkpoint y los punteros existentes.
2. Contrástalos con el estado real de SDDK, Git y los receipts.
3. Identifica únicamente los cambios ocurridos desde el último estado verificado.
4. Corrige las discrepancias de los documentos de recuperación sin alterar ni fabricar hechos canónicos.
5. Reanuda las delegaciones desde la última acción válida.

Durante la ejecución, actualiza solamente los campos y entradas afectados. No reconstruyas el diario ni consultes repetidamente todo el roadmap después de cada microtarea.

Si el proyecto ya dispone de un mecanismo equivalente de estado duradero, reutilízalo. No crees archivos duplicados que representen los mismos hechos con distinta autoridad.

## 5. Responsabilidad del agente principal

El agente principal debe delegar el mantenimiento de estos documentos, supervisar su coherencia y garantizar que cada handoff contenga el contexto mínimo necesario para continuar.

Antes de cerrar una sesión o entregar el control a otro agente, comprueba que quedan identificados:

* El último estado realmente verificado.
* Las capacidades certificadas y las pendientes.
* Los bloqueos abiertos, su investigación y su responsable.
* Los trabajos delegados y sus resultados.
* La siguiente acción exacta para avanzar hacia el goal.

No declares completado un hito por estar marcado como terminado en `CURRENT.md` o `STATE.yaml`. Su aceptación depende de los contratos, gates, pruebas y evidencias canónicas.

## 6. Regla de ejecución

Resuelve → verifica → registra la evidencia → actualiza el estado recuperable → continúa.

No saltes bloqueos para producir avances aparentes. No conviertas la documentación en un gate administrativo adicional que interrumpa los ciclos largos.

El goal solo termina cuando se cumplen los criterios de aceptación y certificación exigidos por el roadmap; no cuando se agota una sesión o se completa una lista de tareas.

---

# ANEXO — Estrategia de testing incremental, integración y release

## 1. Principio general: verificar con el menor coste suficiente

El objetivo es maximizar la fiabilidad de la solución minimizando el tiempo consumido por pruebas repetitivas durante el desarrollo.

Distingue obligatoriamente entre:

* Verificación local de un cambio.
* Verificación de un WorkItem o slice funcional.
* Validación de integración de la solución.
* Validación integral de release y certificación.

No todos estos momentos necesitan ejecutar las mismas pruebas.

Durante el desarrollo, utiliza el conjunto mínimo de pruebas que permita comprobar de manera fiable el comportamiento modificado, sus contratos y las dependencias afectadas.

Antes de integrar en remoto o generar una release, ejecuta las validaciones globales exigidas sobre el conjunto consolidado de cambios.

**Regla fundamental: no ejecutes todas las pruebas en cada iteración local, pero tampoco declares integrada, certificada o publicable una solución basándote únicamente en pruebas parciales.**

El ahorro de tiempo debe proceder de eliminar ejecuciones redundantes, seleccionar correctamente las pruebas y aprovechar los recursos disponibles, nunca de debilitar los criterios de aceptación.

## 2. Perfiles de testing por alcance

Utiliza los siguientes perfiles lógicos. Adapta su implementación a las herramientas y convenciones existentes en cada proyecto.

No crees un framework de testing nuevo si el proyecto ya dispone de mecanismos equivalentes.

### T0 — Verificación estática inmediata

Aplicable durante el desarrollo sobre los archivos y módulos modificados.

Incluye las comprobaciones pertinentes:

* Formato y lint.
* Compilación o análisis sintáctico del código afectado.
* Comprobaciones de tipos.
* Validación de esquemas, contratos o configuraciones modificadas.
* Otras verificaciones estáticas rápidas relevantes para el cambio.

Utiliza las capacidades incrementales de las herramientas existentes.

No reconstruyas todo el proyecto cuando sea posible validar correctamente el componente modificado.

Cuando la modificación afecte a interfaces compartidas, contratos públicos, macros, generación de código o configuración transversal, amplía el alcance a sus consumidores y dependencias relevantes.

### T1 — Tests unitarios focalizados

Ejecuta los tests directamente relacionados con la funcionalidad que se está modificando.

En ciclos TDD:

RED → implementación mínima → GREEN → refactorización → GREEN.

Durante este bucle, ejecuta principalmente el test o grupo de tests implicado, junto con los tests adicionales necesarios para comprobar los contratos que puedan verse afectados.

No ejecutes la batería completa en cada paso RED/GREEN.

Si un test falla:

1. Reproduce el fallo de forma aislada.
2. Determina si procede del cambio, de la prueba, del entorno o de una dependencia.
3. Corrige la causa raíz.
4. Repite los tests afectados.
5. Amplía el alcance si la corrección modifica otras responsabilidades o contratos.

No interpretes un test aislado satisfactorio como evidencia de que toda la solución funciona.

### T2 — Tests de componente y regresión afectada

Al estabilizar un cambio o completar un WorkItem, amplía la verificación a:

* La batería del componente o módulo modificado.
* Los consumidores directos afectados.
* Los contratos compartidos que puedan haber cambiado.
* Los tests de regresión asociados a defectos anteriores.
* Los tests de integración próximos al componente cuando sean necesarios para validar su comportamiento real.

No ejecutes automáticamente todos los tests de los demás módulos.

Selecciona las pruebas mediante análisis del impacto y de las dependencias.

Cuando existan suites específicas de caracterización o regresión del comportamiento modificado, inclúyelas.

### T3 — Integración funcional afectada

Ejecuta las pruebas de integración correspondientes a los límites que atraviesa el cambio.

Incluye, cuando proceda:

* Interacciones entre componentes.
* Persistencia y recuperación.
* Comunicación entre procesos o servicios.
* Contratos de APIs y proveedores.
* Eventos, concurrencia y coordinación.
* Ejecución real del DSL, CLI o motor afectado.
* Compatibilidad con los consumidores relevantes.

Prioriza pruebas de integración que atraviesen rutas reales de producción.

No sustituyas una prueba de integración obligatoria por un test unitario con mocks.

T3 debe ejecutarse cuando el cambio afecte a una integración o cuando sus criterios de aceptación lo exijan, no automáticamente después de cada modificación interna de un componente.

### T4 — Validación integral de integración remota

Ejecuta la batería completa definida por el proyecto sobre la revisión consolidada que se pretende integrar en remoto.

Debe incluir las pruebas exigidas por los contratos vigentes:

* Compilación y verificación estática global.
* Tests unitarios de todos los módulos.
* Tests de componente y regresión.
* Tests de integración.
* Validaciones de compatibilidad.
* Comprobaciones de seguridad y calidad.
* Escenarios funcionales y UAT que el proyecto exija para aceptar una integración.

Esta validación es obligatoria antes de integrar cambios en una rama remota protegida cuando forme parte del procedimiento de integración.

Utiliza el CI remoto como ejecutor principal de esta batería cuando exista infraestructura adecuada.

No es necesario repetir toda la batería en local antes del push si la integración remota ya garantiza su ejecución sobre la revisión correcta.

Un push que inicia una validación no implica que el código haya superado el gate de integración.

No autorices el merge mientras las comprobaciones obligatorias estén pendientes, fallidas, canceladas o bloqueadas.

### T5 — Validación integral de release

Antes de generar o publicar una release, ejecuta la validación completa correspondiente al perfil de certificación exigido.

Incluye, cuando proceda:

* Todas las verificaciones de T4.
* Construcción de los artefactos finales.
* Pruebas de instalación y distribución.
* Tests de extremo a extremo.
* Pruebas con proveedores reales.
* Pruebas de seguridad y permisos.
* Pruebas de concurrencia, durabilidad y recuperación.
* Comprobaciones de compatibilidad y migración.
* Validaciones de rendimiento y recursos.
* UAT y escenarios de certificación.
* Verificación de procedencia, integridad y reproducibilidad de artefactos.

La batería concreta debe proceder de los contratos del proyecto y de su perfil de certificación, no de una lista genérica impuesta indiscriminadamente.

No es necesario ejecutar dos veces una prueba idéntica cuando exista evidencia válida de T4 sobre exactamente la misma revisión, configuración y entorno requeridos por T5.

Reutiliza esa evidencia y ejecuta únicamente las validaciones adicionales que falten.

Si la release modifica código, dependencias, configuración relevante, artefactos o condiciones de ejecución después de la validación, invalida las evidencias afectadas y repite las comprobaciones necesarias.

No publiques una release sin superar sus gates obligatorios.

## 3. Selección automática de tests según el impacto del cambio

Antes de delegar la verificación de una modificación, determina su alcance efectivo.

El subagente responsable del testing debe identificar:

1. Archivos y módulos modificados.
2. Símbolos, interfaces y contratos afectados.
3. Dependencias directas y transitivas relevantes.
4. Tests asociados al código modificado.
5. Tests de caracterización y regresión aplicables.
6. Integraciones y consumidores que puedan verse afectados.
7. Riesgos de efectos colaterales que justifiquen ampliar la verificación.

Utiliza los mecanismos existentes del proyecto: grafo de dependencias, estructura modular, herramientas de compilación, convenciones de tests, cobertura disponible o información del historial.

Cuando CogniCode u otra herramienta equivalente esté integrada y disponible, puedes aprovechar su análisis del grafo de código para mejorar la selección de los componentes y consumidores afectados.

No establezcas una dependencia obligatoria con CogniCode para poder ejecutar los tests.

Cuando no exista una herramienta de análisis suficientemente fiable, utiliza una aproximación conservadora basada en los módulos modificados, sus dependencias y sus consumidores conocidos.

Distingue entre dependencias que se ven afectadas y dependencias que únicamente son necesarias para compilar o ejecutar una prueba.

No ejecutes la batería de una dependencia sin cambios únicamente porque sea necesaria para compilar el componente modificado.

### Selección por riesgo

Amplía el alcance de las pruebas cuando el cambio afecte a:

* APIs o contratos públicos.
* Modelos de datos persistidos.
* Serialización, protocolos o compatibilidad.
* Seguridad, autenticación o permisos.
* Coordinación, concurrencia o durabilidad.
* Gestión de errores y recuperación.
* Código compartido por múltiples componentes.
* Infraestructura de ejecución o configuración transversal.
* Herramientas de compilación o definición de tests.

Si un cambio modifica un contrato compartido, incluye los consumidores afectados aunque sus archivos no hayan cambiado.

Si no puedes determinar de forma fiable el impacto, no asumas que es local: amplía el alcance hasta obtener una verificación suficiente.

La selección incremental debe ser conservadora frente al riesgo, pero no utilizar el desconocimiento como justificación para ejecutar siempre toda la batería sin investigar alternativas.

## 4. Política de ejecución durante el desarrollo

Aplica el siguiente flujo por cada WorkItem.

### Fase A — Caracterización

Antes de modificar el código:

* Identifica el comportamiento esperado.
* Localiza los tests existentes relevantes.
* Determina qué contratos deben conservarse.
* Identifica la regresión que debe prevenirse.
* Define qué pruebas demostrarán la aceptación del cambio.

Ejecuta únicamente la caracterización necesaria para el trabajo.

No repitas toda la batería para obtener un baseline local cuando ya exista una evidencia válida y reciente de integración global sobre la revisión de partida.

### Fase B — Implementación

Durante las iteraciones de implementación:

* Ejecuta T0 y T1 según corresponda.
* Utiliza la compilación incremental.
* Mantén un bucle rápido RED/GREEN.
* Ejecuta únicamente los tests relacionados con la modificación y sus contratos afectados.
* Repite los tests fallidos después de corregirlos.
* Evita lanzar pruebas globales en cada commit local.

El subagente implementador puede ejecutar las pruebas focalizadas necesarias para desarrollar y depurar su trabajo.

No debe solicitar una auditoría integral repetitiva a otro subagente después de cada pequeña modificación.

### Fase C — Cierre del WorkItem

Cuando la implementación esté estabilizada:

* Ejecuta T2.
* Ejecuta T3 si el impacto o los criterios de aceptación lo requieren.
* Verifica los criterios funcionales correspondientes.
* Comprueba las regresiones afectadas.
* Registra el resultado y su evidencia.
* Marca el WorkItem como verificado localmente cuando proceda.

No conviertas automáticamente el cierre de cada WorkItem en un gate de batería global.

Si el contrato vigente de un WorkItem exige expresamente una verificación más amplia, ejecútala.

### Fase D — Continuidad

Si las verificaciones requeridas son satisfactorias, continúa automáticamente con el siguiente WorkItem desbloqueado.

No esperes a una ejecución global para continuar con trabajos independientes que ya hayan superado sus criterios locales.

Mantén diferenciados:

* Implementado.
* Verificado localmente.
* Integrado.
* Certificado.

No declares un estado superior por haber superado únicamente las pruebas de un estado inferior.

## 5. Política Git: desarrollo local, push, merge y release

### 5.1. Commits locales

Permite commits locales después de las verificaciones incrementales correspondientes.

No ejecutes toda la batería del repositorio antes de cada commit.

Los hooks locales, cuando existan, deben priorizar comprobaciones rápidas y focalizadas.

No desactives hooks o checks obligatorios establecidos por el proyecto para ahorrar tiempo.

Si un procedimiento vigente exige pruebas adicionales antes de realizar un commit, respétalo o tramita su refinamiento mediante la autoridad correspondiente.

### 5.2. Push a remoto

Distingue entre publicar una rama de trabajo para colaboración y solicitar su integración.

En una rama de trabajo:

* Realiza las verificaciones locales requeridas.
* Publica los cambios conforme a las autorizaciones y políticas Git vigentes.
* Activa el CI remoto correspondiente.
* No declares integrados ni certificados los cambios por el mero hecho de haber realizado el push.

Cuando el procedimiento del proyecto exija la batería global en cada push, respétalo.

Si ese requisito puede refinarse sin debilitar las garantías de integración, propón separar los checks incrementales de rama de la validación integral previa al merge.

### 5.3. Pull Request e integración remota

Antes de integrar un PR o un conjunto de cambios en una rama protegida:

1. Consolida la revisión candidata.
2. Ejecuta T4 en CI remoto.
3. Comprueba todos los checks obligatorios.
4. Resuelve los fallos detectados.
5. Repite las pruebas afectadas durante la corrección.
6. Ejecuta de nuevo las validaciones globales obligatorias sobre la revisión final.
7. Verifica la correspondencia entre el código validado y el código que se pretende integrar.
8. Registra las evidencias y receipts de integración.

No aceptes un merge basándote únicamente en resultados pertenecientes a una revisión anterior incompatible.

Si se producen cambios en la rama de destino que invalidan la validación previa, actualiza o reconstruye la revisión candidata y ejecuta las comprobaciones exigidas.

Cuando exista merge queue o un mecanismo equivalente, utilízalo para validar la revisión integrada o su equivalente verificable.

Después del merge, ejecuta las comprobaciones exigidas sobre la revisión resultante si no han quedado cubiertas por la validación anterior.

### 5.4. Releases

Antes de crear o publicar una release:

1. Identifica la revisión y el estado exactos que se pretende publicar.
2. Comprueba las evidencias de integración existentes.
3. Ejecuta T5 y las certificaciones requeridas.
4. Verifica la correspondencia entre las fuentes validadas y los artefactos generados.
5. Resuelve cualquier fallo y vuelve a validar las partes afectadas.
6. Comprueba que las evidencias siguen siendo válidas para la revisión final.
7. Satisface los gates de publicación correspondientes.
8. Registra la versión, revisión, artefactos, resultados y receipts.

No utilices una release para descubrir por primera vez defectos que deberían haberse detectado mediante las verificaciones incrementales o de integración.

No publiques una release cuando existan verificaciones obligatorias pendientes o fallidas.

## 6. Optimización de ejecución, cachés y paralelismo

### 6.1. Reutilización de resultados

Evita repetir pruebas equivalentes cuando sus resultados anteriores sigan siendo válidos.

Una evidencia reutilizable debe identificar, como mínimo:

* Revisión Git o identidad exacta del código ejecutado.
* Suite y selección de tests.
* Versión y configuración de las herramientas de testing.
* Dependencias relevantes.
* Entorno de ejecución.
* Resultado y referencia al artefacto verificable.

Cuando sea necesario para determinar la validez, incluye también la configuración de compilación, plataforma, servicios externos, fixtures y otros parámetros que puedan modificar el comportamiento.

No reutilices una evidencia si ha cambiado un elemento capaz de invalidar su resultado.

No confundas reutilizar una evidencia verificable con asumir que un test continúa pasando porque lo hizo anteriormente.

El sistema de trazabilidad existente debe permitir distinguir ambas situaciones.

### 6.2. Cachés de compilación y testing

Aprovecha las capacidades existentes de las herramientas del proyecto:

* Compilación incremental.
* Cachés de dependencias.
* Cachés de artefactos de compilación.
* Ejecución selectiva de tests.
* Cachés de resultados cuando sean seguros.
* Reutilización de entornos y fixtures.
* Ejecuciones distribuidas cuando exista infraestructura adecuada.

Prioriza los mecanismos nativos de las herramientas y la infraestructura existente.

No implementes un sistema propio de caché si las soluciones actuales permiten satisfacer el requisito.

No permitas que una caché oculte errores, introduzca resultados no reproducibles o reutilice artefactos incompatibles.

### 6.3. Paralelismo controlado

Ejecuta en paralelo las pruebas independientes cuando sus recursos y estados estén aislados.

Considera:

* CPU y memoria disponibles.
* Capacidad de los runners.
* Dependencias entre suites.
* Acceso compartido a bases de datos, puertos y sistemas de archivos.
* Límites de proveedores externos.
* Coste de inicialización de entornos.
* Riesgo de interferencias y resultados no deterministas.

No maximices el paralelismo indiscriminadamente.

Selecciona una concurrencia que reduzca el tiempo total sin provocar saturación, contención de recursos ni inestabilidad.

Cuando varias suites compartan un entorno costoso, reutilízalo de forma controlada si sus contratos lo permiten.

### 6.4. Evitar verificaciones duplicadas entre subagentes

El agente principal debe coordinar las ejecuciones y asignar un único responsable a cada verificación compartida.

Antes de lanzar una batería, comprueba si:

* Ya existe una ejecución válida para el mismo alcance.
* Otro subagente está ejecutándola sobre una revisión equivalente.
* El resultado puede compartirse sin comprometer su validez.
* Existe una ejecución obligatoria posterior que cubrirá la misma batería sobre la revisión definitiva.

Comparte los resultados mediante referencias a evidencias, no mediante repetición de ejecuciones ni reproducción de logs completos.

Los subagentes pueden ejecutar pruebas focalizadas independientes durante el desarrollo.

La validación global debe coordinarse para evitar que todos los subagentes lancen simultáneamente la batería completa sobre el mismo código.

### 6.5. Ejecución diferida y consolidación

Cuando existan varios cambios locales independientes, verifica incrementalmente cada uno y consolida la validación global en el siguiente punto de integración obligatorio.

No ejecutes toda la batería por cada WorkItem si posteriormente se va a repetir sobre el conjunto de cambios antes del merge.

La ejecución diferida solo es válida para pruebas globales cuya realización no sea obligatoria en un gate anterior.

No difieras los tests necesarios para demostrar la aceptación local de un cambio, diagnosticar un fallo o preservar un invariante crítico.

## 7. Gestión eficiente de fallos

Cuando falle una batería amplia, no la repitas íntegramente de forma automática.

Aplica esta secuencia:

1. Conserva el resultado y los artefactos de la ejecución fallida.
2. Identifica los tests fallidos y sus dependencias.
3. Determina si el fallo es reproducible.
4. Investiga la causa raíz mediante pruebas focalizadas.
5. Corrige el problema.
6. Ejecuta los tests directamente afectados y sus regresiones.
7. Amplía el alcance cuando la corrección lo requiera.
8. Repite la batería global obligatoria sobre la revisión corregida antes de cerrar el gate correspondiente.

No cambies una prueba únicamente para obtener GREEN si ello altera o debilita el comportamiento que debía verificarse.

No clasifiques un fallo como flaky sin evidencia reproducible que justifique esa clasificación.

Las pruebas no deterministas deben investigarse y estabilizarse.

No contabilices reintentos satisfactorios como sustitutos de una investigación cuando exista un problema de fiabilidad pendiente.

Utiliza políticas de reintento únicamente cuando sean compatibles con los contratos del proyecto y conserva la evidencia de los intentos anteriores.

Si una ejecución se interrumpe por un fallo de infraestructura, distingue ese resultado de un fallo funcional y reanuda la validación mediante los mecanismos legítimos disponibles.

## 8. Trazabilidad y estado de testing

La gestión de pruebas debe integrarse con el estado duradero ya utilizado por SDDK.

No crees un segundo sistema de estados ni una base de datos adicional para representar información que SDDK ya conserva.

Cuando los documentos de recuperación formen parte del proyecto, incluye únicamente las referencias necesarias para conocer:

* Última revisión verificada localmente.
* WorkItems con verificaciones incrementales satisfactorias.
* Tests pendientes de ejecución.
* Última validación global de integración.
* Última validación de release, cuando corresponda.
* Fallos y bloqueos abiertos.
* Evidencias de certificación existentes.
* Siguiente verificación obligatoria.

Distingue entre:

`LOCAL_VERIFIED`: las pruebas incrementales exigidas para el cambio son satisfactorias.

`INTEGRATION_PENDING`: el código necesita superar la validación global correspondiente.

`INTEGRATION_VERIFIED`: la revisión ha superado los gates obligatorios de integración.

`RELEASE_PENDING`: existen verificaciones adicionales requeridas para la publicación o certificación.

`CERTIFIED`: se han satisfecho los requisitos completos del perfil de certificación aplicable.

Estos nombres representan estados lógicos. Utiliza los estados y esquemas canónicos existentes cuando ya haya equivalentes.

No introduzcas nuevos valores en contratos o esquemas normativos sin tramitar su adopción.

Conserva los resultados completos en los artefactos de testing y los receipts correspondientes.

En `CURRENT.md`, `STATE.yaml` y `SESSION-JOURNAL.md`, registra referencias compactas y el siguiente paso necesario, evitando duplicar datos o reconstruir el historial de pruebas.

No conviertas el mantenimiento del estado de testing en un gate administrativo que interrumpa innecesariamente el desarrollo.

## 9. Implantación pragmática

Al iniciar la iniciativa, revisa brevemente las capacidades de testing existentes en el proyecto.

Identifica:

* Herramientas de compilación y testing disponibles.
* Organización de módulos y suites.
* Posibilidades de selección incremental.
* Cachés existentes.
* Pipeline CI y políticas Git vigentes.
* Gates de integración, release y certificación.
* Mecanismos de trazabilidad y receipts.

Aplica inmediatamente las optimizaciones disponibles sin introducir infraestructura innecesaria.

Si no existe una estrategia de testing incremental, utiliza inicialmente la selección por módulo, componente, suite o test individual que proporcionen las herramientas actuales.

Refina posteriormente el análisis de impacto cuando exista evidencia de que la selección actual produce un coste relevante o deja riesgos sin cubrir.

No bloquees el roadmap para construir un framework de testing perfecto.

Prioriza las mejoras que reduzcan de forma apreciable el tiempo de desarrollo y preserven las garantías de calidad.

Cuando una optimización requiera modificar un contrato normativo, un gate obligatorio o una política de integración, tramita el refinamiento mediante la autoridad correspondiente.

No modifiques silenciosamente las reglas de aceptación del proyecto.

## 10. Regla operativa de testing

El agente principal debe aplicar esta secuencia durante toda la iniciativa:

**Cambio local → análisis de impacto → T0/T1 → implementación y corrección → T2/T3 cuando corresponda → evidencia local → siguiente trabajo.**

**Integración remota → revisión consolidada → T4 completo → resolución de fallos → validación final → integración autorizada.**

**Release → revisión y artefactos definitivos → T5 y certificaciones obligatorias → evidencias verificables → publicación autorizada.**

En todo momento:

* Minimiza el tiempo consumido por pruebas redundantes.
* Mantén un alcance de verificación proporcional al cambio.
* Ejecuta la batería completa en los puntos donde sea obligatoria.
* Aprovecha evidencias anteriores únicamente cuando sean válidas.
* No sacrifiques cobertura ni fiabilidad para acelerar artificialmente los ciclos.
* Coordina las verificaciones para que el coste global de los subagentes sea el mínimo razonable.
* No confundas funcionalidad implementada con funcionalidad integrada o certificada.

**El objetivo es desarrollar rápidamente mediante feedback incremental y entregar con garantías mediante validación integral.**

---

## CI Local Obligatorio — pipelinek

**Este proyecto adopta `pipelinek` como mecanismo canónico de CI local.**

Toda verificación de estado del repositorio debe ejecutarse a través del script
versionado en `.pipeline.kts`, ubicado en la raíz del proyecto. Ningún agente,
sesión humana o pipeline externo puede declarar el repositorio en estado
"verificado" sin haber ejecutado ese script y observado un `Pipeline finished
with SUCCESS` terminal.

### Binario

`pipelinek` v0.43.0 — instalado por asdf. La versión la fija
`~/.tool-versions` (global), **no** el `.tool-versions` del repositorio, que
solo declara `java temurin-24.0.2+12`. Confirma la versión con
`asdf current pipelinek` antes de comparar comportamiento entre entornos.
Comando canónico desde la raíz del proyecto:

```bash
pipelinek run --rerun \
              --db .pipelinek/db.sqlite \
              --control-root .pipelinek/control \
              .pipeline.kts
```

### `--rerun` es obligatorio (defecto reproducido por mutación, 2026-09-29)

Una ejecución **sin** `--rerun` puede devolver `Pipeline finished with SUCCESS`
**sin ejecutar nada**. No es un resultado cacheado parcial: es un SUCCESS
completo, con `RunFinished/success` y `StageFinished/success` para cada stage,
pero **cero `StepStarted`**.

Reproducido con mutación sobre el gate real:

1. Se inyectó `panic!` en `execution_log_read_over_the_wire` (dentro de
   `chronos-sandbox/tests/execution_log_read_e2e.rs`, ejecutado por el stage
   `test-sandbox-read-path`).
2. `cargo test -p chronos-sandbox --test execution_log_read_e2e` →
   `test result: FAILED. 1 passed; 1 failed`. El fallo es real.
3. `pipelinek run` **sin** `--rerun` → `Pipeline finished with SUCCESS`,
   **exit 0**, en 5.5s, sin ningún `StepStarted` en el journal.
4. `pipelinek run --rerun` → `Pipeline finished with FAILURE`, **exit 1**,
   detectando la mutación.

Consecuencia: **toda** afirmación sobre el estado del pipeline debe ejecutarse
con `--rerun`. Un SUCCESS sin `--rerun` no es evidencia de nada: puede
certificar un árbol de trabajo que ya no compila. Debe tratarse como cacheado, no
como verificación.

El cache key de compilación
(`sha256:9f26bb0571919bd012ebb6736de43b3b8bffe566b904dc76baab69923f6d96d6`)
no cambia entre un árbol limpio y uno mutado, por eso la invalidación no ocurre.

#### Revalidado en v0.43.0 (2026-09-30) — el defecto NO está arreglado

El incidente se reprodujo de nuevo sobre la versión actual, así que **no es
deuda obsoleta**: el criterio sigue vigente y `--rerun` sigue siendo obligatorio.

Secuencia observada, con control explícito para no leer un `tail` como verde:

1. `panic!` inyectado en `execution_log_read_over_the_wire`.
2. Control: `cargo test -p chronos-sandbox --test execution_log_read_e2e` →
   `FAILED. 1 passed; 1 failed`. El fallo es real.
3. `pipelinek run` **sin** `--rerun` → `Pipeline finished with SUCCESS`,
   **exit 0**, con `RunFinished/success` y 5 `StageFinished/success` pero
   **cero `StepStarted`** (el journal pasa de `CompilationFinished` en seq 43 a
   `RunStarted` en seq 44 y salta directo a stages).
4. Mutación revertida; `pipelinek run --rerun` → 9 `StepStarted`, 0 `StepFailed`,
   E2E `2 passed; 0 failed`, exit 0.

Un SUCCESS sin `--rerun` es por tanto un **falso verde autenticado** sobre un
árbol que no pasa sus propios tests. No lo leas como verificación bajo ninguna
circunstancia.

Registrado como debt de backlog `bl-bl-01M3PWNDVW000387DS8ZH6DX00`. `pipelinek`
es un binario distribuido, no código de este repo, así que la causa raíz no
puede corregirse aquí; `--rerun` es la única mitigación honesta hasta que se
arregle la invalidación del cache.

### Criterios de éxito (todos deben cumplirse)

1. `Pipeline finished with SUCCESS` en la línea final del run.
2. Journal SQLite presente en `.pipelinek/db.sqlite` con eventos tipados
   (`CompilationStarted`, `RunStarted`, `StageStarted`, `StepStarted`,
   `EchoOutputCaptured` o equivalente, `StageFinished/success`,
   `RunFinished/success`).
3. Control root presente en `.pipelinek/control/{last-run, retry-control,
   wait-until-control, workspace/<stage-name>}`.
4. Cero `StepFailed` ni `RunFinished/failure` en el journal del último run.
5. SHA-256 del `.pipeline.kts` registrado en la sesión y comparable con
   `git log -- .pipeline.kts` para detectar drift no intencional.

### Comando de validación rápida

```bash
test -f .pipeline.kts && \
  pipelinek validate .pipeline.kts && \
  echo "pipelinek CI local: configuración válida"
```

### Extensión del script

Cualquier stage nuevo debe:

* Declarar su propósito en el `echo` inicial del stage.
* Usar **rutas absolutas** dentro de los `sh(...)` (el motor no
  resuelve el cwd del script).
* Producir efectos secundarios solo a través de los directorios
  `.pipelinek/` y `evidence/` (no contaminar el árbol del proyecto).
* Mantener `discover-repo` como primer stage para que un run nuevo
  siempre documente el estado del repositorio.
* Respetar la separación entre `evidence/`, `cycle-artifacts/`,
  `metrics/` y `.atl/`: nada del CI debe escribirse fuera de esos
  directorios o de `.pipelinek/`.

### Compatibilidad con otros runners

`pipelinek` es la fuente de verdad local. GitHub Actions, GitLab CI,
Jenkins o cualquier otro runner remoto **debe** invocar el mismo
`.pipeline.kts` desde el mismo checkout. Si un runner remoto produce
PASS y `pipelinek` local produce FAIL, prevalece `pipelinek` local hasta
que la divergencia se investigue y documente en este mismo archivo.

### Excepciones documentadas

Ninguna hasta la fecha. Toda excepción requiere entrada en
`SESSION-JOURNAL.md` y aprobación explícita del maintainer del proyecto.

---

## Identidad del proyecto SDDK — deriva de casing (2026-09-30)

### Regla

Este repositorio está **pinnado** a `p-3416cfb8288f8964` mediante
`.sddk/project-pin.json`. No lo borres sin leer esta sección.

### Qué pasó

`git remote.origin.url` es `git@github.com:Rubentxu/chronos.git` — con **R mayúscula**.
SDDK 2.3.1 normaliza ese remote a minúsculas y deriva
`project_id = p-55f14aab9263c12f`, que **no existe en el ledger canónico**:
0 eventos, 0 ciclos, vault vacío.

Consecuencia práctica: `sddk context bootstrap` creó una adopción vacía y todos
los comandos que **infieren** el proyecto (`sddk cycle status`,
`sddk adopt status`, `sddk context bootstrap`) reportaban un proyecto sin
estado, ocultando el ledger real con **465 eventos y 86 ciclos**.

### El pin no lo respeta todo

Comportamiento observado con el pin aplicado, y reproducible:

| Comando | Respeta el pin |
|---|---|
| `sddk project resolve` | **sí** |
| `sddk ledger verify` | **sí** |
| `sddk backlog list` / `backlog show` | **sí** |
| `sddk adopt status` / `adopt plan` | **no** — sigue en `p-55f…` |
| `sddk context bootstrap` | **no** — sigue en `p-55f…` |
| `sddk cycle status` (inferido) | **no** — sigue en `p-55f…` |

### Cómo trabajar mientras tanto

Usa el flag explícito `--cycle` para todo comando de ciclo:

```bash
sddk cycle status --cycle p-3416cfb8288f8964/release-pipeline-honesty-v014
sddk cycle next    --cycle p-3416cfb8288f8964/<ciclo>
```

Y verifícalo con `sddk ledger verify`: debe devolver **465 eventos**. Si
devuelve 0, estás leyendo el proyecto equivocado.

### Debt

`bl-bl-01M3SP3DT5000387KC746KRBG0` (P1). Arreglo upstream: normalización de
casing en la resolución de identidad, o que todo comando que lee identidad
consulte el pin.

---

## Estado del ledger: los ciclos no cerrados no son homogéneos (2026-09-30)

Clasificados por **evidencia observada**, no por su campo `status`. La
distinción importa: cerrar un ciclo entregado y descartar un residuo son
operaciones distintas, y una exige autoridad que el agente no tiene.

| Grupo | Ciclos | Artefactos | Qué hacer |
|---|---|---|---|
| A. Entregado y verificado | `m9-81` | 7 | Cerrar; requiere aprobación humana |
| B. Entregado, cierre pendiente | `m9-04` | 6 | Reconstruir; bloqueado, ver abajo |
| C. Entregado, gate humano | `m10-readpath` | 6 | Cerrar; requiere `uat.toml` + 2 firmas |
| D. `BLOCKED` con evidencia | `m9-62`, `m9-63`, `m9-64` | 6 cada uno | Desbloquear upstream |
| E. Residuo sin evidencia | 9 ciclos | **0** | Descartar, no reimplementar |

### El grupo E no es deuda técnica

`m5-02-query-service-extract` y `m5-02a-events-extract` llevan `PAUSED` desde
2026-09-09 con cero artefactos, pero su trabajo **ya está implementado**:

- `crates/chronos-query/` expone `QueryEngine` (`engine.rs`), `expr_eval.rs` y
  `projection.rs` — 2742 líneas.
- `crates/chronos-services/src/query_service.rs` — 125 líneas.
- Events vive en `events_read.rs` (426) y `events_cursor.rs` (386).

Son ciclos cuyo trabajo se entregó por otro camino y que nadie cerró.
Reimplementarlos sería duplicar código que ya existe.

El resto del grupo E: `m5-preflight-clippy-drift-cleanup`,
`m5-preflight-2-sandbox-drift-cleanup`, `m-ci-flake-preflight`,
`m-ci-flake-cleanup-2-residual-timing`, `m10-product-evolution-propose`,
`train-b`, `m9-88-find-m9-81-fk-investigation`.

### Por qué el agente no puede cerrar nada

`sddk cycle supersede` responde:

```
error: ADMISSION: approval required before mutating 'cycle_state'
(decision_id=approval-system-cycle_supersede)
```

Solo `sddk approval grant` lo levanta, y su `--actor` está documentado como
*"Human actor id"*. **No lo emitas desde un agente**: sería simular una
aprobación humana. Registrado como `bl-bl-01M3SYRX4M000387KXJ99CHMC0` (P1).

`m9-04` añade un segundo bloqueo: `sddk cycle next` responde
`has no replayable state events`. Su historia empieza en una transición
(`workflow.transition.succeeded` del 2026-09-11) sin evento `cycle.created`
previo, así que `sddk cycle rebuild` no tiene de qué reconstruir.

Detalle menor detectado al verificar `m9-04`: cuatro de los seis SHAs de su
`apply-checkpoint.json` no resuelven — `083f5ba`, `7f86a1c`, `6ec8757` y
`140d53a` tienen 7 caracteres donde los otros dos tienen 8. No se perdió
trabajo: los seis commits existen y son los correctos. Es un error de
transcripción de un carácter en el fichero de bookkeeping.

### Los tres bloqueos de cierre, y por qué son distintos

El grupo D (`m9-62`, `m9-63`, `m9-64`) **no** está bloqueado por aprobación
humana. Su transición `archive.vault.complete` pide dos artefactos
(`vault-receipt`, `archive-manifest`) y tres gates. El primero de esos
artefactos es lo que falla:

```
$ sddk release vault --cycle p-3416cfb8288f8964/m9-62-bounded-stop-probe
error: cycle ... has no delivery_kind declared; vault route requires
ManagedClosureDelivery
```

**0 de los 86 ciclos del proyecto declara `delivery_kind`.** Tampoco los 67
`CLOSED`. Y el valor requerido no es obtenible:

- `find ... -iname "*0075*"` en el bundle 2.3.1 → nada. ADR-0075, que `sddk
  release vault --help` cita como procedencia de la decisión, no se distribuye.
- El bundle no incluye ningún fichero ADR.
- `grep -r delivery_kind` sobre el framework → cero resultados.
- `sddk cycle replan` solo acepta `--restage-to` y `--delta`.

No hay ninguna vía por CLI para declarar el valor. Editar el manifest a mano
en `ledger.sqlite` sería fabricar estado canónico, así que no se ha hecho.

Los tres ciclos están genuinamente entregados —`b98b2a4f`, `8dc1063d` y
`339f7b5e` son ancestros del trunk; `v0.7.64`, `v0.7.65` y `v0.7.66` peel
exacto; CC#52 existe en `vault-drift-sweep.md:2582`; el
`bounded_join_with_timeout(..., Duration::from_secs(10))` está en
`probe_backend.rs:728`; `scripts/check_vault_drift.sh` y
`.github/workflows/vault-drift.yml` existen— y sus checkpoints ya declaran
`status: CLOSED` y `archive_status: archived`. **El estado del ledger es la
anomalía, no el trabajo.**

Resumen de los tres bloqueos, para no re-derivarlos:

| Ciclo | Transición | Bloqueo | ¿Lo resuelve un humano? |
|---|---|---|---|
| `m9-81` | `cycle.supersede` | `ADMISSION` → `sddk approval grant` | Sí, con `grant` |
| `m10-readpath` | `release.complete` | `release-uat-approved`: `uat.toml` + 2 firmas | Sí |
| `m9-04` | — | sin evento `cycle.created`; replay imposible | No, requiere reconstructura |
| `m9-62/63/64` | `archive.vault.complete` | `delivery_kind` no declarable | No, requiere fix upstream |

Registrado como `bl-bl-01M3SZDDVD000387KYVFNVQGC0` (P1). Es la segunda
aparición del mismo hallazgo tras `CLOSE-OUT-2026-09-29`, aún sin resolver.

### Con eso quedan tipificados los 19 ciclos no cerrados

Ninguno ofrece trabajo de producto pendiente. Todos son cierre pendiente,
residuo, o bloqueados por una de las cuatro causas de la tabla anterior.

`m9-88-find-m9-81-fk-investigation` y `train-b` son **residuo**: tres eventos
cada uno, con `cycle.created` y `cycle.block` compartiendo `sequence: 1` en el
mismo instante, cero artefactos, y el directorio de `m9-88` inexistente en
disco. `grep -r "m9-88\|FOREIGN KEY" **/*.rs` → 0 resultados: la investigación
de foreign key nunca llegó al código.

Backlog: `bl-bl-01M3T0VXDS000387M1NFVT8VG0` (P2).

---

## `crates/chronos-mcp/src/server.rs` — composición medida (2026-09-30)

8170 líneas, 2.7× el siguiente fichero del workspace
(`chronos-services/src/output.rs`, 3001). No estaba registrado como deuda en
ningún sitio. Auditoría de solo lectura, sin cambios.

### Hipótesis descartadas antes de proponer nada

| Hipótesis | Verificación | Resultado |
|---|---|---|
| Código generado o repetitivo | 4 handlers, una macro `tool_handler!` | Descartada |
| Tests inflando el tamaño | `#[cfg(test)]` empieza en la línea 8118 de 8170 | Descartada |
| Falla un lint | `cargo clippy -p chronos-mcp --all-targets` → 0 warnings | Descartada |

### Composición real

| Bloque | Líneas |
|---|---|
| `impl ServerHandler` | 2953 |
| `impl ChronosServer` | 2952 |
| Resto: 110 bloques top-level | 2265 |

Los 2265 restantes son 81 `struct`/`enum`/`type`, 45 funciones puras, 8
`const` y 3 módulos de test inline. Los dos `impl` suman el **72%** del
fichero.

### Por qué el corte es viable

La región no-`impl` toca el estado del servidor **12 veces en 2433 líneas**,
y de sus 4 `impl` internos solo `impl ChronosServer` depende de él. Los otros
tres son `impl TripwireConditionType`, `impl SessionCompareWire` e
`impl Default for ChronosServer`. Los 67 `Params` ya son `pub`.

Es decir: la región es casi todo tipo y función pura. Extraerla a un módulo
`tools_params` no requiere reescribir lógica ni tocar los 44 métodos del
handler.

### Precedente en el propio crate

| Módulo | Líneas |
|---|---|
| `composition.rs` | 458 |
| `security.rs` | 201 |
| `concurrency_wire.rs` | 191 |
| `cost_memory_wire.rs` | 172 |
| `telemetry_wire.rs` | 115 |

Todos son *composition-root seams* con documentación a nivel de módulo.

### Por qué se hizo el split en vez de dejarlo pendiente

Se hizo el 2026-09-30, tras medir. Criterio de aceptación mecánico: el
baseline `cargo test -p chronos-mcp` de esa revisión era **192 passed,
0 failed**, y el refactor debía dejarlo exactamente igual.

| Fichero | Antes | Después |
|---|---|---|
| `server.rs` | 8170 | 6474 |
| `tools_params.rs` | — | 1713 |

Lo que se movió son 74 `Params`/enums, 45 funciones puras, 8 `const` de
toolset y los parsers. Se quedaron en `server.rs` los `impl ChronosServer` y
`impl Default for ChronosServer`, que sí tocan estado, más los helpers
`text_content` / `json_content` / `session_envelope`, que son infraestructura
de respuesta compartida con los handlers.

`server.rs` hace `pub use crate::tools_params::*`, así que
`chronos_mcp::server::*` sigue resolviendo a los mismos elementos y los tests
de integración no cambian.

Resultado verificado: 192 passed / 0 failed (idéntico al baseline),
`clippy -D warnings` limpio, `cargo fmt` limpio, workspace compila,
`pipelinek --rerun` runId `07dbfe7f` PASS.

Backlog: `bl-bl-01M3T0VGK6000387M1N5921NW0` (P2).

### Lo que NO se puede hacer: extraer los tests inline

Se intentó el 2026-09-30 y se revirtió. Los 2279 tests inline son la mayor
porción restante, pero **no son separables**, y el motivo es encapsulación,
no volumen.

Los tests llegan a `ChronosServer` por `use super::*`. Ese struct tiene
**0 campos `pub` y 0 métodos `pub(crate)`**: todo es privado por diseño, y el
composition root basado en ports depende de esa privacidad. Extraer los tests
exigía abrir 28 métodos y 13 campos.

Peor: al hacerlo, el widening automático **degradó dos ítems que ya eran
`pub fn`** — `is_tool_listed` y `active_toolset` pasaron a `pub(crate) fn` — y
eso rompió `crates/chronos-mcp/tests/server_cohesion.rs`, porque los ficheros
bajo `tests/` compilan como crate aparte y `pub(crate)` no llega hasta allí.

| | Extracción que sí entró | Extracción que se revirtió |
|---|---|---|
| Región | params y constantes (275–2703) | tests inline |
| Símbolos que hubo que abrir | 19, todos ya `pub` de intención | 41, casi todos privados a propósito |
| Efecto en tests externos | ninguno | rompió `server_cohesion.rs` |
| Coste de reversión | — | `git checkout`, 5 min |

**Conclusión:** partir `server.rs` más allá de la región de params exige abrir
`ChronosServer`, y eso es una decisión de diseño con coste real, no un
refactor mecánico. Los 4196 líneas de producción restantes son dos `impl`
(2952 router, 688 construcción) más el struct, y tienen el mismo prerrequisito.

Backlog: `bl-bl-01M3T2KD09000387M55HZKHG80` (P3).

---

## Las dos bases de conocimiento: no hay conflicto (2026-09-30)

`.sddk-knowledge/` (294 ficheros) contra
`~/.sddk-knowledge/p-3416cfb8288f8964/` (168 ficheros):

| Categoría | Rutas |
|---|---|
| Solo en el repo | 205 |
| Solo en el vault externo | 34 |
| Mismo path, contenido distinto | **0** |

**No hay nada que fusionar.** El item de backlog que decía "241 paths differ"
confundía *presencia* con *divergencia*, y por eso estaba en P1 sin que hubiera
nada que arreglar. Descartado como `superseded` por
`bl-bl-01M3T2W3BT000387M5VPCBCPC0`.

Quién tiene qué: el externo posee `adrs/` y 16 ciclos archivados que el repo no
tiene, y su `_log.md` tiene entradas del 2026-09-29, así que **sigue
escribiéndose**. El repo posee 205 rutas más, incluidos los 102 manifests de
`changes/archive/` que `m9-76` construyó para que `CC#4` verifique sus SHAs, y
`vault-drift.yml` protege el del repo. Los dos crecen.

### Aviso de método: `diff` está localizado en este host

`diff -rq` imprime `Sólo en …` en español. Un `grep "Only in"` devuelve **cero
sin fallar**, y la lectura resultante —"no hay ninguna ruta exclusiva"— es
falsa. Ya produjo una conclusión equivocada en esta sesión. Filtra por
`Sólo en` o usa `comm`, no texto en inglés.

### Lo que sí hay: 50 ciclos CLOSED sin artefactos en ningún sitio

| Medición | Valor |
|---|---|
| Ciclos `CLOSED` en el ledger | 67 |
| Con directorio en `cycle-artifacts/` | **14** |
| Sin directorio | 53 |
| De esos, ausentes también del vault externo | **50** |

Para esos 50, `git log --all -- <path>` devuelve **cero commits**: nunca
materializaron artefactos en este repositorio. El rango va del 2026-09-08 al
2026-09-28, así que no es una regresión reciente.

El ledger registra ciclos que el repositorio nunca contuvo. El ledger es la
autoridad para el *estado* de un ciclo, y eso es compatible con que los
artefactos sean opcionales; pero nada en el repo lo declara, y la suposición
"los artefactos están en el vault" es insegura con 50 huecos detrás.

Backlog: `bl-bl-01M3T2W3BT000387M5VPCBCPC0` (P2).

---

## El ledger no registra dos tercios del trabajo (2026-10-01)

Auditoría del registro de deuda contra los criterios vigentes. El resultado
es más grave que la nota del 2026-09-30 sobre los "50 ciclos CLOSED sin
artefactos": ese hallazgo miraba un solo lado. Los dos lados casi no se
solapan.

| Población | Cantidad |
|---|---|
| Ciclos en el ledger canónico (`p-3416cfb8288f8964`) | 86 |
| Directorios en `cycle-artifacts/p-3416cfb8288f8964/` | 150 |
| Coincidencia exacta (artefacto **y** ciclo) | **24** |
| Artefactos **sin** ciclo en el ledger | **126** |
| Ciclos **sin** directorio de artefactos | **62** |

### Los 126 huérfanos son trabajo real, no borradores

Cada verificación se hizo por separado, porque cada una podría dar un "0" que
se lee mal:

1. **No están como filas de ciclo.** `cycles` tiene 86 filas y ninguna coincide.
2. **No están en ningún evento.** `events_v1` sobre `cycle_id` y `subjects_json`
   para 40 de ellos muestreados: 0 coincidencias.
3. **No están en el vault externo.** `~/.sddk-knowledge/p-3416cfb8288f8964/cycles/`
   tiene 10 entradas; 0 de los 126.
4. **No están en ningún ledger del host.** Se sondearon los 322
   `p-*/ledger.sqlite` de `~/.local/state/sddk/projects/` buscando tres
   representativos (`m9-76-cc4-regen-tool-in-repo`,
   `m9-60-cycle-artifacts-existence`, `m9-90-stale-branches-cleanup`):
   0 coincidencias en todos.
5. **Son trabajo entregado.** `git log --all -- <ruta>` para los 126:
   **126/126 tienen commits que tocan su ruta.** Ninguno es un borrador.

Entre ellos está la espina dorsal histórica del proyecto: la serie
`m9-05` … `m9-98`, unas 90 unidades consecutivas con su juego completo de
`change-entry`, `verify-findings`, `verify-report`, `merge-receipt` y
`release-receipt`. `m9-76-cc4-regen-tool-in-repo` es la que construyó los 102
manifests de los que el propio AGENTS.md depende para `CC#4`. Ninguna de esas
unidades es visible para SDDK.

### Consecuencia: "el ledger es la autoridad" hay que acotarlo

Toda conclusión sacada de "el ledger dice X" describe, como mucho, 24 de las
150 unidades de artefacto y 86 de las ~212 unidades de trabajo reales
(86 ciclos + 126 huérfanos) que este repositorio contiene. `cycle.created`
es 26 de 472 eventos: 60 filas de `cycles` existen sin ese evento, así que el
snapshot y el stream ya discrepan internamente.

Esto no invalida los hallazgos anteriores — los 86 ciclos del ledger son
reales y su clasificación sigue siendo válida — pero sí acota su alcance:
**describen el registro de SDDK, no el proyecto.**

### El error que se repite, y que ya ha mordido dos veces

Este hallazgo nació de una afirmación falsa en el backlog:
`bl-bl-01M3T0VXDS000387M1NFVT8VG0` dice "the m9-88 directory does not exist on
disk". Es falso: existe `cycle-artifacts/.../m9-88-cc55-drift-remediation`,
con 7 ficheros y 44K. La razón es que se buscó el slug completo
`m9-88-find-m9-81-fk-investigation`, que efectivamente no tiene directorio, y
se concluyó que no había ningún `m9-88`. Pero `m9-88-cc55-drift-remediation`
es **otra unidad de trabajo**, y no es ningún ciclo del ledger.

Es el mismo error dos veces, en direcciones opuestas:

- Con `m9-80` y `v014`: se buscaron prefijos desnudos creyendo que eran ids
  completos, y se concluyó que no tenían registro. Retractado en
  CLOSE-OUT-2026-09-29.
- Con `m9-88`: se buscó el id completo creyendo que cubría el prefijo, y se
  concluyó que el directorio no existía.

**Regla:** un `m9-NN` es un *prefijo de familia*, no un identificador. Antes
de afirmar que algo no existe, hay que buscar por prefijo (`m9-88*`) y luego
distinguir los ids completos. Y antes de afirmar que algo existe, comprobar
que es el id completo y no un hermano.

### Qué parte de SDDK está rota y qué parte no (2026-10-01)

Matiz importante, porque la caracterización amplia ("SDDK no funciona") es
incorrecta y llevaría a **decisiones equivocadas**. El pin sí se respeta
en unas superficies y en otras no. Medido el 2026-10-01:

| Superficie | ¿Honra el pin? | Evidencia |
|---|---|---|
| `project resolve` | **sí** | `identity_source: pinned`, devuelve `p-3416cfb8288f8964` |
| `ledger verify` | **sí** | 472 eventos canónicos |
| `backlog list` | **sí** | 11 items vivos, los canónicos |
| `backlog capture` / `discard` / `triage` | **sí** | un `discard` escribiría 1 evento nuevo; el canónico pasó de 34 a 35 en `backlog_item_events_v1` |
| `adopt status` | **no** | devuelve `p-55f14aab9263c12f` |
| `cycle status` | **no** | idem, incluso con `--root`/`--scope`/`--remote` explícitos |
| `context bootstrap` | **no** | enlaza la sesión al fantasma |
| `context delta --publish` | **no** | el delta de cierre se publicó en el fantasma |

**Consecuencia práctica:** el plano *ledger + backlog* funciona y es
mantenible — se puede registrar deuda, triarla y descargarla contra el
proyecto canónico. Lo que está roto es el plano *ciclo* (crear, transicionar,
archivar) más el binding de sesión. Eso acota el daño: **la trazabilidad se
puede sostener hoy; el ciclo de vida de ciclos, no.**

El proyecto fantasma está además completamente vacío: 0 en `events_v1`,
0 en `backlog_items_v1`, 0 en `backlog_item_events_v1`. No contiene historia
real que se pueda recuperar de él, así que **no fusionar nada hacia él**: eso
solo crea ruido invisible. La decisión de si migrar el estado canónico al id que SDDK
deriva hoy sigue siendo del maintainer, por lo que implica en
`reconstruction-contracts.toml` y en las rutas de artefactos.

### Auditoría del registro de deuda: un item ya no era deuda

`bl-bl-01M3T0VGK6000387M1N5921NW0` ("server.rs is 8170 lines", P2) se
descargó como `superseded`, con sucesor
`bl-bl-01M3T2KD09000387M55HZKHG80`. Motivo: la extracción que el propio item
recomendaba se ejecutó en `22f49b08`, con `server.rs` de 8170 → 6474 líneas y
`tools_params.rs` naciendo con 1713. Verificado el 2026-10-01 sobre `12c11e6a`.

El framework impone esa relación: `--reason superseded` sin `--superseded-by`
se rechaza con "discarding without a successor loses findings that lived only
in this item". Es correcto: el hallazgo de que `server.rs` es grande no se
pierde, porque su sucesor lo sigue arrastrando — con el número corregido a
**2316** líneas de tests inline (el módulo `mod tests` abre en la línea 4159 de
6474; los `#[cfg(test)]` de 430/675/697 son ítems sueltos en la región de
producción, no el módulo de tests) y **0** campos `pub` en `ChronosServer`, así
que el bloqueo sigue siendo encapsulación y no volumen.

## Los adaptadores de lenguaje colgaban y fugaban procesos (2026-10-01)

WorkItem `language-adapter-spawn-has-no-real-test`. El defecto que motivó el
item era la fuga de procesos, pero al ejecutar los tests apareció otro más grave
en el mismo camino de código.

### La fuga, que era la esperada

`DelveSubprocess::spawn`, `JavaSubprocess::spawn` y `PythonSubprocess::spawn`
tenían un `impl Drop` **vacío** con el comentario *"SIGTERM is sent
automatically when Child is dropped"*. Es falso: `tokio::process::Child` solo
mata al dropearse si se configura `kill_on_drop`. Un `dlv` huérfano sobrevivió
**11m36s** reparentado a `systemd --user`. Corregido con
`cmd.kill_on_drop(true)` —el patrón que `chronos-js` ya usaba— y eliminado el
`Drop` vacío, que además mentía.

Python era la tercera copia literal del mismo defecto: mismo fichero, misma
estructura y el mismo comentario falso pegado. Se detectó al buscar **los demás
adaptadores que lanzan procesos** tras arreglar dos: ese barrido es parte del
trabajo, no una extracurricular. Resultado del barrido, sobre los seis
adaptadores que lanzan procesos:

| Adaptador | Mecanismo | Estado |
|---|---|---|
| `chronos-js` | `kill_on_drop` | ya correcto |
| `chronos-browser` | `Drop` propio con `kill()` + `wait()` | ya correcto |
| `chronos-go` | `kill_on_drop` | corregido aquí |
| `chronos-java` | `kill_on_drop` | corregido aquí |
| `chronos-python` | `kill_on_drop` | corregido aquí |
| `chronos-ebpf` | `std::process`, guarda **solo el pid** y mata por `kill -9` en `stop_capture` | **otro diseño**: `kill_on_drop` no aplica |

eBPF queda **fuera de este item y sin corregir**, y conviene decirlo claro: al
no retener el `Child`, depende de que se llame a `stop_capture`; si la sesión se
dropea sin él, el target queda vivo. No es el mismo defecto ni se arregla con la
misma línea, y su test ya está `#[ignore]`d por `CAP_BPF`. Queda como
candidato, no como deuda ya cerrada.

Los dos tests que deberían haberlo cazado no afirmaban nada: el de Go descartaba
el resultado con `let _result = ...` y su propio comentario decía *"we just
verify it doesn't panic"*; el de Java compilaba una clase y terminaba en *"so we
just verify the spawn function works"*. Ambos dropeaban el handle dejando el
proceso vivo. **Un test que no afirma nada no es cobertura: es la razón por la
que el defecto llegó a `main`.**

### El defecto grave: el banner JDWP se leía del stream equivocado

`JavaSubprocess::spawn` leía **solo stderr**, y su parser solo reconocía el
formato antiguo `address: 127.0.0.1:<port>`. temurin 24.0.2 escribe el banner a
**stdout** y sin el prefijo de host:

```
$ java -agentlib:jdwp=...=address=127.0.0.1:0 -cp . NoSuchClass >/tmp/out 2>/tmp/err
$ cat -A /tmp/out | head -1
Listening for transport dt_socket at address: 52849$
$ cat -A /tmp/err
(vacío)
```

Con `suspend=y` la JVM **no muere**: queda suspendida esperando debugger. Así que
`read_line` se quedaba bloqueado para siempre sobre un pipe de stderr vacío.
`JavaAdapter::attach` (`adapter.rs:134`) la invoca dentro de un `block_on`, luego
**el adaptador Java colgaba sin error y sin timeout**: inusable en cualquier JDK
soportado. Dos defectos encadenados —stream equivocado y formato equivocado— que
el bucle no acotado convertía en un cuelgue silencioso en vez de un error.

**Cómo pasó inadvertido durante tres caracterizaciones:** todas fusionaban los
flujos con `2>&1` (`2>&1 | cat -A`, `> file 2>&1`, `java -version 2>&1 | head`),
así que el stream **nunca fue observable**. Separarlos fue lo que reveló el
defecto. Cuando se sospeche de un proceso hijo, `> /tmp/o 2>/tmp/e` y mirar
ambos por separado; no fusionar.

Corrección: se vigilan **los dos flujos** (`tokio::select!`, y un flujo ya
cerrado devuelve `pending()` en vez de `None` para no hacer spin sobre EOF), se
aceptan **las dos formas** del banner, y la espera se acota con
`JDWP_PORT_TIMEOUT` para que un banner irreconocible sea un error y no un
cuelgue. `kill_on_drop` garantiza además que la JVM muera en la ruta de error.

### Por qué los tests de `spawn` siguen siendo `#[ignore]`

`ci.yml` no declara `java` ni `dlv`. Un test que hace `return` temprano cuando
falta la herramienta **convierte un verde vacío en un falso verde**, que es
justo lo que este repo prohíbe. Se mantienen omitidos por defecto y los ejecuta
el tier opt-in del gate.

La lección es que **la ruta de lectura sí se puede cubrir sin toolchain
externo**: cinco tests alimentan banners por `sh` a través del bucle real
(`read_loop_accepts_the_modern_banner_on_stdout`,
`..._legacy_banner_on_stderr`, `..._skips_noise_before_the_banner`,
`..._errors_when_the_jvm_exits_silently`,
`..._is_bounded_against_a_silent_jvm`). Esos **sí corren en el gate por
defecto**, sin `java` instalado. Un contrato que se puede fijar sin la
dependencia externa, se fija sin ella.

### Verificación

Mutation-tested en ambos commits, porque un guard que no falla al quitarle el
fix no es un guard:

| Qué se muteó | Resultado |
|---|---|
| `read_jdwp_port` solo con `stderr` (el bug original) | 2 tests `FAILED`, reproduce el cuelgue como error a los 30 s |
| `kill_on_drop` desactivado (Go) | el guard falla: *"el proceso dlv 3079456 seguía vivo tras dropear"* |
| `kill_on_drop` desactivado (Java) | el guard falla y reproduce la fuga |
| `kill_on_drop` desactivado (Python) | el guard falla tras los 10 s de sondeo, y el `python3` sobrevive |

Control tras revertir ambos: 4 tests opt-in verdes y **0 procesos huérfanos**
(antes se contaban por ejecución). Gate local TIER 1 `f1eea6e6`:
`RunFinished: success`, 9/9 stages, 18 `StepStarted`, **0 `StepFailed`**.
## `Inspector.detached` nunca llegaba: variante unidad con `params` (2026-10-01)

Alerta registrada el día anterior en `cdp_client.rs:676` y marcada como
"limitación de serde, no bug de código". **Verificada, y era understated:** no
era una limitación de serde, era un defecto funcional, en dos crates.

### Qué pasaba

`CdpEventType` usa un enum **adyacente**: `#[serde(tag = "method", content =
"params")]`. Con esa forma, serde solo acepta una variante **unidad** si el
mensaje **no trae** la clave `params`. Y Chrome **siempre** envía
`Inspector.detached` con `params`, porque `reason` es obligatorio según el
protocolo.

Medido sobre el código real, no deducido:

| JSON | Pre-fix |
|---|---|
| `{"method":"Debugger.resumed"}` | OK |
| `{"method":"Debugger.resumed","params":{}}` | **ERR** |
| `{"method":"Inspector.detached"}` | OK |
| `{"method":"Inspector.detached","params":{"reason":"target_closed"}}` | **ERR** |

### Por qué era grave, no cosmético

`InspectorDetached` es la **única señal que termina los bucles de captura**:

- `chronos-browser/src/adapter.rs:281` → `s.running = false; break;`
- `chronos-browser/src/wasm_detector.rs:94` → `break`
- `chronos-js/src/adapter.rs:212` → mismo patrón

Como el evento nunca deserializaba, cada detach caía en el `warn!("Failed to
parse CDP event")` y se descartaba. Ni el bucle de captura ni el detector de
WASM podían cerrarse por esa vía. **Y `chronos-js` tenía el mismo enum
duplicado con el mismo defecto** — el barrido de "responsabilidades similares"
lo saca otra vez.

### El arreglo

Las dos variantes pasan a ser tupla con `Option<Value>`, que acepta **ambas**
formas. Se comprobó antes de escribir el fix que `Option` como `content` resuelve
con y sin `params` (`Resumed(None)`, `Resumed(Some(Object {}))`,
`Detached(Some({"reason": "target_closed"}))`).

Deliberadamente **no** se arregló el `Other` catch-all, que sigue sin funcionar
para eventos desconocidos con `params`: hacerlo los dejaría pasar al broadcast y
volcaría `Network.*` y `Page.*` en todos los consumidores. Es un cambio de
comportamiento de riesgo desconocido, no una corrección. Se deja como está, con
su test de caracterización, que **sigue siendo exacto** y se verificó otra vez
tras el cambio.

### Verificación

Mutation-tested contra el código **pre-fix**: el test
`test_cdp_event_inspector_detached_with_params_parses` falla con *"Inspector.detached
con params debe parsear"*. Post-fix, 4 tests nuevos en `chronos-browser` y 2 en
`chronos-js` cubren ambas formas, **sin `#[ignore]` y por tanto dentro del gate
por defecto** — que es la diferencia con el resto de adaptadores: aquí no hace
falta Chrome para fijar el contrato.

Sin Chrome en el host, la evidencia es deserialización contra la forma exacta
del protocolo, no un detach observado en vivo. Es conocimiento negativo que
conviene no olvidar.
## La capacidad `browser-probe` se anunciaba disponible donde no podía (2026-10-01)

Tercera instancia de la misma familia: **un nombre o comentario que afirma algo
que el código no hace**. Encontrada al barrer los tests sin aserción.

### Qué pasaba

```rust
pub fn is_chrome_available() -> bool {
    ChromeProcess::attach_port(9222).is_ok()   // NO es una sonda de alcance
}
```

`attach_port` **no consulta nada**: crea un `TempDir`, compone las URLs y deja
`process: None`. Solo puede fallar si no se puede crear el tempdir, así que en
cualquier host normal devuelve `Ok`. Es decir, `is_chrome_available()` devolvía
`true` **siempre**.

Medido en este host, que **no tiene Chrome**:

```
PROBE is_chrome_available()      = true      <-- falso positivo
PROBE ChromeLocator resolve ok   = false     <-- la verdad
PROBE attach_port(9222).is_ok()  = true      <-- por eso
```

Y `BrowserProbeFactoryImpl::create()` hace short-circuit con
`if !is_chrome_available()`. Como nunca era `false`, **la puerta no disparaba
nunca**: la capacidad `browser-probe` se anunciaba como disponible en máquinas
que no pueden ejecutarla, y el fallo solo aparecía más tarde y más hondo, como
un error de spawn.

El repo ya tenía la respuesta correcta a un metro de ahí: `ChromeLocator`, que
resuelve `CHROME_PATH` y la lista de candidatos, y que además está diseñado para
poder testearse sin depender del host. El defecto era **solo de cableado**.

### El doc registraba la observación contraria

`H1.5-runtimes-capabilities-benchmarks.md` afirmaba que en el host base
`is_chrome_available()` devuelve `false`. **No era reproducible**: bajo el
cableado antiguo devolvía `true` incluso sin navegador. Corregido en el doc,
marcando qué se midió antes y qué se mide ahora, porque un documento que
consigna un dato falso engaña tanto como el código que lo produce.

### El test que lo tapaba

`test_browser_adapter_is_available` hacía `let _available = ...`, sin afirmar
nada. Su comentario — *"Just verify the method works"* — es de la misma familia
que el ya corregido en los adaptadores de lenguaje. Hay **cinco** tests
`*_is_available` así en los adaptadores; son el siguiente barrido pendiente.

### Arreglo y verificación

`is_chrome_available()` pasa a `ChromeLocator::from_host().resolve().is_ok()`, y
el test ahora fija el cableado con tres casos deterministas: coincidencia con
el descubrimiento del host, locator vacío ⇒ no disponible, y binario existente
⇒ disponible (sin depender de que haya Chrome).

Mutation-tested: al volver a `attach_port`, el test falla con `left: true,
right: false`, que es exactamente el falso positivo que se estaba publicando.
49 tests del crate y 87 de `chronos-mcp` (consumidor de la capacidad) en verde.
## `waitpid(-1, __WALL)`: los tests de ptrace se robaban los hijos (2026-10-01)

`cargo test -p chronos-native --lib` fallaba 3 tests de `ptrace_tracer` y
**colgaba** 2 de `capture_runner` de forma intermitente. El gate local pasaba
limpio, lo que lo hacía parecer un problema del entorno. No lo era.

### La causa

`PtraceTracer::wait_event` tiene dos ramas:

- `follow_children: true` (o sin `main_pid`) → **`waitpid(-1, __WALL)`**, que
  reape **cualquier hijo del proceso actual** (`ptrace_tracer.rs:482`).
- `follow_children: false` → `waitpid(pid, WNOHANG)`, acotado a un pid
  (`ptrace_tracer.rs:511`).

`capture_runner` usa `follow_children: true` (líneas 196, 214, 343) y los tests
de `ptrace_tracer` usan `false`. Pero `cargo test` corre los tests en **hilos
paralelos dentro del mismo proceso**, así que el `waitpid(-1)` de un test se
comía el estado de salida del `/bin/true` que otro estaba trazando. El
afectado nunca veía su evento `Exited`: o fallaba el `assert!(got_exit)` o se
quedaba bloqueado en `do_wait` para siempre.

**El gate lo ocultaba**: `test-workspace-lib` corre
`cargo test --workspace --lib -- --test-threads=1`. En serie no hay colisión.
`ci.yml` hace lo mismo. Nadie ejecutaba el crate en paralelo.

### Por qué NO se tocó producción

Porque el código de producción es correcto. `ChronosServer` guarda **una sola**
sesión activa (`active_session: Arc<Mutex<Option<String>>>`, `server.rs:209`), así
que hay un solo tracer siguiendo un solo árbol de procesos, y `waitpid(-1, __WALL)`
es la forma correcta de ver todos sus hilos y clones. El defecto era de
**aislamiento entre tests**, no del producto. Poner un lock en producción
habría escondido la restricción real: este crate soporta una sesión de traza
activa por proceso.

### El arreglo

`crates/chronos-native/src/test_support.rs` (nuevo, solo `#[cfg(test])`) expone
`TRACE_TEST_LOCK`, y los **5** tests que trazan hijos lo toman durante todo su
cuerpo. El lock tolera veneno: un panic dentro de un test no puede envenenar el
mutex y hacer fallar a los siguientes con un error engañoso.

Resultado: **`cargo test -p chronos-native --lib` pasa en 11,23 s** en paralelo,
donde antes colgaba indefinidamente. En serie, 13,15 s. 111 tests, 0 fallos.

### El binario de integración tenía el mismo defecto, y otro más grave (2026-10-01)

`cargo test -p chronos-native --test m2_function_frame_capture` fallaba **5 de
7** tests en paralelo y pasaba 7/7 con `--test-threads=1`. El default local de
la suite estaba rojo en un checkout limpio.

Misma causa raíz: el binario de integración enlaza la lib **sin** `cfg(test)`,
así que no incluye `test_support::TRACE_TEST_LOCK`. Y `start_probe` —el camino
de `live_probe_emits_real_function_entries_to_execution_log`— cae en
`PtraceConfig::default()`, luego en la rama `waitpid(-1, __WALL)`.

Caracterización, en orden:

| Ejecución | Resultado |
|---|---|
| paralelo, 7 tests | 2 pasan, **5 fallan** |
| `--test-threads=1` | 7 pasan |
| `--skip live_probe`, paralelo | 6 pasan ← aísla el disparador |
| `live_probe` solo | 1 pasa |
| `live_probe` + `spawn_capture`, paralelo | 1 pasa, 1 falla ← **mutua** |

Los dos últimos casos prueban que la interferencia es simétrica: `live_probe`
rompe y se deja romper. Los síntomas observados eran el fixture del par
reportado como `Signaled { signal: 9 }` — `stop_probe` manda SIGKILL al pid que
cree suyo (`probe_backend.rs:701-709`) — y "no identity-bearing FunctionEntry
captured".

El arreglo replica `test_support.rs` en el binario de integración, con un lock
propio: `cfg(test)` no está activo cuando un test de integración enlaza la lib,
así que el estático de la lib no existe ahí. **Dos locks, una restricción
documentada**; cada uno referencia al otro. Producción intacta.

**`--test-threads=1` no se quitó de `.pipeline.kts` ni de `ci.yml`.** Sigue siendo
correcto. Lo que cambia es que el binario ya no depende de él.

### Un test verde que no probaba nada

`pie_fixture_compute_load_bias_is_nonzero` es el **único** coverage de que
`Int3Injector::compute_load_bias` devuelve una base no nula y alineada a página
para un binario `ET_DYN` bajo ASLR, es decir, de que el tracer **relocaliza
símbolos**. Nunca había ejecutado una aserción.

`pie_fixture_source()` subía **un** nivel desde `CARGO_MANIFEST_DIR`, así que
resolvía `crates/chronos-sandbox/programs/c/test_function_frames_pie.c` cuando
el source vive en `chronos-sandbox/programs/c/...`, en la raíz del workspace.
`src.exists()` era siempre `false`, `compile_pie_fixture()` devolvía `None`
siempre, y el test retornaba temprano reportando `ok`. El skip es un
`eprintln` informativo, así que nada fallaba de forma visible.

El doc comment de la función ya describía el recorrido correcto de dos niveles;
solo el código discrepaba de él. Corregido en `7ae5f7af`; el lock en `c7d70066`.

### La regla que deja esto

`--test-threads=1` en el gate **no es una garantía, es una muleta**: oculta
flakiness en vez de detectarla. Un test que lanza o traza hijos debe tomar el
lock de su binario aunque hoy la serie lo tape. Y un skip que devuelve `None`
es un verde vacío: si el requirement depende de encontrar algo, que su ausencia
falle o se vea en el recuento (`#[ignore]`), no que se imprima y se pase.

## `variables_in_scope` nunca había devuelto una variable (2026-10-01)

Búsqueda de tests sin aserción, en `chronos-native/src/dwarf/variables.rs`. De
los 5 tests del módulo, 2 tenían el cuerpo **solo con comentarios** y 1 terminaba
en `assert!(result.is_empty() || !result.is_empty())` — una tautología, cierta
para cualquier entrada. Los otros 2 no llegaban lejos.

Detrás de la aserción inútil había **dos defectos de producción**.

### 1. `DW_AT_high_pc` se leía solo en su forma DWARF 5

`is_pc_in_function` matcheaba `AttributeValue::Addr` y `Data8`, y todo lo demás
caía en `false`. Pero `DW_AT_high_pc` tiene dos codificaciones, y gcc/clang
emiten la de **DWARF ≤ 4**: una **longitud relativa a `low_pc`**
(`DW_FORM_data1/2/4/8`), que gimli normaliza a `Udata`. El match no la veía.

El fallo es silencioso y total: una dirección absoluta y una longitud son
ambas enteros pequeños sin signo, así que leer la longitud como dirección da
`pc < 160`, falso para casi todo `pc`. Medido sobre el fixture antes del fix:

| low | high | rango real | ¿contiene `pc=0x116c`? |
|---|---|---|---|
| `Addr(4457)` | `Udata(160)` | `[0x1169, 0x1209)` | **sí** |

`simple_function`. La función decía que no, y devolvía vacío para **toda
dirección de toda compilación**.

### 2. `DW_FORM_strp` no se leía, así que los nombres eran `"unknown"`

`get_string_attr_value` aceptaba solo `AttributeValue::String` (inline). Los
toolchains reales emiten `DW_FORM_strp`, un offset a `.debug_str`. Con el
arreglo 1 aplicado, el mismo fixture daba 11 variables, **10 de ellas
llamadas `"unknown"`** — porque los llamantes sustituyen por ese literal.

### Resultado

`variables_in_scope(0x116c)` pasa de **0** a **11** variables con nombre
(`param1` … `loop_var`), y el límite semiabierto es exacto: `0x1208` sigue
siendo `simple_function` y `0x1209` ya es `no_params_function`.

Commits `00cdcab9` (rango) y `e29a4396` (nombres + tests).

### Nota: la capacidad sigue sin consumidor

`DwarfReader::variables_in_scope` **no lo llama nada** fuera de su propio
módulo en todo el workspace. El fix no puede romper aguas abajo, y a la vez
dice que la capacidad está expuesta y desconectada. No se ha tocado: decidir
si se conecta al hot-path de captura es producto, no calidad.

### La regla: una tautología no es cobertura

`assert!(x.is_empty() || !x.is_empty())` se **lee** como cobertura. Oculta que
la función devolvía siempre vacío durante meses. Una batería donde todo pasa
no distingue nada: al revertir solo el arreglo 1, los 3 tests dependientes
fallan y el de degradación pasa — que es lo correcto, porque el código viejo
siempre devolvía vacío. Un test que no puede fallar no es un test, y uno que
falla siempre por lo mismo que sus vecinos tampoco informa.

## Dos UATs queepasaban sin ejecutarse (2026-10-01)

Del barrido de tests sin aserción. Los dos casos de gravedad 3, y los dos
vienen del mismo molde: **`let _ =` sobre todo lo que importa**.

### `m1_08_auto_compaction_daemon_runs_in_process_impl`

`chronos-sandbox/tests/m1_acceptance.rs`. Cero aserciones. Si
`McpTestClient::start()` fallaba, `eprintln!` + `return` → verde. Las dos
llamadas a tools iban a `let _ =`. El `shutdown()` también.

Es decir: **un servidor que no arrancó y uno que respondió bien eran
indistinguibles.** Y el test se llama `..._daemon_runs_in_process`.

El detalle que lo hace instructive: su propio doc ya declaraba el contrato
que incumplía — *"just makes sure the binary launches … and shuts down
cleanly with the daemon attached"*. El arreglo fue hacer que el cuerpo cumpla
el doc, no reescribir el doc. Ahora afirma: `start()` tiene éxito, ambos tools
rechazan la sesión inexistente nombrándola (medido:
`RpcError("Live probe session 'no-such-session' not found.")`), y `shutdown()`
devuelve `Ok`.

El doc además afirmaba que el binario arranca *"with the env var set"* y el
test **no ponía ninguna env var**. El daemon lee
`CHRONOS_AUTO_COMPACT_INTERVAL_SECS` al arrancar y `0` lo desactiva; el
default 30s ya lo habilita. El doc ahora dice eso. El test deliberadamente
**no** llama `set_var`: muta estado de proceso y competiría con los otros
tests del binario.

Y sigue sin probar que una ronda de compactación ocurra —una ronda solo
recorre `live_probes`, y una sesión que nunca se inició no está ahí—, pero el
doc lo dice de forma clara en vez de insinuarlo.

### `cih_g_uat_c2_01_diagnostic_first_event_timing`

`chronos-sandbox/tests/rec_c2_2_uat_c2.rs`. Imprimía sus números y `return`.
Los dos resultados que existe para distinguir —el evento llegó, o nunca llegó
— daban `ok`.

La firma del segundo está documentada en ese mismo fichero, a 500 líneas:
4 subidas de deadline (10s→30s→60s→300s), todas expirando a ratio ~1.0x con
`total_buffered=0`.

Su gemelo, **UAT-C2-01, ya decidía ese caso bien**: relee el wire, saca
`session_live` de `status` y `pipeline_silent` de `total_buffered`, y se los
pasa a `verdict_is_unobservable` — helper que existe para esto y que
`unobservable_verdict_requires_a_live_but_silent_pipeline` testea en el mismo
fichero. **Solo el gemelo diagnóstico nunca la llamó.** El arreglo reutiliza
esa política en vez de inventar una segunda, para que los dos no puedan
divergir sobre qué significa "no observable".

Medido en este host, **las dos ramas son reales**: ejecutado solo, el
diagnóstico vio `first_event_after_ms=11 count=1`; ejecutado dentro del binario
completo, bajo la carga del UAT previo, C2-01 se pasó los 300 s con el fixture
corriendo. Es exactamente la condición que el veredicto clasifica.

### La regla

Un UAT que no puede fallar no es un UAT, es un `println!` con presupuesto. Y
cuando la política ya existe a 500 líneas de distancia —un helper, una función de
veredicto, un precedente— **reutilízala**: duplicar la política es cómo dos
tests empiezan a discrepar sobre el mismo caso. `UNDER_TARPAULIN` se elevó a
nivel de módulo por exactamente eso: la necesitaban dos tests y una copia por
test dejaría que el contractual y el diagnóstico no coincidieran.

## Catorce tests del sandbox que solo imprimían (2026-10-01)

### El patrón

Cuatro ficheros de `chronos-sandbox/tests/` compartían la misma forma de pasar en
verde sin probar nada:

```rust
match result {
    Ok(v)  => { println!("…: {} cambios", v.changes.len()); }
    Err(e) => { println!("… returned error (also acceptable): {:?}", e); }
}
```

El nombre del test promete una propiedad —«sin carreras», «sin cambios», «error»—
y el cuerpo acepta **ambos** brazos del `match`, así que el resultado es el mismo
haga lo que haga el servidor. Se añadieron 14 asserts reales en
`diff_tools.rs`, `memory_tools.rs`, `boundary_conditions.rs` y
`error_handling.rs`.

### Todos los ceros tenían la misma causa

Antes de escribir un solo assert se midió el comportamiento real, y los resultados
explicaron por sí solos por qué todos esos tests devolvían vacío: **la captura de
los fixtures en C no produce evidencia de registros ni de escrituras a memoria**.
Volcando los eventos de `test_add`, los 128 eventos son `kind=Unresolved`, con
descriptores `SyscallEnter`/`SyscallExit`. Nada más.

De ahí se sigue que `state_diff`, `inspect_causality` y `debug_detect_races` son
vacías **por construcción**, y que afirmar «vacío» es honesto siempre que el
comentario diga por qué. `test_debug_detect_races_threads` mereció un comentario
nuevo tras leer `test_threads.c`: cada worker solo escribe su `sum` local de
pila, así que cero carreras es un hecho del fixture, no una moneda al aire.

### Un comentario corregido por la medición

`test_probe_start_program_crashes_sigsegv` decía que el crash «podía no detectarse».
No: `debug_find_crash` devuelve `Some`, con `signal == "SIGSEGV"` y `event_id`
concreto. El test ahora lo exige, porque detectar el crash es su propósito.

### El acoplamiento a literales que sí se acepta

Cuatro asserts comprueban subcadenas del mensaje de error del servidor:
`"ExecutionLog unavailable"`, `"Invalid program path"`, `"Program not found"`,
`"Non-absolute path rejected"`. Antes, esos tests admitían cualquier resultado.
Es un intercambio deliberado: el mensaje es parte observable del contrato de
error, y se comprueban **subcadenas estables**, no el mensaje completo, para que
reformular la redacción no rompa el test. Un test de error que no mira el error
no prueba que el error sea el correcto.

### `debug_diff` estaba roto siempre, y no por la migración que lo tocó

`McpTestClient::debug_diff` (`chronos-sandbox/src/client/tools.rs`) manda
`state_query` con `kind: "state_diff"`, y ese variant **no existe** en
`StateQueryKind` (`crates/chronos-services/src/output.rs:1164`, que tiene
`register_diff`, `memory_read`, `register_snapshot`, `memory_analysis`,
`expression_eval` y `variable_snapshot`). El servidor responde siempre
`-32602 "unknown variant 'state_diff'"`.

Tres cosas que la investigation descartó:

- **No es culpa de la migración C5.3.1.** El doc de `m5-agent-api-v2-scoping.md`
  que fusiona `debug_diff` en `session_compare` es un *scoping proposal* que él
  mismo declara «no production code changes»; nunca se aplicó. Y aunque el
  rollback literal se hiciera, tampoco deserializaría: el handler del servidor
  emite forma `StateDiffSnapshot` (`event_id_a`, `variables_added`,
  `registers_changed`), mientras que `DebugDiffResponse` espera `event_a_id`,
  `registers_diff` y `summary`, sin un solo `#[serde(default)]`. **Esa deriva es
  anterior a la migración.**
- **`state_diff` sí funciona** y usa `kind=register_diff`; los dos viven en el
  mismo fichero, a 480 líneas de distancia.
- **Borrarlo no es opción**: `session_compare` compara sesión contra sesión, no
  acepta eventos.

Queda abierto con su consumidor (dos tests de `diff_tools.rs` y uno de
`state_depth.rs` aceptan hoy ambos brazos por esta causa). Además, y adyacente:
`"state_diff"` sigue listado en seis listas por perfil
(`crates/chronos-mcp/src/tools_params.rs:48,78,120,164,208,252`) pese a que su
handler fue borrado como alias, de modo que `capabilities` anuncia tools que no
existen.

### La regla

**Mide antes de afirmar, y afirma el brazo que corresponde.** Cuando un test
acepta `Ok` y `Err`, la primera pregunta no es qué debería pasar: es qué pasa de
verdad. Aquí la respuesta fue la misma en los catorce casos, y no era ninguna de
las dos que el test toleraba.

## `debug_diff`: una tool que fallaba siempre, y dos formas que no se hablaban (2026-10-01)

### El síntoma

`McpTestClient::debug_diff` devolvía error en el 100 % de las llamadas, con
`-32602 "unknown variant 'state_diff'"`. Sus tres consumidores —dos en
`diff_tools.rs` y uno en `state_depth.rs`— aceptaban ambos brazos del `match`, así
que el workspace estaba verde mientras la tool estaba muerta.

### Dos defectos, no uno

**El transporte.** El método mandaba `state_query` con `kind: "state_diff"`, y
ese variant no ha existido nunca en `StateQueryKind`
(`crates/chronos-services/src/output.rs:1164`, que expone `register_diff`,
`memory_read`, `register_snapshot`, `memory_analysis`, `expression_eval` y
`variable_snapshot`).

**La forma, que es anterior.** `DebugDiffResponse` pedía `session_id`,
`event_a_id`, `event_b_id`, `registers_diff`, `memory_diff` y `summary`. El
handler del servidor (`crates/chronos-mcp/src/server.rs:1684-1731`) emite otra
cosa: `StateDiffSnapshot`, con `event_id_a`, `event_id_b`, `variables_added`,
`variables_removed`, `variables_changed`, `registers_changed` y
`timestamp_delta_ns`.

`git log -S DebugDiffResponse` lo resuelve: **un solo commit**, `1e4f842b` (el
framework e2e del sandbox). En ese commit el cliente llamaba a la tool correcta y
el servidor ya emitía `StateDiffSnapshot`. **La forma del cliente era inventada y
nunca tuvo contraparte**, así que el test no podía haber pasado ni en el día en
que se escribió. C5.3.1 (`73431af2`) migró el transporte y dejó la forma igual:
convirtió un fallo de serde en un variante inexistente. Ninguna de las dos
migraciones lo detectó, porque los tests aceptaban ambos brazos.

### Por qué se arregló el cliente y no el servidor

`StateDiffSnapshot` es la forma real, está cubierta por
`state_diff_snapshot_roundtrips` (`output.rs:2424`) y no ha cambiado desde
`1e4f842b`. Adaptarla a la forma inventada habría exigido **inventar datos**:
`DebugReadService::diff` no toca memoria —`memory_diff` no tendría origen—,
`summary` no tiene fuente, y `registers_changed` es un mapa de cadenas hex
(`"0x7ffd..."`), no pares `(u64, u64)`. Alinearse con el servidor modela un
contrato existente y verificado.

Las dos salidas alternativas se descartaron con el código, no con criterio:
`register_diff` (`tools.rs:957`) compara entre *timestamps*, así que usarlo
obligaría a resolver `event_id → timestamp` en el cliente y perdería la
comparación de variables; y `session_compare` (`server.rs:3090-3093`) es
sesión contra sesión, no acepta eventos.

El doc de `docs/milestones/m5-agent-api-v2-scoping.md:59`, que mapea
`debug_diff → session_compare`, es un *scoping proposal* que él mismo declara
«no production code changes». Nunca fue un decreto, y la fila nunca se aplicó.

### El tipo se espeja, no se importa

`chronos-sandbox` depende de `chronos-domain`, `chronos-capture` y
`chronos-query`, **no** de `chronos-services`. Por eso el cliente se había
inventado sus propios DTO. Se añadió `StateDiffSnapshot` local en
`client/types.rs` con una nota de sincronización, en vez de añadir una
dependencia de peso al sandbox por un solo tipo de datos.

### El toolset se pinea, por el mismo motivo que la base de datos

`debug_diff` está en `ALL_TOOL_NAMES` y aparece en `tools/list`, pero **no está en
ninguno de los siete perfiles por toolset**. Con un perfil explícito, el guard lo
rechaza. `McpProcess::spawn_with_env` ya eliminaba `CHRONOS_DB_PATH` del ambiente
por un motivo idéntico —que un desarrollador con la variable exportada no redirija
el servidor sandbox en su almacén real—, así que ahora hace lo mismo con
`CHRONOS_ACTIVE_TOOLSET` y fija `auto`, que es el default del servidor. Sin eso,
cualquier assert sobre `Ok` fallaría por una variable del entorno del operador y
no por el código.

### La regla

Cuando el cliente y el servidor llevan años hablando en dos dialectos, el test
que acepta ambos brazos no es un test débil: es el que está **impidiendo** que se
vea. Arreglar el transporte sin mirar la forma solo habría movido el fallo. Y
antes de «arreglar» una capa, comprueba con `git log -S` de dónde salió la forma
que no funciona: casi siempre la respuesta es que nunca funcionó.

## Un helper del sandbox que siempre devolvía `-32601` (2026-10-01)

### El hallazgo

Al convertir los tests de profundidad apareció uno que no fallaba por la razón
que parecía. `test_evaluate_expression_simple_arithmetic` llamaba a
`state_query` mediante `call_with_timeout` y aceptaba `Ok` **y** `Err`. Al
medirlo: `Err(-32601 "method not found")`.

La causa no estaba en el test ni en la tool. `call_with_timeout` envía un
**método JSON-RPC crudo**, y desde C5.3.2 el servidor solo implementa
`tools/call`. Ese helper no puede funcionar nunca para ninguna tool.

Un segundo agente, trabajando en otro par de ficheros y sin saber nada del
anterior, encontró lo mismo: los dos tests de umbral de `race_depth` invocaban
`execution_query` por ese camino y llevaban meses midiendo `-32601`. **Dos
investigaciones independientes, mismo defecto.** Eso es la señal de que la causa
es compartida y no de cada test.

### Un caso peor: la ruta JSON equivocada

`events_read` con `mode = query` anida la página en `result.events`. El test lo
leía en el nivel superior con `v.get("events")` y reportaba «0 events» sobre una
página llena de cinco. No fallaba, no estaba verde por tolerancia: estaba
**leyendo otra cosa** y le daba un nombre que sonaba razonable.

Un assert sobre la clave equivocada es peor que no tener assert, porque además
de no probar nada parece que sí.

### La regla

Un `-32601` o un «0 eventos» en un test tolerante no es un detalle del fixture:
es una **ruta de llamada que dejó de existir**. Y cuando varios tests, en
ficheros distintos y para tools distintas, devuelven el mismo código de error,
sospecha del helper compartido antes que de cada test.

## Dieciséis tests de profundidad del sandbox, y lo que no se puede afirmar (2026-10-01)

Seis ficheros, dieciséis tests convertidos de «imprimir» o «aceptar ambos
brazos» a aserciones medidas. Todos se invirtieron y se vieron fallar antes de
revertir; las seis suites quedaron verdes (43 tests).

### El límite que hay que escribir en el propio test

`test_detect_races_threads` ahora exige cero carreras. Es un assert válido, pero
solo con dos datos verificados de forma independiente:

1. `test_many_threads.c` no tiene carrera por construcción: cada worker acumula
   en su `volatile long sum` **de pila propio** y solo lee `ids[i]`, que `main`
   escribe antes de crear los hilos.
2. Aun así, `total_writes` y `access_count` son 0 sobre 374–376 eventos: la
   captura ptrace de un fixture en C no emite escrituras a memoria, así que el
   detector no tiene direcciones que emparejar.

El segundo punto significa que **el verde no demuestra que el detector supiera
encontrar una carrera si existiera**. Eso está escrito en el doc-comment, porque
un test que aparenta más de lo que prueba es una trampa para el que lo lea seis
meses después.

En `concurrency_stress` se descartó todo lo que dependiera del reloj:
`duration_ms` y `elapsed` se midieron en 6–10 ms en un host compartido,
`ebpf_detached` refleja privilegios y kernel, y los recuentos absolutos variaron
entre 128 y 129 entre tests. Se afirmó lo estructural: identificadores distintos
por ciclo, ids ascendentes y sin duplicar dentro de cada página, y las 20
llamadas mezcladas terminando todas en `Ok`.

### Guardas anti-vacuidad

Varios asserts «todo vacío» se acompañaron de una guarda que demuestra que la
sesión no estaba muerta: `total_events >= 100`, o un control `limit = 5` junto al
`limit = 0` que se está afirmando. **Cero porque no hay nada** y **cero porque
no se capturó nada** producen el mismo test verde; la guarda es lo que los
distingue.

Un caso límite que se dejó anotado sin tocar: `CS7 probes_different_durations`
ya afirmaba `drain3.len() >= drain1.len()`, que sí depende de temporización.
Queda fuera de esta tanda y marcada como riesgo latente.

### Un nombre que ya no describe lo que hace

`ED2 debug_call_graph_has_edges` ahora afirma que **no hay** aristas, porque la
captura en C no produce grafo de llamadas. Se conserva el nombre porque
renombrarlo rompería referencias de UAT y receipts; la contradicción queda
documentada en el doc-comment. Renombrar es lo correcto en abstracto, y lo
incorrecto aquí.

### La regla

Cuando afirmes un vacío, afirma también **por qué está vacío** y **que no era
porque nada funcionara**. Y cuando descartes un assert por ser frágil,
escríbelo: un assert que se descartó con el motivo escrito se puede reevaluar
después; uno que se descartó en silencio se vuelve a proponer tres meses más
tarde como si fuera nuevo.

## Dos servicios que respondían igual a cosas distintas (2026-10-01)

### «No hay variables» y «no existe ese evento»

`DebugReadService::get_variables` pasaba el `event_id` al motor sin comprobar
existencia, así que un id equivocado respondía `Ok([])`, exactamente igual que
un evento real sin datos de marco. Quien leyera una lista vacía no tenía forma
de distinguir una ausencia verdadera de un error de tipeo — y un agente que
construye su hipótesis forense sobre esa lista no puede saber cuál de las dos
está mirando.

La asimetría de la que había que darse cuenta: **`register_snapshot` ya
distinguía los dos casos**, con `EventNotFound` frente a `NoRegisterState`. Dos
variantes del mismo enum, en el mismo fichero, respondiendo distinto ante la
misma pregunta. El precedente estaba a treinta líneas.

Antes de escribir el arreglo se comprobó lo que podía hacerlo falso:
`get_variables_at_event` (`crates/chronos-query/src/engine.rs:526-540`) es una
**búsqueda por igualdad exacta** con `binary_search_by_key`, no una ventana. Un
id nunca capturado no cae legítimamente en ningún rango. El caso que sí tiene
semántica de rango es `get_memory_at`, «at or before timestamp», y quedó
intacto. Sin esa comprobación, «no encontrado» y «sin frame data» se habrían
mezclado justo al separarlas.

La validación se puso en el servicio y no en el brazo del `match` porque hay dos
call sites de producción —`state_query` y la tool v1 `debug_get_variables`— y
arreglar solo uno deja dos comportamientos para el mismo concepto.

### Un tripwire que no puede dispararse

`tripwire_create` aceptaba `EventType([])`. El tripwire se creaba, se listaba,
se contaba, y ningún evento podía satisfacerlo jamás. El llamante pagaba la
suscripción y recibía silencio.

El alcance del rechazo se acotó a propósito a las condiciones **insatisfacibles
por su propia estructura**: conjuntos vacíos contra los que probar pertenencia
(`EventType`, `SyscallNumber`, `Signal`) y rangos invertidos (`MemoryAddress`).
Tres casos vecinos quedaron fuera, y el motivo importa:

- `FunctionName { pattern: "" }` solo casa con un nombre de función vacío
  (`glob_inner` devuelve `ti == t.len()` con el patrón agotado). Es
  insatisfacible en la práctica, pero probarlo sería una afirmación sobre **todos
  los adaptadores**, no sobre este tipo.
- `VariableName { name: "" }` tiene el mismo argumento.
- `ExceptionType { exc_type: "" }` es el fallo **contrario**: `contains("")` es
  cierto para cualquier cadena, así que casa con *todos* los eventos de
  excepción en lugar de con ninguno. Rechazarlo aquí convertiría en silencio una
  configuración ruidosa y equivocada en una rechazada, que es otra decisión.

**Un error con dos causas necesita dos mensajes.** La primera versión devolvió
`ServiceError::InvalidCondition`, que ya existía para «este `event_type` no lo
reconozco». El test falló con:

```
observe: unknown event_type 'event_types is empty, so no event could ever satisfy it'
```

La capa MCP renderiza ambas causas con la misma etiqueta, así que rechazar una
lista vacía le decía al llamante que su tipo de evento era desconocido. Pasó a
`ServiceError::UnsatisfiableCondition`, y un test de servicio fija que el
mensaje **no** vuelva a contener `unknown event_type`. Añadir la variante
rompió un `match` exhaustivo en `server.rs:1118`, que el compilador cazó.

### La regla

Antes de unificar dos errores porque «son el mismo enum», comprobar si el
llamante puede distinguirlos. Y cuando el rechazo de un caso sea correcto pero
su alcance no esté claro, escribir **por qué los otros quedan fuera**: un
límite justificado se puede revisar; uno arbitrario se vuelve a ampliar sin
querer dentro de seis meses.
