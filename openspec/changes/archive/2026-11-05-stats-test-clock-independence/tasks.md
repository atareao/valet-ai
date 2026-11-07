# Tasks

## 1. Diagnóstico y evidencia

- [x] 1.1 Reproducir el fallo en local con `cargo test --lib test_background_summary_last_call_at` y registrar la salida (`left: None`, `right: Some("2026-10-05T10:00:00")`, panic en `src/db/repos/stats.rs:2151`). Reproducido también desde el log del CI (run 41967949983): `1561 passed; 1 failed`.
- [x] 1.2 Dejar constancia de la causa raíz: fixture con fecha absoluta + comparación de cadenas (`'T'` vs `' '` en la posición 10) + corte relativo que se desplaza a diario.

## 2. TDD: el test deja de depender del reloj

- [x] 2.1 **RED** — Confirmado que el rojo del CI es el único de la suite (`1561 passed; 1 failed`) y que `cargo fmt`/`cargo clippy` ya pasaban.
- [x] 2.2 **GREEN** — `test_background_summary_last_call_at` reescrito con fechas relativas, expectativa leída de la base de datos y cobertura de la ventana (`days = 30` → `calls == 2`; `days = 90` → `calls == 3`). `cargo test --lib db::repos::stats` → 44 passed.
- [x] 2.3 Cobertura determinista de la ventana incluida en 2.2 (dos llamadas con distinto `days` sobre el mismo juego de datos, en vez de fechas absolutas).
- [x] 2.4 Aserción negativa conservada y ampliada: `archivist` → `last_call_at` es `None` y `calls == 0`.
- [x] 2.5 **REFACTOR** — Helper `set_created_at` con doc-comment del porqué; aserción de `rows_affected` para que un id con errata falle con mensaje claro.
- [x] 2.6 Homogeneizados al mismo idioma los tres fixtures hermanos de test de ventana del mismo fichero (`test_background_summary_days_window`, `test_by_model_days_window`, `test_purge_old`) a `datetime('now','-60 days')`, sin cambiar ninguna aserción.

## 3. Verificación

- [x] 3.1 `cargo fmt --all -- --check` sin diffs.
- [x] 3.2 `cargo clippy --all-targets -- -D warnings` sin warnings.
- [x] 3.3 `cargo test --lib` → **1562 passed; 0 failed; 1 ignored** (antes 1561 passed; 1 failed).
- [x] 3.4 `just check-all` → exit 0 (fmt, clippy, test, frontend-lint, frontend-test, frontend-check; bundle inicial 47,77 kB del presupuesto de 70 kB).
- [x] 3.5 `openspec validate --all --strict` → 63 passed, 0 failed.

## 4. Auditoría

- [x] 4.1 `@rust-reviewer` sobre el diff: **APRUEBA**, 0 bloqueantes y 0 mayores. Menores y nits resueltos o aceptados conscientemente (ver `design.md`).
- [x] 4.2 Re-verificación tras aplicar el nit de `rows_affected`: 3.2, 3.3 y 3.4 repetidos en verde.

## 5. Desbloqueo de la cola de Dependabot

- [x] 5.1 Mergear el arreglo a `development` (PR propio con su CI verde) y comprobar que el CI de `development` pasa a verde.
- [x] 5.2 `update-branch` + espera de CI de los cuatro PRs que apuntan a `development` (#278 `uuid`, #279 `sha2`, #283 `@testing-library/user-event`, #282 `typescript-eslint`) y mergearlos con `--merge --delete-branch`.
- [x] 5.3 Recrear el bump de `rustls` 0.23.45 → 0.23.46 (security update) en una rama desde `development`, para que fluya por `release` → `main` sin mover la imagen `latest` a un commit sin release; cerrar el PR #268 explicando el motivo.
- [x] 5.4 Sincronizar `main` ← `development` y confirmar el CI de `main` en verde.
- [x] 5.5 Anotar como pendientes de trabajo de adaptación los saltos mayores con CI rojo: #284 (`antd` 5→6), #281 (`react-leaflet` 4→5), #280 (`react` + `@types/react`), y decidir el PR #288 (`chore/docs-agents-lock`, en conflicto).

## 6. Cierre

- [x] 6.1 `openspec archive stats-test-clock-independence`.
- [x] 6.2 Renombrar el plan a `plans/PLAN-016-ARCHIVED.md` y actualizar la fila del índice en `AGENTS.md` §VI.
