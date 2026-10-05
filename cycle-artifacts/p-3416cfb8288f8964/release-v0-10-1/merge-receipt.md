# Merge receipt — v0.10.1

| Campo | Valor |
|---|---|
| Head SHA | 2969d4a2d4de15e8071bc579086b158eac78c436 |
| Base SHA | 57c36040f29ffea2df11335324037a35d393641c |
| Branch | main |
| Date | 2026-10-05 |

## Qué se integró

Once commits sobre `main`, de `260eb5ce` a `2969d4a2`: siete `fix`, dos `docs`, un `test` y un
`chore(release)`. La versión se derivó del historial, no se eligió a mano: ni `feat` ni breaking en el
rango, luego PATCH bajo la convención 0.x.

`git diff v0.10.1..HEAD -- crates/ chronos-sandbox/ Cargo.toml Cargo.lock` sale **vacío**: el tag contiene
todo el producto, no una parte.

## Puertas antes de integrar

- `cargo fmt --all --check` limpio
- `cargo clippy --workspace --all-targets -- -D warnings` sin salida
- `vault-drift-sweep`: PASS (50 python CCs, 7 bash CCs)
- `validate_cycle_artifacts.py`: PASSED
- `regen_manifest_index_shas.py --check`: clean, 104 manifiestos
- hook `pre-push` ejecutado con fmt, clippy workspace y CC#4. **Sin `--no-verify` y sin bypassear nada.**

## Integración remota (T4)

Seis de seis workflows en verde sobre `d0b146dba44489d39eb643478e99a783d0dcdd52`: CI, Coverage,
Architecture Contracts, Sandbox Debt Sentinel, Supply chain security y Vault Drift Sweep. En el job `Test`,
`Skip args:` vacío — nada diferido — y cero `test result: FAILED` en todo el log.

`test_abort_crash_detected_sigabrt ... ok` y `test_divide_by_zero_crash_detected ... ok`: los dos tests que
fallaban antes de R6.9, en verde dentro de la autoridad de integración y no solo en local.
