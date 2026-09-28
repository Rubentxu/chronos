# verification-report — M10 read path production entry

- cycle: `p-3416cfb8288f8964/m10-readpath-production-entry`
- phase: Verify
- subject: `c45488c46be5b8c5121134258fa1f752aa869f8b`
- gate receipts: `gate-tests-pass-ea458bff5bd88ae2-1`,
  `gate-policy-compliant-ea458bff5bd88ae2-1`
- implementation receipt: `implementation-receipt.md` (adjunto al ciclo)

## Veredicto

El WorkItem **está verificado localmente**. El comportamiento que introduce
cumple los invariantes que el ciclo declara, y esa conclusión se apoya en
evidencia ejecutada, no en recuento documental.

Lo que **no** está verificado, y no se declara: `sddk lint` sigue fallando con
5 errores. Ver "Blocker 1" más abajo.

## Comprobaciones ejecutadas (OBSERVED)

| id | comando | exit | resultado | digest |
|---|---|---|---|---|
| fmt | `cargo fmt --all -- --check` | 0 | limpio | `e3b0c442…b855` (cadena vacía) |
| clippy | `cargo clippy -p chronos-mcp --all-targets` | 0 | 0 warnings | `b4915759…6350` |
| mcp tests | `cargo test -p chronos-mcp --lib` | 0 | **87 passed, 0 failed** | `da17660e…04fe` |
| svc tests | `cargo test -p chronos-services --lib` | 0 | **539 passed, 0 failed** | `0a207f4d…e031c` |
| CI canónico | `pipelinek run … .pipeline.kts` | 0 | `Pipeline finished with SUCCESS` | journal `RunFinished` outcome=success, `StepFailed`=0 |

`.pipeline.kts` SHA-256: `ec1d4f77db5025addcf9b349112f0de560379898448724137753d02c33cc0d62`

## Qué se verificó del comportamiento, no solo del código

1. **Fail-closed con prueba negativa.** `execution_log_read` en modo `poll`
   sobre una sesión sin log devuelve `is_error: true` y **no** emite la forma
   `event_count: 0`. Un mutation test (sustituir la rama de error por un éxito
   vacío) rompe el test, así que la guarda está realmente fijada.
2. **Cursor ajeno rechazado.** Un cursor acuñado para `session-a` presentado
   contra `session-b` falla en vez de reanclar silenciosamente en el log
   equivocado.
3. **Causality responde sin engine.** Con log legible y sin engine registrado,
   el modo `causality` responde `Unsupported` con `engine_loaded: false`, sin
   ser error. Esto es distinguible del caso 1, donde no hay log.
4. **Descubribilidad.** `tool_availability_map_covers_every_registered_tool`
   deriva el tamaño del router vivo y comprueba cobertura nombre por nombre:
   un tool registrado pero no descubrible ahora rompe el test.
5. **Una sola autoridad.** Ambos constructores de `ChronosServer` enlazan el
   mismo `Arc<SessionExecutionLogRegistry>`; leído en el código y afirmado en el
   comentario de construcción.

## Límites de esta verificación

- **No** es UAT. No hay agente real consumiendo el tool por MCP sobre una
  sesión real; las pruebas invocan el handler Rust directamente.
- **No** es certificación. El perfil de certificación del proyecto exige
  escenario de extremo a extremo; esto no lo cubre.
- Las pruebas usan `SessionExecutionLog::create_for_tests` sobre un log real en
  disco temporal, no un proveedor real de probes.

## Blocker 1 — `sddk lint` (abierto, fuera de este WorkItem)

```
SDDK005 schemas/            no existe
SDDK009 docs/generated/workflow.md   ausente o stale
SDDK010 docs/generated/inventory.md  ausente o stale
SDDK011 permissions.yaml    no existe
SDDK014 manifest.toml       no existe
lint: 5 error(s), 0 warning(s)
```

**No es una regresión de este ciclo.** `git log -- schemas permissions.yaml
manifest.toml` está vacío: esos ficheros nunca existieron en la historia del
repositorio, mucho antes de `98257c9f` (Phase 1 MVP). El proyecto está
*adoptado* (`sddk adopt status: complete`, con receipt e identidad), pero
adoptar identidad yDeclarar los contratos del repositorio son cosas distintas.

No he creado `schemas/`, `permissions.yaml` ni `manifest.toml` a mano para poner
verde el lint: inventar contratos que el proyecto no ha decidido para
satisfacer un linter sería fabricar evidencia, justo lo contrario de lo que este
informe pretende.

### Investigación posterior: un bucle, y dos correcciones a mi diagnóstico

**Corrección 1.** Mi hipótesis inicial fue que `sddk generate docs` e
`inventory` eran un no-op defectuoso: imprimen `wrote …` y salen con exit 0 sin
crear nada. **Es falso.** La generación funciona; por defecto el output va a
`~/.local/share/sddk/projects/p-3416cfb8288f8964/generated/docs/generated/`
(XDG), y `sddk generate docs --root . --check` responde
`docs/generated/workflow.md is current`.

