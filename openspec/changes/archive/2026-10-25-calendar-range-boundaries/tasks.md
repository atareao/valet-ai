# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y **OK explícito** del usuario antes de escribir código o tests. *(OK del 2026-10-25.)*

## 1. RED — repositorio: rango por instante y solape

- [x] 1.1 Test que falla: `list_by_date_range` con `end = "2026-10-31"` (fecha suelta) devuelve el evento que **termina** ese día (regresión del bug reportado).
- [x] 1.2 Test que falla: solape por delante — un evento que empieza antes del rango y termina dentro SHALL aparecer.
- [x] 1.3 Test que falla: un evento que termina **exactamente** en el límite inferior del rango NO SHALL aparecer (intervalo semiabierto).
- [x] 1.4 Test que falla: con una fila almacenada con offset (`+02:00`) y otra con `Z`, la inclusión SHALL decidirse por instante, no por orden lexicográfico.
- [x] 1.5 Test que falla: el resultado SHALL ordenarse por instante de inicio con formatos mezclados.
- [x] 1.6 Test que falla: una fila con timestamps ilegibles SHALL incluirse (falla abierto) y quedar al final del orden.

## 2. GREEN — repositorio: rango por instante y solape

- [x] 2.1 `src/db/repos/events.rs`: helper público `parse_range_bound(raw, upper)` que acepta RFC 3339 (offset o `Z`) o `YYYY-MM-DD` (día UTC completo) y devuelve `Option<DateTime<FixedOffset>>`.
- [x] 2.2 `list_by_date_range`: recibe los límites como `DateTime<FixedOffset>`, filtra por solape (`start_time < end AND end_time > start`) comparando instantes y ordena por instante.
- [x] 2.3 `list_by_category`: ordena por instante con el mismo criterio.
- [x] 2.4 Actualizar los tests existentes del módulo que pasan cadenas a `list_by_date_range`.

## 3. RED — recurrencia

- [x] 3.1 Test que falla: `expand_recurring` con límites de fecha suelta genera las instancias del rango (hoy devuelve `vec![]`).
- [x] 3.2 Test que falla: una instancia que solapa parcialmente el rango SHALL incluirse.
- [x] 3.3 Actualizar los tests unitarios existentes de `expand_recurring` que pasan cadenas.
- [x] 3.4 Test que falla: las instancias se emiten en UTC canónico aunque la base esté almacenada con offset (`+02:00`), sin reetiquetar la hora de pared.

## 4. GREEN — recurrencia

- [x] 4.1 `expand_recurring`: recibe `DateTime<FixedOffset>` y aplica solape a cada instancia.
- [x] 4.2 `fmt_rfc3339`: convertir el instante a UTC antes de formatearlo, de modo que una base con offset no se reetiquete con su hora de pared.

## 5. RED — huecos libres

- [x] 5.1 Test que falla: `find_free_slots` con un evento guardado con offset y otro con `Z` calcula los huecos por instante (no por cadena).

## 6. GREEN — huecos libres

- [x] 6.1 `find_free_slots`: construye los extremos del día como instantes y compara por instante.

## 7. RED — `/api/events`

- [x] 7.1 Test que falla: `GET /api/events?start=2026-09-27&end=2026-10-31` devuelve el evento que termina el 31 de octubre (regresión).
- [x] 7.2 Test que falla: `GET /api/events` con un límite no parseable devuelve **422**.
- [x] 7.3 Test que falla: `POST /api/events` con `start_time` con offset lo guarda normalizado a UTC canónico.
- [x] 7.4 Test que falla: `POST /api/events` con `start_time` no parseable devuelve **422**.
- [x] 7.5 Test que falla: `PUT /api/events/:id` con `start_time` con offset lo guarda normalizado a UTC canónico.

## 8. GREEN — `/api/events`

- [x] 8.1 `src/handlers/events.rs`: parsear los límites del listado con `parse_range_bound` (422 si no parsean) y pasarlos tipados al repositorio.
- [x] 8.2 `src/handlers/events.rs`: normalizar `start_time`/`end_time` a UTC canónico en `create_event` y `update_event` (422 si no son RFC 3339).

## 9. RED — tool `calendar`

- [x] 9.1 Test que falla: `create_event` con `start` con offset guarda `start_time` normalizado a UTC canónico.
- [x] 9.2 Test que falla: `update_event` con `start` con offset guarda `start_time` normalizado a UTC canónico.

## 10. GREEN — tool `calendar`

- [x] 10.1 `src/tools/calendar.rs`: `create_event` y `update_event` normalizan los extremos a UTC canónico; corregir el comentario falso sobre el almacenamiento en UTC.

## 11. RED — frontend

- [x] 11.1 Test que falla: `CalendarView` envía el rango como RFC 3339 **con el offset local**, cubriendo desde el inicio del primer día visible hasta el inicio del día siguiente al último día visible.
- [x] 11.2 Test que falla: el rango enviado incluye el último día del mes visible (regresión de octubre de 2026).

## 12. GREEN — frontend

- [x] 12.1 `frontend/src/components/CalendarView.tsx`: calcular los límites con `dayjs` y enviarlos con `format()` (offset local, intervalo semiabierto).

## 13. Verificación y cierre

- [x] 13.1 `cargo test` en verde.
- [x] 13.2 `cargo clippy --all-targets -- -D warnings` (0 warnings).
- [x] 13.3 `cargo fmt --check` en verde.
- [x] 13.4 `npx tsc --noEmit`, `npm run lint` y `npx vitest run` en verde.
- [x] 13.5 `openspec validate calendar-range-boundaries --strict` en verde.
- [x] 13.6 Revisión con `@rust-reviewer` (backend) y `@react-reviewer` (UI); hallazgos aplicados.
- [ ] 13.7 Verificación de extremo a extremo contra la base de datos de producción: el rango de octubre de 2026 devuelve los 16 eventos, los dos del 31 de octubre incluidos.
- [ ] 13.8 Verificar que las specs destino conservan sus escenarios originales tras el `archive` (los `## MODIFIED` los reemplazan enteros: no SHALL perderse ningún escenario previo).
- [ ] 13.9 `openspec archive calendar-range-boundaries` y registro del cierre en `AGENTS.md` §V si procede.
