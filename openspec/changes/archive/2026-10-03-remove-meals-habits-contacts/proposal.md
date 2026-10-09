# Change: Eliminar las herramientas meals, habits y contacts

## Why

Tras la auditoría del sistema de herramientas (`src/tools/`) se decidió podar el catálogo. `meals`,
`habits` y `contacts` son herramientas que no encajan en el producto actual: `contacts` además era
una **tool fantasma** (implementada, con 17 tests, pero nunca registrada en `lib.rs`). Las tres
arrastran repositorios, tablas, semillas de BD, exportación de datos y entradas en la documentación
que mantienen superficie y complejidad sin uso real. La decisión del usuario es eliminarlas de
forma completa, incluido el `DROP` de sus tablas.

## What Changes

- **Tools fuera del registry.** Se eliminan los ficheros `src/tools/meals.rs`, `src/tools/habits.rs`
  y `src/tools/contacts.rs`, sus entradas en `src/tools/mod.rs` y el registro de `MealsTool` y
  `HabitsTool` en `src/lib.rs` (el orquestador deja de exponerlas al LLM).
- **Repositorios huérfanos fuera.** Se eliminan `src/db/repos/meal_plans.rs`,
  `src/db/repos/shopping_list.rs`, `src/db/repos/habits.rs` y `src/db/repos/contacts.rs`, junto con
  sus entradas en `src/db/repos/mod.rs`. Sus únicos consumidores eran las tres tools eliminadas.
- **Semillas de BD fuera.** Se quitan las filas `meals`, `habits` y `contacts` de
  `seed_defaults` en `src/db/repos/tools.rs`. Una migración nueva las borra de bases existentes.
- **`DROP` de tablas.** Se añade una migración idempotente que elimina `contacts_fts`, `contacts`,
  `meal_plans`, `shopping_list`, `habits` y `habit_logs`. **Irreversible: se pierden los datos.**
- **Exportación alineada.** `src/routes/export.rs` deja de exportar `contacts`, `meal_plans`,
  `shopping_list`, `habits` y `habit_logs` (sus tablas ya no existen).
- **`unified_search` sin la dimensión de contactos.** Su consulta a `contacts_fts` deja de tener
  tabla; se elimina la dimensión `contacts` de `src/tools/unified_search.rs` (tool que sigue
  implementada pero no registrada; esto evita una consulta a una tabla inexistente).
- **`StatsRepo::db_sizes` alineado.** Su lista de tablas está cableada e incluía las cinco que se
  dropean; se quitan para que no consulten tablas inexistentes.
- **Binario de siembra `src/bin/seed.rs`.** Deja de sembrar contactos, hábitos, planes de comidas y
  lista de la compra (funciones y llamadas eliminadas).
- **Tests inline de `src/db/schema.rs`.** Se invierten los tests que exigían esas tablas.
- **Documentación.** Se actualizan `README.md` y `README.es.md`: fuera las viñetas de Meals/Habits y
  la lista de «14 tools» pasa a reflejar las que quedan.

## Impact

- Afecta a: `src/tools/{mod,meals,habits,contacts}.rs`, `src/tools/unified_search.rs`, `src/lib.rs`,
  `src/db/repos/{mod,meal_plans,shopping_list,habits,contacts,tools,stats}.rs`, `src/routes/export.rs`,
  `src/bin/seed.rs`, `src/db/schema.rs` (tests), `migrations/<nueva>.sql`, `README.md`,
  `README.es.md`, `tests/api/tools.rs` y `tests/db/migrations.rs`.
- Specs: `openspec/specs/db/repos/spec.md` (1 MODIFIED) y
  `openspec/specs/orchestrator/profile-injection/spec.md` (1 MODIFIED). No existen specs activas
  propias de `meals`, `habits` ni `contacts`, así que no hay requirements que retirar por ese lado.
- Sin cambios de frontend (ninguna referencia a esas tools).
- Sin tocar `docker-compose.prod.yml` ni el despliegue.

### Riesgo asumido

El `DROP` de tablas es destructivo: se pierden los datos existentes de esas cinco tablas. Es una
decisión explícita del usuario.

### Fuera de alcance

- Cablear `notes`/`contacts`/`unified_search` (las fantasma restantes): queda para un change aparte.
- El resto de hallazgos del informe (toggle `enabled`, permisos por operación, `search_places`,
  `get_weather` por ciudad, etc.).
