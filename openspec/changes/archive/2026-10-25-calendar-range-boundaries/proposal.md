# Change: límites de rango del calendario — comparar instantes, no cadenas

## Why

Los eventos del **31 de octubre de 2026** existen en la base de datos y **Valet (chat) los ve**, pero
el calendario de la UI no los muestra. Reproducido y medido contra la base de datos de producción:

| Consulta sobre `co1` | Eventos devueltos |
|:---|---:|
| `start='2026-09-27'`, `end='2026-10-31'` (lo que pide la UI) | **14** |
| `start='2026-09-27'`, `end='2026-11-01'` | **16** |

Los dos ausentes son exactamente los dos eventos del 31 de octubre (`🏃 Rodaje cómodo — 8 km` y
`🏃 Rodaje cómodo — 9 km`). Los logs del contenedor lo confirman: la consulta del listado no
recurrente devuelve `rows_returned=14`.

**Causa raíz.** `EventsRepo::list_by_date_range` filtra comparando los timestamps **como texto**:

```sql
WHERE profile_id = ?1 AND rrule IS NULL AND start_time >= ?2 AND end_time <= ?3
```

y `list_events` (`src/handlers/events.rs`) pasa los límites **sin normalizar**. El frontend envía
fechas sin hora (`YYYY-MM-DD`) y la comparación lexicográfica

```
'2026-10-31T09:00:00Z' <= '2026-10-31'   →  FALSE
```

es falsa (la cadena con hora es más larga con el mismo prefijo), así que **todo evento que termina el
último día del rango desaparece**. Además la comparación es de **contención**
(`start_time >= start AND end_time <= end`), no de solape: un evento que empieza antes del rango y
termina dentro también se pierde.

**Por qué octubre y no otro mes.** `CalendarView` calcula `endOf("month").endOf("week")`, que solo
colapsa al propio último día del mes cuando ese día es **sábado**. Octubre de 2026 lo es
(`2026-10-31`); noviembre de 2026 da `2026-12-05`, agosto de 2026 da `2026-09-05` y enero de 2027 da
`2027-02-06`. Es un bug **intermitente según el calendario**, no un fallo de un mes concreto.

**Por qué el chat sí los ve.** El change `calendar-event-delete` normalizó los límites del *tool*
`get_events` a UTC (`src/tools/calendar.rs`), pero la ruta HTTP `/api/events` quedó sin tocar. Es el
mismo defecto con un camino arreglado y el otro no. El bug es **preexistente**: no lo introdujo
v0.20.1.

**Tres defectos latentes que comparten la misma raíz** (los tres verificados en producción):

1. **Los eventos recurrentes nunca se ven en la UI.** `expand_recurring` exige RFC 3339
   (`DateTime::parse_from_rfc3339`) y con límites `YYYY-MM-DD` devuelve `vec![]`. Hoy no muerde
   porque en producción hay **0 eventos con `rrule`**.
2. **Los timestamps almacenados no están normalizados.** El comentario de `src/tools/calendar.rs`
   afirma que «events are stored normalized to UTC with a `Z` suffix» y es **falso**: en producción
   hay 11 `start_time` con offset local (`+02:00`) y 5 con `Z`, mientras `end_time` es `Z` en 16/16.
   Los escribe el propio *tool*: `create_event` guarda `start` tal cual llega del LLM y `end_time` ya
   normalizado; `update_event` hace lo mismo con `start`. Toda comparación lexicográfica se apoya en
   una uniformidad de formato que **no existe**.
3. **`find_free_slots` compara cadenas.** `cursor < event.start_time` sobre esos mismos valores
   mezclados, con la misma fragilidad.

## What Changes

- **Límites del rango: parsear, no comparar texto.** `/api/events` SHALL aceptar `start`/`end` en
  RFC 3339 (con offset o `Z`) **o** como fecha suelta `YYYY-MM-DD` (que SHALL interpretarse como el
  día UTC completo) y SHALL responder **422** ante un límite no parseable.
- **Solape en vez de contención.** `list_by_date_range` SHALL devolver los eventos cuyo intervalo se
  **solape** con el rango consultado (semiabierto `[start, end)`), no solo los contenidos en él.
- **Comparación y orden por instante.** El filtrado y el orden de los eventos SHALL resolverse sobre
  instantes parseados en Rust, nunca sobre el orden lexicográfico de las cadenas almacenadas.
- **Recurrentes con cualquier límite.** `expand_recurring` SHALL recibir instantes ya parseados, de
  modo que los eventos con `rrule` vuelvan a aparecer en la UI.
- **`find_free_slots` por instante.** Los huecos libres SHALL calcularse comparando instantes.
- **Rango en hora local.** `CalendarView` SHALL enviar los límites en RFC 3339 con su offset local,
  de forma que el rango consultado sea exactamente el rango visible.
- **Escritura canónica.** `create_event`/`update_event` (tool y API HTTP) SHALL guardar `start_time`
  y `end_time` normalizados a UTC canónico (`...Z`), para dejar de generar filas con formatos
  mezclados.
- **Comentario corregido.** El comentario falso de `src/tools/calendar.rs` SHALL corregirse.

## Impact

- **Specs afectadas:** `events-api`, `db/schema`, `frontend/agenda`, `tools/agenda`.
- **Código:** `src/db/repos/events.rs` (`list_by_date_range`, `expand_recurring`, `find_free_slots`,
  `list_by_category` y un nuevo parseo de límites), `src/handlers/events.rs` (validación,
  normalización y 422), `src/tools/calendar.rs` (escritura canónica y comentario),
  `frontend/src/components/CalendarView.tsx` (formato del rango).
- **Sin cambios de esquema ni migraciones.** Las 16 filas existentes **no se tocan**: con la
  comparación por instante dejan de importar sus formatos.
- **Sin cambios de firma en el frontend:** `useEvents(start, end)` y `api.listEvents(start, end)`
  conservan su firma; cambia el **contenido** de lo que se les pasa.

### No-goals

- **No se migran los datos existentes**: los 11 `start_time` con `+02:00` se quedan como están.
- **No se toca `docker-compose.prod.yml`.**
- **No se cambia la semántica del tool `get_events`**: `start` + `end`/`duration` siguen
  comportándose igual; solo se arregla el sustrato de comparación.
- **No se valida la coherencia `end_time > start_time`** ni se rechazan payloads con fechas
  incoherentes: fuera del alcance de este change.
- **No se arregla la discrepancia del contrato `CalendarEvent`** de la spec de `frontend/agenda`
  (declara `start`/`end` y el tipo real usa `start_time`/`end_time`): se deja anotada.
