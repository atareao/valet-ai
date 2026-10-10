# Tasks

## 0. Preparación y aprobación

- [x] 0.1 Presentar este change (proposal + specs + tasks) y **esperar aprobación explícita** antes de escribir código.

## 1. RED: fijar el contrato

- [x] 1.1 Tests que fallan (migración): `timeline_events` existe con `id`, `timestamp`, `category`, `fact`, `source_message_id` y `created_at`; el `CHECK` de `category` rechaza un valor fuera del conjunto; los tres índices existen; la migración es idempotente.
- [x] 1.2 Tests que fallan (migración): `llm_requests` acepta `kind='timeline'` y **conserva todas las filas previas**, incluidas las de los cinco valores anteriores del `CHECK`.
- [x] 1.3 Tests que fallan (parser): `{"events": […]}` válido produce hechos; `{"events": []}` no produce ninguno; un cuerpo vacío, truncado o sin JSON produce un error tipado, nunca un pánico; y un `source` no numérico o ausente descarta ese hecho.
- [x] 1.4 Tests que fallan (validación de hechos): una categoría desconocida se normaliza a `lifestyle` sin descartar el hecho; un `fact` vacío o en blanco descarta ese hecho; un `source` fuera del lote descarta ese hecho; el `timestamp` y el `source_message_id` se derivan del mensaje señalado por `source`, nunca del modelo ni de la hora del sistema; los hechos repetidos dentro de un lote se deduplican.
- [x] 1.5 Tests que fallan (pasada): un lote con hechos los persiste en `timeline_events` con su `source_message_id`, y la ficha, el estado y la marca se escriben en la misma transacción.
- [x] 1.6 Tests que fallan (best-effort): un extractor que devuelve vacío o JSON inválido **no aborta la pasada** —ficha, estado y marca se escriben igual— y la estadística queda con `kind='timeline'` y `status='error'`.
- [x] 1.7 Tests que fallan (prompt): el extractor recibe el lote **numerado** y con el `created_at` de cada mensaje, y usa `settings.timeline_prompt`, con un respaldo mínimo si la clave falta, está vacía o no se puede leer.
- [x] 1.8 Tests que fallan (tools): `timeline_get_events` filtra por `start`/`end` y por `category`, aplica `limit` y devuelve los hechos más recientes primero; `timeline_add_event` usa la hora actual en UTC cuando se omite `timestamp`; `timeline_delete_event` borra por `id` y avisa cuando el id no existe.
- [x] 1.9 Tests que fallan (permisos): `timeline_get_events` y `timeline_add_event` no requieren confirmación; `timeline_delete_event` exige aprobación explícita.
- [x] 1.10 Tests que fallan (catálogo): el catálogo tiene **ocho** skills y **veinte** herramientas; las tres `timeline_*` están todas en `timeline`; el test de integridad del catálogo pasa.
- [x] 1.11 Tests que fallan (modelo): `TIMELINE_MODEL` cae a `MEMORY_MODEL` cuando la variable no está definida, y una clave `GENERATION_TIMELINE_*` ausente cae al default de su rol.

## 2. GREEN — Backend

- [x] 2.1 Migración `…_timeline_events.sql`: la tabla, su `CHECK`, sus tres índices y la reconstrucción de `llm_requests` con el `CHECK` de `kind` ampliado, copiando todas las filas.
- [x] 2.2 Migración `…_timeline_prompts.sql`: siembra idempotente de `timeline_prompt`, `SKILL_TIMELINE_PROMPT` y `ROUTER_SKILL_TIMELINE_ENABLED`, sin sobrescribir un valor existente.
- [x] 2.3 `src/db/repos/timeline.rs`: inserción de un lote, listado por rango y categoría con límite (más recientes primero) y borrado por id.
- [x] 2.4 `CallKind::Timeline` con su cadena `'timeline'` y `GenerationRole::Timeline` con sus defaults.
- [x] 2.5 Capa D en `src/workers/episodic_memory.rs`: la llamada en modo JSON, el parser, la normalización, la escritura dentro de la transacción existente y la degradación sin abortar.
- [x] 2.6 `src/tools/timeline.rs` con las tres herramientas, y su registro en `build_tool_registry` (`src/lib.rs`) y en `src/tools/mod.rs`.
- [x] 2.7 Skill `timeline` en el catálogo cerrado de `src/orchestrator/skills.rs`, con su id, instrucciones, criteria, umbral, herramientas, `prompt_key` y `prompt_heading`.
- [x] 2.8 `TIMELINE_MODEL` en `src/config.rs`, con respaldo a `MEMORY_MODEL`.

## 3. REFACTOR / limpieza

- [x] 3.1 `cargo fmt` + `cargo clippy --all-targets -- -D warnings` (0 warnings).
- [x] 3.2 Revisar catálogo y registry: ocho skills, veinte herramientas, sin nombres duplicados y sin herramientas huérfanas.
- [x] 3.3 Retirar andamiaje muerto si algún test o variante deja de usarse.

## 4. Documentación y plantillas

- [x] 4.1 Documentar `TIMELINE_MODEL` en `.env.example` y en `.env.j2`, junto a los demás modelos.

## 5. VERIFY / cierre

- [x] 5.1 `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` y `openspec validate activity-timeline --strict` en verde.
- [x] 5.2 Review de `@rust-reviewer`; hallazgos aplicados.
- [x] 5.3 `openspec archive activity-timeline` y cierre del Tema en `plans/PLAN-005.md`.
- [ ] 5.4 En producción: comprobar que aparecen filas de `llm_requests` con `kind='timeline'` y que `timeline_events` crece con los días.
