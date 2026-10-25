# Design — límites de rango del calendario

## Contexto

El calendario pide el rango visible como fechas sueltas y el repositorio compara los timestamps
almacenados **como cadenas**. Los valores almacenados, además, no comparten formato (11 filas con
offset local en `start_time`, 5 con `Z`). Cualquier arreglo que se apoye en el orden lexicográfico
—incluido normalizar solo los límites— deja el defecto vivo para las filas con offset.

## Decisiones

### D1 — Comparar por instante en Rust, no por cadena en SQL

`list_by_date_range` SHALL recibir los límites como instantes ya parseados y SHALL decidir el filtro
comparando `DateTime` (instantes), no cadenas.

**Alternativas descartadas:**

- **`datetime()`/`strftime()` en SQL.** Devuelven `NULL` **en silencio** ante un valor no parseable:
  la fila desaparecería sin error, que es exactamente el fallo que estamos arreglando. Descartado.
- **Normalizar los límites y seguir comparando texto.** Con 11 filas en offset local el resultado
  sigue siendo incorrecto: el mismo instante escrito con dos offsets distintos se compara al revés
  cuando coincide la parte de fecha. Descartado.
- **Pre-filtro SQL «ancho» + filtro exacto en Rust.** Un pre-filtro lexicográfico no es un
  superconjunto demostrable del filtro por instante (contraejemplo: `2026-10-06T19:00:00+02:00` es
  anterior a `2026-10-06T18:30:00Z` como instante y posterior como cadena). Descartado por no ser
  seguro.

**Consecuencia aceptada:** la consulta deja de usar el índice `idx_events_start_time` para el filtro
de tiempo; se traen las filas del perfil y se filtran en memoria. Es el mismo patrón que ya usa la
rama de recurrentes (`WHERE profile_id = ?1 AND rrule IS NOT NULL`, sin filtro de fechas) y la tabla
es por perfil y de tamaño reducido (16 filas en producción). Se prioriza la corrección sobre el uso
del índice.

### D2 — Intervalo semiabierto `[start, end)`

La contención se sustituye por solape: `start_time < range_end AND end_time > range_start`.

Un intervalo semiabierto hace que un límite de fecha suelta se traduzca sin ambigüedad:
`end = 2026-10-31` significa «hasta el final del 31 de octubre» si se interpreta como el **inicio del
día siguiente** (`2026-11-01T00:00:00Z`), sin inventar un `23:59:59` que excluiría el último segundo.

### D3 — Límites: RFC 3339 o fecha suelta

- RFC 3339 con offset o `Z` → se usa el instante tal cual.
- `YYYY-MM-DD` → día UTC completo: límite inferior `T00:00:00Z`, límite superior el **inicio del día
  siguiente**.
- Cualquier otra cosa → límite no parseable.

**Compatibilidad:** el frontend actual envía `YYYY-MM-DD`, y ese caso sigue funcionando con la
semántica esperada («todo el 31 de octubre»). Es lo que hace que el arreglo sea retrocompatible sin
tocar el cliente, y por eso se conserva.

### D4 — Un límite no parseable en `/api/events` es un 422

Hoy un límite basura devuelve una lista vacía **sin avisar** (comparación lexicográfica). Con el
parseo explícito el silencio deja de ser una opción: el handler SHALL responder
`422 Unprocessable Entity`. El *tool* ya devuelve `ToolError::InvalidArguments` para un `end`
inválido; se alinea el comportamiento.

El parseo vive en el **borde** (handler y tool), no en el repositorio: el repositorio recibe
instantes ya tipados. Así el 422 es una decisión HTTP donde corresponde y no hace falta un tipo de
error nuevo en la capa de datos.

### D5 — El rango se envía en hora local (decisión del usuario)

`CalendarView` calcula el rango visible con `dayjs` en hora local y SHALL enviarlo como RFC 3339 con
el offset del navegador (`format()`), no como `YYYY-MM-DD`.

**Por qué no usar la zona configurada en `settings`:** el calendario **renderiza** en la zona del
navegador (`dayjs(...).format("YYYY-MM-DD")` para colocar cada evento en su celda). Si el rango se
calculara con otra zona, el rango consultado y el rango visible dejarían de coincidir. El offset del
navegador es el único que garantiza que «lo que pido» es «lo que muestro».

