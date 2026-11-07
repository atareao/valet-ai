# PLAN-016 — Desbloquear el CI: el test de la ventana temporal de `background_summary` deja de depender del reloj

- **Estado:** ✅ Completado, archivado y desplegado como **v0.27.2** el 2026-11-05.
- **Fecha:** 2026-11-05
- **Change de OpenSpec:** `stats-test-clock-independence` (`skip_specs: true`, como el precedente
  `2026-09-29-fix-spec-validation-lint-flaky`), archivado en
  `openspec/changes/archive/2026-11-05-stats-test-clock-independence/`.
- **Release:** `v0.27.2` desde `main`.

---

## 1. Motivo

El usuario pidió fusionar los cinco PRs de Dependabot que estaban en verde (#283, #282, #279, #278, #268). Al
comprobarlos, **ninguno se podía mergear**: los cuatro que apuntan a `development` estaban `BLOCKED` con el
job **Backend (Rust)** en rojo, y #268 mostraba verde solo porque sus checks eran del día anterior.

El rojo no tiene nada que ver con los bumps: lo producía **un único test con fecha incrustada** que caducó esa
madrugada. Como `main` y `development` comparten árbol y el job de Rust es obligatorio, el fallo **bloquea
todos los PRs del repositorio**.

## 2. Diagnóstico (evidencia)

**Test:** `db::repos::stats::tests::test_background_summary_last_call_at` (`src/db/repos/stats.rs:2115`).

- CI: `test result: FAILED. 1561 passed; 1 failed; 1 ignored`. Es el **único** fallo de la suite.
- Salida: `assertion left == right failed / left: None / right: Some("2026-10-05T10:00:00")`, panic en
  `src/db/repos/stats.rs:2151`.
- Reproducido en local: `cargo test --lib test_background_summary_last_call_at` → mismo panic.
- El CI corre `cargo fmt --all -- --check → cargo clippy --all-targets -- -D warnings → cargo test`: fmt y
  clippy **pasan**, el job muere en el paso de tests.
- #268 (`rustls`): checks `SUCCESS` de `2026-11-04T19:05Z` (antes del cambio de día), base `ef3342e` mientras
  `main` ya está en `3611984`; al actualizar la rama, cualquier run nuevo cae en la misma bomba.

**Causa raíz (doble):**

1. **Comparación de cadenas.** `created_at` es `TEXT` y SQLite compara como texto. El corte de
   `datetime('now','-30 days')` usa el formato `YYYY-MM-DD HH:MM:SS` (**espacio** en la posición 10) y el
   fixture usaba `YYYY-MM-DDTHH:MM:SS` (**`T`** en la posición 10). Como `'T'` (0x54) > `' '` (0x20), el
   fixture del 5-oct pasaba por delante del corte y entraba en la ventana. El test pasaba **por accidente de
   formato**.
2. **El corte se desplaza a diario.** El 2026-11-05 el corte pasó a `'2026-10-06 …'`, mayor que el fixture →
   la fila salió del `MAX(created_at)` → `last_call_at: None`.

La consulta de producción (`src/db/repos/stats.rs:371`, `background_summary`) es **correcta**; no se toca.

**Barrido de bombas latentes** (8 fechas absolutas en `src/`): las demás son inocuas.
`notifications.rs` (`2020-01-01`) y `push.rs` (`2000-01-01`) las usan solo para forzar el **orden** de las
filas, sin ventana relativa; `unified_search.rs` (`2025-01-01T…Z`) como valor estático de aserción;
`stats.rs:1210` (`test_db_sizes_includes_timeline_events`) solo cuenta filas.

## 3. Objetivo y alcance

**Objetivo:** que el test pase cualquier día, cubriendo de forma determinista la semántica de la ventana, y
dejar el CI de `main` y `development` verde para poder retomar la cola de Dependabot.

**Alcance:** `src/db/repos/stats.rs`, **solo** el bloque `#[cfg(test)]`.

**Fuera de alcance:** la consulta y la firma de `background_summary`; los tests con fechas absolutas antiguas
(se documentan como aceptados); un detector genérico de «tests con fecha fija»; cualquier otro asunto abierto
del repositorio.

## 4. Diseño del arreglo

- Los fixtures se escriben con fechas **relativas** al reloj de SQLite: `UPDATE llm_requests SET created_at =
  datetime('now', '-N days') WHERE id = ?1`. Se usa el reloj de SQLite porque es exactamente el que evalúa la
  ventana, con el mismo formato y sin depender de Rust.
- La expectativa de `last_call_at` se **lee de la base de datos** (`SELECT created_at …`), nunca de un literal.
- La pertenencia a la ventana se comprueba **cambiando `days`** sobre el mismo juego de datos (`days = 30`
  excluye la fila de `-40 days`; `days = 90` la incluye), en vez de comparar contra fechas absolutas.
- Juego de datos: `lc-a` (`now - 40 days`, fuera), `lc-b` (`now - 2 days`, dentro), `lc-c` (`now - 1 day`,
  dentro y la más reciente), todas `kind = 'router'`, `model = 'A'`; más el `kind` `archivist` sin filas
  (`last_call_at` = `None`).
- Descartado: inyectar el reloj o congelar el tiempo (`faketime`, crates de freezegun) — dependencia y
  variable de entorno en CI para un solo test; y calcular la fecha en Rust con `chrono`, que reproduce el
  problema del formato.

## 5. Plan de ejecución

1. **Diagnóstico y evidencia** (hecho antes de abrir el change): reproducción local y del CI, causa raíz,
   barrido de bombas latentes.
2. **TDD-RED**: el test ya falla; se confirma que es el único rojo de la suite.
3. **TDD-GREEN**: fixtures relativos + expectativa leída de la BD + cobertura de la ventana cambiando `days`.
4. **TDD-REFACTOR**: limpieza del test y comentario que explique por qué las fechas son relativas.
5. **Verificación**: `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test`, `just check-all`,
   `openspec validate --all --strict`.
6. **Auditoría**: `@rust-reviewer` sobre el diff del test.
7. **Desbloqueo de la cola** (ver §7).
8. **Cierre**: `openspec archive` + este plan a `PLAN-016-ARCHIVED.md`.

## 6. Criterios de aceptación

- [x] `cargo test` en verde: 1562 tests de lib + migraciones.
- [x] `cargo clippy --all-targets -- -D warnings` sin warnings y `cargo fmt --all -- --check` sin diffs.
- [x] El test no contiene ninguna fecha absoluta ni compara contra un literal de fecha.
- [x] El test cubre dentro / fuera de ventana / sin filas de forma determinista.
- [x] CI de `development` y de `main` en verde el mismo día.
- [x] Los cinco PRs de Dependabot resueltos según §7.
- [x] `openspec validate --all --strict` sin errores.

## 7. Cola de mantenimiento de Dependabot

**Cinco PRs en verde antes del bloqueo:**

| PR | Paquete | Salto | Base | Ruta |
|:---|:---|:---|:---|:---|
| #278 | `uuid` | 1.28.0 → 1.29.0 | `development` | merge directo |
| #279 | `sha2` | 0.11.0 → 0.11.1 | `development` | merge directo |
| #283 | `@testing-library/user-event` | 14.6.7 → 14.6.8 | `development` | merge directo |
| #282 | `typescript-eslint` | 8.71 → 8.72 | `development` | merge directo |
| #268 | `rustls` | 0.23.45 → 0.23.46 | `main` | **security update** |

**Decisión del usuario (2026-11-05) sobre #268:** es una *security update* (hay 4 alertas abiertas de `rustls`
y 1 de `minimatch`), y Dependabot **no respeta `target-branch`** en las alertas de seguridad, por eso apunta a
`main`. Mergearlo a `main` dispararía `image.yml` y movería la imagen `latest` de GHCR a un commit sin tag de
release. Por eso **se recrea el bump de `rustls` en una rama desde `development`** para que fluya por
`release` → `main`, y se **cierra #268 explicando el motivo**.

