# Proposal: El test de la ventana temporal de `background_summary` deja de depender del reloj

## Why

El test `db::repos::stats::tests::test_background_summary_last_call_at` inserta dos filas con fechas **fijas**
(`'2026-10-01T10:00:00'` y `'2026-10-05T10:00:00'`) y después consulta `background_summary(&pool, 30)`. La
consulta real filtra por `created_at >= datetime('now', '-' || ?1 || ' days')`, es decir, el corte de 30 días
**se desplaza cada día**, y SQLite compara **cadenas**. El 2026-11-05 a las 00:00 UTC el corte pasó de
`'2026-10-05 04:29:56'` a `'2026-10-06 …'`: el literal del fixture dejó de caer dentro de la ventana, la fila
desapareció del `MAX(created_at)` y `last_call_at` pasó de `Some("2026-10-05T10:00:00")` a `None`.

El efecto no es cosmético. El job **Backend (Rust)** de CI está en rojo con ese único fallo (1561 tests en
verde, 1 en rojo; `cargo fmt` y `cargo clippy` pasan, el CI corre `fmt → clippy → tests`), y ese rojo
**bloquea todos los PRs del repositorio**: los cinco PRs de Dependabot pendientes —incluidos los dos que ni
tocan Rust, porque el job de Rust es obligatorio— y el CI de `main` y `development`, que comparten árbol y por
tanto también están rojos hoy. El producto **no** está roto: la consulta de `background_summary` es correcta y
lo que está mal es el test, que confía en que la fecha del calendario no avance.

## What Changes

- El test SHALL construir sus datos con fechas **relativas** al reloj de SQLite (`datetime('now', '-N days')`)
  y SHALL comparar `last_call_at` contra el valor **leído de la base de datos**, nunca contra un literal de
  fecha. Así el test deja de caducar.
- El test SHALL cubrir de forma determinista los tres casos de la ventana: fila **dentro** de la ventana, fila
  **fuera** de la ventana (más antigua que `days`) y origen **sin filas** (`last_call_at` = `None`). La
  pertenencia a la ventana SHALL verificarse con dos llamadas al mismo método con distinto `days` sobre el
  mismo juego de datos, no con fechas absolutas.
- No cambia el comportamiento de producción: la consulta, la firma y la semántica de `background_summary`
  quedan intactas.

## Capabilities

### New Capabilities

(ninguna)

### Modified Capabilities

(ninguna: cambio exclusivamente de tests; ningún requisito describe un comportamiento distinto.
El change usa `skip_specs: true` en `.openspec.yaml`, como el precedente
`openspec/changes/archive/2026-09-29-fix-spec-validation-lint-flaky`.)

## Impact

- Código: `src/db/repos/stats.rs`, **solo** el bloque `#[cfg(test)]` (el test citado; opcionalmente, un helper
  de test que fije `created_at` con fechas relativas).
- Sin cambios en producción, en el esquema de base de datos, en la API ni en el frontend.
- Operativa: desbloquea el CI de `main` y `development` y, con él, el merge de los PRs de Dependabot
  pendientes. Cierra el bloqueo detectado el 2026-11-05.
