# Release receipt — v0.10.1

| Campo | Valor |
|---|---|
| Head SHA | 2969d4a2d4de15e8071bc579086b158eac78c436 |
| Base SHA | 57c36040f29ffea2df11335324037a35d393641c |
| Branch | main |
| Date | 2026-10-05 |
| Remote tag | v0.10.1 |
| Remote tag_peel | 2969d4a2d4de15e8071bc579086b158eac78c436 |
| Peel match | true |

## Anotación y procedencia

| Campo | Valor |
|---|---|
| Tag object (remoto) | e6990bb81e24506e4f20c7e2a2570eb37d1d0495 |
| Peel local | 2969d4a2d4de15e8071bc579086b158eac78c436 |
| Peel remoto | 2969d4a2d4de15e8071bc579086b158eac78c436 |
| `Cargo.toml` en el tag | 0.10.1 |
| Autoridad de versión | `cross_checked` (`sddk release plan`) |

El peel local y el remoto son el mismo objeto, comprobado con `git ls-remote` y `git rev-parse` por
separado en lugar de asumir que la creación del tagimplies el push.

## Cómo se publicó

`sddk release apply --tag v0.10.1` devolvió `converged: true`, `applied: 2`, con los receipts de
capacidad `cap-git-push-65ba40988e2e` y `cap-git-tag-2017178f82d9`. **Ningún paso manual de Git**: el
gateway pidió aprobación para la capacidad `git.push` y se resolvió con el `--approve` explícito que el
propio error indicaba, dentro de la autorización previa del operador para esta iniciativa. No se usó
`--no-verify` ni se bypasseó el hook.

## Qué contiene esta release

Cuatro defectos de veracidad del tracer, cerrados y verificados end-to-end, más el cierre de
`DEBT-WAIT-EVENT-UNBOUNDED-01` con su coste medido y su limitación residual declarada:

- el live probe se tragaba las señales del tracee y el programa seguía vivo tras `abort()`
- `find_crash` culpaba al programa del `SIGKILL` que chronos le manda en su propio teardown
- las salidas de syscall publicaban el valor de retorno como número de syscall
- el tick de sondeo estrangulaba la captura a 100 paradas por segundo

Y una limitación que **no** se cierra, registrada como `DEBT-CRASH-VERDICT-SOURCE-01`: los tests
end-to-end de crash no distinguen el arreglo de su ausencia, porque el veredicto se apoya en la entrega
de la señal y no en la muerte del tracee.