**Nota:** los dos extremos pueden llevar offsets distintos (en octubre de 2026, `+02:00` al principio
y `+01:00` al final, porque el cambio de hora fue el 25 de octubre). Es correcto: cada extremo lleva
el offset vigente en su propio instante.

### D6 — Escritura canónica a UTC

`create_event` (tool) guarda hoy `start` tal cual llega del modelo y `end_time` normalizado; de ahí
las 11 filas con `+02:00`. `update_event` hace lo mismo con `start`. Ambos SHALL guardar los dos
extremos normalizados a UTC canónico. La API HTTP SHALL hacer lo mismo en `POST /api/events` y
`PUT /api/events/:id`, y SHALL responder 422 si un `start_time`/`end_time` no es RFC 3339.

**No se migran los datos existentes** (decisión del usuario): con D1 dejan de importar sus formatos.
La escritura canónica evita seguir generando filas mezcladas, que es lo que hace frágil cualquier
comparación futura (incluido el `ORDER BY start_time` de SQL).

### D7 — El contrato de `expand_recurring` pasa a instantes

`expand_recurring` SHALL recibir `DateTime<FixedOffset>` ya parseados en vez de cadenas, y SHALL
aplicar solape a cada instancia (no «la instancia empieza dentro del rango»). Así los límites de
fecha suelta funcionan también para recurrentes y el criterio es idéntico al de las filas normales.

### D8 — Una fila con timestamps ilegibles se incluye (falla abierto)

Si `start_time`/`end_time` de una fila no se pueden parsear, la fila **SHALL incluirse** en el
resultado y SHALL ordenarse al final.

El razonamiento: el fallo que estamos arreglando es precisamente «un evento existente que no
aparece». Ocultar una fila por su formato repetiría el error en su forma más pura. Una fila ilegible
en pantalla es visible y depurable; una fila invisible no. Con D6, además, las filas nuevas ya no
pueden nacer con un formato ilegible, así que este camino solo cubre datos heredados o corruptos.

### D9 — El orden por instante también en `list_by_category`

`list_by_category` ordena hoy con `ORDER BY start_time ASC` (lexicográfico) sobre los mismos valores
mezclados. SHALL ordenar por instante de inicio con el mismo criterio que `list_by_date_range`, para
no dejar dos reglas de orden distintas en el mismo repositorio. Es el único cambio de este change
que no afecta a la visibilidad: sin él, el orden de una lista filtrada por categoría podría diferir
del de la lista general.

## Fuera de alcance (documentado, no arreglado)

- **`check_availability` deriva el día de los 10 primeros caracteres de `start`** (`&start[..10]`),
  es decir del día **UTC** del instante recibido. Si el modelo pasa `2026-10-07T01:00:00+02:00`
  (23:00Z del 6), busca huecos del día 7. Es un defecto real de la misma familia, pero vive en otra
  operación y no tiene relación con la visibilidad del calendario.
- **La incoherencia del contrato `CalendarEvent`** de la spec `frontend/agenda` (declara
  `start`/`end` y el tipo real es `start_time`/`end_time`).
- **La validación de coherencia temporal** (`end_time` posterior a `start_time`).

## Riesgos

- **Tests existentes que asumen contención.** `test_list_by_date_range_outside` sigue siendo válido
  (el evento del 25 de septiembre no solapa con el rango del 24). Hay que revisar los que afirmen
  exclusión por bordes y actualizar las llamadas que pasan cadenas.
- **Cambio de formato en la respuesta de escritura.** `POST /api/events` con `+02:00` devolverá `Z`.
  El frontend interpreta ambos con `dayjs`; no se espera impacto.
- **El 422 nuevo** puede romper un cliente que hoy envía basura y recibe una lista vacía. No hay
  ningún cliente así en el repositorio.

## Verificación

- Backend: `cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`.
- Frontend: `npx tsc --noEmit`, `npm run lint`, `npx vitest run`.
- Extremo a extremo contra la base de datos de producción: el rango de octubre de 2026 devuelve los
  16 eventos, incluidos los dos del 31 de octubre.
