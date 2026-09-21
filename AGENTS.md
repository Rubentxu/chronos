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

## 4. Política de resolución de bloqueos

Resolver el bloqueo es parte del trabajo del goal, no una razón para abandonar el ciclo.
No te detengas ante el primer error ni presentes inmediatamente el problema al operador.
Delega un diagnóstico reproducible y busca primero una solución compatible con la arquitectura y los contratos existentes.
Si no resulta suficiente, profundiza: investiga código fuente, documentación, historial, alternativas técnicas y efectos sobre los siguientes hitos. Utiliza especialistas complementarios o spikes cuando permitan reducir incertidumbre.
No elijas un parche únicamente porque supera un test. Prefiere soluciones que resuelvan la causa raíz, aporten valor real y eviten complejidad innecesaria.
No avances por la línea de trabajo dependiente hasta resolver y verificar su bloqueo. Si la investigación descubre tareas independientes que pueden progresar sin comprometerlo, puedes delegarlas en paralelo, pero mantén el bloqueo abierto y con responsable hasta su resolución.
Escala al operador únicamente cuando, después de investigar las alternativas viables, resulte imprescindible una autorización nueva que exceda esta iniciativa. Presenta entonces una decisión concreta, con evidencias y opciones, no una petición genérica de instrucciones.

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
* Durante la implementación, ejecuta pruebas ajustadas al cambio; utiliza la batería completa en los gates que la exijan.

No omitas verificaciones obligatorias para ahorrar llamadas.
Conserva resultados detallados, receipts y trazabilidad en SDDK. Comunica al operador únicamente avances significativos, hallazgos que cambien el plan, decisiones imprescindibles y el cierre de la iniciativa.
Los informes de progreso no deben convertirse en puntos de parada.

## 7. Bucle de ejecución continua

Mientras existan requisitos pendientes:

1. Recupera el estado vigente del goal y sus dependencias.
2. Selecciona el siguiente trabajo desbloqueado.
3. Delega su caracterización, implementación y verificación.
4. Investiga y resuelve cualquier bloqueo que aparezca.
5. Comprueba los criterios de aceptación y los gates obligatorios.
6. Registra evidencias, commits, receipts y estado de los WorkItems.
7. Continúa automáticamente con el siguiente trabajo.

No finalices la ejecución porque un subagente termine su tarea, se complete un ciclo o aparezca un problema técnico.
Si una sesión termina, conserva un checkpoint duradero y reanuda desde él cuando exista un mecanismo de ejecución disponible.

## 8. Criterio de finalización

 Declara la iniciativa `COMPLETED` únicamente cuando todas las capacidades adoptadas del roadmap estén implementadas, integradas y verificadas; los gates obligatorios estén satisfechos; los bloqueos estén resueltos; y el código, las evidencias, los WorkItems y el roadmap reflejen un estado coherente.
No contabilices pruebas ignoradas, resultados parciales ni gates bloqueados como satisfactorios.
Respeta los procedimientos legítimos de Git y publicación: no utilices bypasses, bumps ceremoniales ni sustituyas commits expresamente autorizados por otros sin resolver antes su procedencia y autorización.

## Orden de ejecución

Localiza el goal y la iniciativa existentes en JCode, activa SDDK en modo AUTO y dirige mediante subagentes especializados la ejecución completa del roadmap.
Ante cada bloqueo: investiga → determina la causa raíz → diseña la solución → implementa → verifica → satisface el gate → continúa.

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
