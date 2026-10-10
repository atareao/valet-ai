# Tasks

## 0. Preparación y aprobación

- [ ] 0.1 Presentar este change (proposal + specs + tasks) y **esperar aprobación explícita** antes de escribir código.

## 1. RED: fijar el contrato

- [x] 1.1 Tests que fallan (`StatsRepo::db_sizes`): el resultado incluye una entrada `timeline_events` con su recuento; las once tablas anteriores siguen apareciendo.
- [x] 1.2 Tests que fallan (`StatsRepo::background_summary`): devuelve **cinco** orígenes en orden canónico con `timeline`; una fila `kind='timeline'` se cuenta (llamadas, tokens, coste y errores); `timeline` sin filas sale a cero y **sigue apareciendo**.
- [x] 1.3 Tests que fallan (migración): las **quince** claves `GENERATION_*` existen tras migrar; `GENERATION_TIMELINE_TEMPERATURE = 0.2`, `_REASONING = off` y `_MAX_TOKENS = 2048`; un valor ya existente no se sobrescribe; la migración es idempotente.
- [x] 1.4 Tests que fallan (frontend, stats): la tarjeta de procesos de fondo etiqueta `timeline` como «Línea temporal»; la tabla de tamaños muestra `timeline_events` con sus filas.
- [x] 1.5 Tests que fallan (frontend, Settings): la pestaña Prompts tiene una sub-pestaña «Timeline» que carga y guarda `timeline_prompt`; la pestaña Generación tiene el rol del timeline con sus tres campos; guardar envía las **quince** claves.
- [x] 1.6 Tests que fallan (stream): la respuesta de `/api/chat/stream` lleva `Cache-Control` con `no-transform` y `X-Accel-Buffering: no`, y mantiene `Content-Type: text/event-stream`.

## 2. GREEN

- [x] 2.1 `src/db/repos/stats.rs`: `timeline` en la lista canónica de `background_summary` y `timeline_events` en el array de `db_sizes`.
- [x] 2.2 Migración `…_timeline_generation_defaults.sql`: siembra idempotente de las tres claves `GENERATION_TIMELINE_*` con `0.2` / `off` / `2048`, sin sobrescribir valores no vacíos.
- [x] 2.3 `frontend/src/components/stats/BackgroundCard.tsx`: etiqueta de `timeline`.
- [x] 2.4 `frontend/src/components/SettingsDialog.tsx`: sub-pestaña «Timeline» en Prompts (`timeline_prompt`), rol del timeline en `GENERATION_BLOCKS` y sus tres claves en el formulario.
- [x] 2.5 `src/routes/stream.rs`: `Cache-Control: no-cache, no-transform` y `X-Accel-Buffering: no`.

## 3. REFACTOR / limpieza

- [x] 3.1 `cargo fmt` + `cargo clippy --all-targets -- -D warnings` (0 warnings) y `npm run lint` del frontend.
- [x] 3.2 Grep de listas cerradas hermanas (`archivist`, `consolidator`, `GENERATION_SEMANTIC`, `ORIGIN_LABELS`) para asegurar que no queda ningún origen ni rol sin `timeline`.

## 4. VERIFY / cierre

- [x] 4.1 `cargo test`, `cargo clippy`, `cargo fmt --check`, `npx vitest run` y `openspec validate timeline-observability-wiring --strict` en verde.
- [x] 4.2 Review de `@rust-reviewer` y `@react-reviewer`; hallazgos aplicados.
- [x] 4.3 `openspec archive timeline-observability-wiring`.
- [ ] 4.4 PR a `development` y merge.
- [ ] 4.5 Desplegar y comprobar en producción: el timeline aparece como proceso de fondo, su tabla sale en Database Sizes, y su prompt y sus parámetros son editables en Settings.
