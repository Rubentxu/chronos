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

`pipelinek` v0.39.0 — instalable desde `pipelinek-0.39.0.zip` (build local:
`v2/pipeline-application/build/install/pipelinek/bin/pipelinek`). Comando
canónico desde la raíz del proyecto:

```bash
pipelinek run --db .pipelinek/db.sqlite \
              --control-root .pipelinek/control \
              .pipeline.kts
```

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
* Usar **rutas absolutas** dentro de los `sh(...)` (el motor v0.39.0 no
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
