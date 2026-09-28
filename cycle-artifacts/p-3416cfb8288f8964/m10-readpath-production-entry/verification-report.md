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

### Investigación posterior: 2 de los 5 son un defecto de la herramienta

Los errores SDDK009 y SDDK010 (docs ausentes o stale) sí tienen una vía
legítima: `sddk generate docs` y `sddk generate inventory`. **La vía está rota
en SDDK 2.0.1.** Ambos comandos imprimen

```
wrote docs/generated/workflow.md
wrote docs/generated/inventory.md
```

y terminan con exit 0, pero **no crean ningún fichero**. Verificado por
`find . -name workflow.md -newermt "-5 minutes"` (vacío), `ls docs/generated/`
(No existe el directorio) y `git status --porcelain` (sin entradas nuevas).

Es un no-op silencioso que reporta éxito: el peor modo de fallo posible en una
herramienta de gate, porque un agente que confíe en el exit code daría el
bloqueador por resuelto. Ningún template ni implementación de referencia
existe en el framework instalado que permita reconstruir la salida esperada.

**Estado de los 5 errores de lint:**

| error | vía de resolución | estado |
|---|---|---|
| SDDK009 `docs/generated/workflow.md` | `sddk generate docs` | **bloqueado: la herramienta no escribe** |
| SDDK010 `docs/generated/inventory.md` | `sddk generate inventory` | **bloqueado: la herramienta no escribe** |
| SDDK005 `schemas/` | decisión del proyecto | requiere decisión |
| SDDK011 `permissions.yaml` | decisión del proyecto | requiere decisión |
| SDDK014 `manifest.toml` | decisión del proyecto | requiere decisión |

Registrado en el backlog SDDK como
`bl-bl-01M3KDHHEF0003876V3S8XMV40`.

**Impacto en el cierre de la iniciativa:** este blocker impide declarar la
iniciativa `COMPLETED`, pero no invalida el WorkItem verificado.

## Estado

- Implementado: sí (`896c2f17`, `c45488c4`)
- Verificado localmente: sí (este informe)
- Integrado en remoto: **no** — los commits verificados siguen sin push
- Certificado: no