**Pendientes de adaptación (saltos mayores, CI rojo, no se mergean sin trabajo):**

- #284 `antd` 5.29.3 → 6.6.8 (**mayor**).
- #281 `react-leaflet` 4.2.1 → 5.0.0 (**mayor**).
- #280 `react` + `@types/react` (**mayor**).
- #288 `chore/docs-agents-lock` (documentación; estaba en conflicto con el índice de `AGENTS.md` al
  comprobarlo el 2026-11-05) — decisión pendiente del usuario.

## 8. Riesgos

- **Granularidad de segundo del reloj de SQLite**: dos filas escritas en el mismo segundo empatarían.
  Mitigado con offsets de **días** distintos.
- **Tentación de arreglarlo tocando la consulta**: no se toca; la consulta es correcta y hace lo que debe en
  producción.
- **El bloqueo afecta a todo el repositorio**: mientras el test esté roto, no entra ningún PR ni se puede
  publicar un release con CI verde (el tag se crea a mano, así que un release podría salir con el CI rojo sin
  que nadie lo note).

## 9. Estado y siguientes pasos

1. ✅ `[OPENSPEC - ARCHIVE]` Change `stats-test-clock-independence` aprobado, implementado con TDD (RED → GREEN →
   REFACTOR) y archivado en `openspec/changes/archive/2026-11-05-stats-test-clock-independence/`.
2. ✅ Auditoría de `@rust-reviewer` sobre el diff del test: **APRUEBA** (0 bloqueantes, 0 mayores). Nits resueltos
   (tareas marcadas, alcance trazado, riesgo residual registrado en `design.md`) o aceptados conscientemente.
3. ✅ Mergeado a `development` con su CI verde (PR #304) y **desbloqueo verificado**: los cuatro PRs de Dependabot
   que apuntaban a `development` pasaron a verde y se mergearon (#278 `uuid` 1.29.0, #279 `sha2` 0.11.1, #283
   `@testing-library/user-event` 14.6.8, #282 `typescript-eslint` 8.72).
4. ✅ El bump de `rustls` 0.23.45 → 0.23.46 se recreó en `development` (PR #305) —**cierra las 4 alertas** de
   `rustls`: #22 low, #23 medium, #24 low, #25 low— y el PR #268 se **cerró** explicando el enrutado por `release`.
5. ✅ Release **v0.27.2** (PR #306, tag `v0.27.2`): CI de `main` en verde y los dos `Image` runs (push a `main` y
   tag) publicados sin incidencias.
6. ⏸️ Pendientes de adaptación (saltos mayores, piden trabajo y no se mergean): #284 (`antd` 5→6), #281
   (`react-leaflet` 4→5), #280 (`react` + `@types/react`) y la decisión del usuario sobre #288
   (`chore/docs-agents-lock`, en conflicto con este índice).
7. ⏸️ Pendiente menor: la alerta de `minimatch` (npm, *dev*) sigue sin PR abierto de Dependabot.
