# Tasks: remove-meals-habits-contacts

## Bloque 0 — Baseline (antes de tocar nada)
- [x] 0.1 Registrar el baseline por CLI: `cargo test` (recuento passed/failed), `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`.
- [x] 0.2 Inventario de residuos: `grep -rn -iE 'meals|habits|contacts' src/ migrations/ README.md README.es.md` y guardar la lista como referencia.

## Bloque 1 — RED (tests que documentan la retirada)
- [x] 1.1 Test de integración en `tests/api/tools.rs`: la lista de `GET /api/tools` NO contiene `meals`, `habits` ni `contacts` (hoy sí → rojo).
- [x] 1.2 Test de migración: tras aplicar todas las migraciones, `sqlite_master` NO contiene las tablas `contacts`, `contacts_fts`, `meal_plans`, `shopping_list`, `habits` ni `habit_logs`.
- [x] 1.3 Ajustar tests existentes que fijan lo eliminado:
  - `src/db/repos/tools.rs` (`tools.len()` 10 → 7).
  - `tests/db/migrations.rs`: retirar/invertir `test_migrations_creates_meal_plans_table`, `test_migrations_creates_habits_table`, `test_migrations_creates_habit_logs_table` y el bucle que exige `meal_plans`/`shopping_list`/`habits`/`habit_logs`.
  - `src/db/schema.rs` (tests inline): invertir los mismos cuatro tests y el bucle (líneas ~126-218).
- [x] 1.4 Verificar por CLI: `cargo test` → los nuevos tests en rojo y el resto en verde.

## Bloque 2 — GREEN (eliminar código)
- [x] 2.1 Borrar `src/tools/meals.rs`, `src/tools/habits.rs`, `src/tools/contacts.rs` y sus `pub mod` en `src/tools/mod.rs`.
- [x] 2.2 Quitar el registro de `MealsTool` y `HabitsTool` en `src/lib.rs`.
- [x] 2.3 Borrar `src/db/repos/meal_plans.rs`, `shopping_list.rs`, `habits.rs`, `contacts.rs` y sus entradas en `src/db/repos/mod.rs`.
- [x] 2.4 Quitar la dimensión `contacts` de `src/tools/unified_search.rs` (enum de dimensiones, mapeo a `contacts_fts` y descripción).
- [x] 2.5 Quitar las semillas `meals`, `habits` y `contacts` de `seed_defaults` (`src/db/repos/tools.rs`).
- [x] 2.6 Quitar `contacts`, `meal_plans`, `shopping_list`, `habits` y `habit_logs` de `EXPORTABLE_TABLES` y de `export_all_tables` (`src/routes/export.rs`).
- [x] 2.7 Quitar `contacts`, `habit_logs`, `habits`, `meal_plans` y `shopping_list` de la lista cableada de `StatsRepo::db_sizes` (`src/db/repos/stats.rs`).
- [x] 2.8 Quitar de `src/bin/seed.rs` las llamadas y funciones `seed_contacts`, `seed_habits`, `seed_meal_plans` y `seed_shopping_list` (y el `use uuid::Uuid` si queda sin uso).
- [x] 2.9 Verificar por CLI: `cargo test` y `cargo check` en verde.

## Bloque 3 — Migración destructiva
- [x] 3.1 Añadir `migrations/<timestamp>_drop_meals_habits_contacts.sql`: `DROP TABLE IF EXISTS` para `contacts_fts`, `contacts`, `meal_plans`, `shopping_list`, `habits`, `habit_logs` (FTS antes que su tabla contenido) y `DELETE FROM tools WHERE name IN ('meals','habits','contacts')`.
- [x] 3.2 Verificar por CLI que las migraciones aplican limpias (test de 1.2 en verde; `cargo test` completo).

## Bloque 4 — Documentación
- [x] 4.1 `README.md` y `README.es.md`: quitar viñetas de Meals/Habits y actualizar la lista de herramientas.
- [x] 4.2 Spec deltas redactados: `openspec/specs/db/repos/spec.md` (listado de tablas de `db_sizes`) y `openspec/specs/orchestrator/profile-injection/spec.md` (tabla de descripciones enriquecidas).

## Bloque 5 — REFACTOR y cierre
- [x] 5.1 `cargo fmt --check` y `cargo clippy --all-targets -- -D warnings` limpios.
- [x] 5.2 `grep -rn -iE '\b(meals|habits|contacts)\b' src/ migrations/ README.md README.es.md` sin residuos en producción.
- [x] 5.3 Revisión con `rust-reviewer`.
- [x] 5.4 Verificación final por CLI y PR a `development`; después PR de archivado (`openspec archive remove-meals-habits-contacts`).