**Corrección 2.** Escribí que `--in-repo` era un flag oculto ausente del help y
que **ambos** generadores estaban bloqueados. **También es falso.**
`sddk generate docs --help` sí documenta `--root`, `--check` e `--in-repo`
("Write into the repo (docs/generated/) instead of XDG — dogfooding only"), y
`generate inventory --in-repo` **funciona**: escribe el fichero en el repo sin
necesidad de `workflow/workflow.yaml`.

**Resultado real: SDDK010 resuelto, 5 errores → 4.** El fichero generado es un
informe exacto del estado actual del repositorio (0 agentes, 0 skills
registrados), no un stub.

**Lo que queda bloqueado es un bucle de dependencias real**, sobre
`workflow/workflow.yaml` y `schemas/`:

1. `sddk generate docs --in-repo` falla con
   `failed to load canonical workflow: failed to read workflow manifest "./workflow/workflow.yaml"`.
   Ese manifest no existe en el repositorio. (El framework trae cuatro
   definiciones de workflow en `prompts/sddk/workflows/`, pero son de capa
   prompt: no tienen `schema_version` ni la forma de objeto que el CLI exige.)
2. SDDK005 exige `schemas/` con seis schemas canónicos por nombre exacto:
   `adoption`, `agent-result`, `artifact-ref`, `cycle`, `phase-result`,
   `workflow`. El propio binario referencia
   `schemas/workflow.schema.json` como contrato para validar (1).
3. SDDK011 y SDDK014 exigen `permissions.yaml` y `manifest.toml`.

No existe scaffolding que rompa el bucle: `sddk pack scaffold` genera un pack,
no el manifest del repositorio, y no hay comando que emita `schemas/`.

**Estado final de los 4 errores restantes:**

| error | vía de resolución | estado |
|---|---|---|
| SDDK005 `schemas/` (6 schemas canónicos) | authoring manual del contrato | requiere decisión del proyecto |
| SDDK009 `docs/generated/workflow.md` | `sddk generate docs --in-repo` | bloqueado por el bucle: falta `workflow/workflow.yaml` |
| SDDK011 `permissions.yaml` | decisión del proyecto (mapa de `agents`) | requiere decisión |
| SDDK014 `manifest.toml` | sin scaffolding disponible | requiere decisión |

Backlog: `bl-bl-01M3KDNYW30003876VBVWD2G80` (corrección de
`bl-bl-01M3KDHHEF0003876V3S8XMV40`, descartado como `superseded` por su
diagnóstico erróneo; se conserva el rastro en vez de reescribir la historia).

**Impacto real en el cierre de la iniciativa.** El bucle NO bloquea la
verificación de integración: `grep -c sddk .pipeline.kts` devuelve **0**. El
CI canónico (`discover-repo`, `workspace-check`, `build-domain`, `evidence`) no
invoca `sddk lint` en ninguna etapa, y según AGENTS.md es la única fuente de
verdad para declarar el repositorio verificado. Los 4 errores de lint son
deuda de contrato del repositorio, no un fallo de la batería que gobierna el
push.

Lo que sí bloquea el cierre es `release-uat-approved`, que no es fabricable sin
UAT real (ver "Límites de esta verificación").

## Estado

- Implementado: sí (`896c2f17`, `c45488c4`)
- Verificado localmente: sí (este informe)
- Integrado en remoto: **sí** — push `488a6120..f94864ad`, `HEAD == origin/main`
  (merge-receipt adjunto al ciclo)
- Certificado: no

## Por qué el ciclo no puede cerrarse en AUTO

`release.complete` exige el gate `release-uat-approved`. La configuración del
proyecto (`sddk uat config show --project p-3416cfb8288f8964`, `uat.toml`
inexistente, defaults) declara:

```toml
[release_gate]
major = required
minor = required
patch = skip

[human]
developer = true
architect = true
```

Dos consecuencias:

1. Cualquier release `major` o `minor` exige UAT ejecutado de verdad. No es
   alcanzable con pruebas unitarias: requiere un release candidate
   (`sddk uat plan --release …`) y sesiones de agente reales consumiendo
   `execution_log_read` por MCP, ingeridas con `sddk uat ingest` y agregadas
   con `sddk uat report`.
2. El gate exige **dos firmas humanas** (developer y architect). Firmar en
   nombre de un humano que no ha revisado el trabajo sería falsificar una
   aprobación, que es exactamente lo que este informe rechaza en las demás
   secciones.

Por tanto el ciclo queda en `RELEASE_PENDING` con la implementación integrada y
verificada, y el cierre depende de una decisión humana que excede la autoridad
de este agente. No es un bloqueo que se pueda resolver investigando más.
