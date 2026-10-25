# events-api Specification

## Purpose
API REST de eventos de Valet: endpoints CRUD para listar por rango de fechas, crear, actualizar y eliminar eventos.

## Requirements

### Requirement: Events REST API CRUD endpoints

La API SHALL exponer endpoints CRUD bajo `/api/events` para listar por rango de fechas, crear,
actualizar y eliminar eventos.

Los límites `start` y `end` del listado SHALL aceptarse en RFC 3339 (con offset o sufijo `Z`) **o**
como fecha suelta `YYYY-MM-DD`, SHALL interpretarse como **instantes** y SHALL definir un intervalo
**semiabierto** `[start, end)`. Una fecha suelta SHALL cubrir el día UTC completo: como límite
inferior, su `T00:00:00Z`; como límite superior, el **inicio del día siguiente**. Un límite ausente,
vacío o no parseable SHALL producir `422 Unprocessable Entity`. El listado SHALL devolver los eventos
cuyo intervalo **se solape** con el rango consultado (`start_time < end AND end_time > start`) y
SHALL ordenarlos por instante de inicio. La comparación SHALL resolverse sobre instantes, **nunca**
sobre el orden lexicográfico de las cadenas almacenadas. La creación y la actualización SHALL
normalizar `start_time` y `end_time` a UTC canónico (RFC 3339 con sufijo `Z`) antes de guardarlos y
SHALL producir `422 Unprocessable Entity` si alguno no es RFC 3339.

**Contracts:**

```
GET    /api/events?start=<rfc3339|YYYY-MM-DD>&end=<rfc3339|YYYY-MM-DD>
       → 200 [Event]   eventos que solapan con [start, end), ordenados por instante de inicio
       → 422           límite ausente, vacío o no parseable

POST   /api/events      → 201 Event   (start_time/end_time normalizados a UTC canónico)
                        → 422         start_time/end_time ausentes o no RFC 3339
PUT    /api/events/:id  → 200 Event   (start_time/end_time normalizados a UTC canónico si vienen)
                        → 422         start_time/end_time presentes pero no RFC 3339
DELETE /api/events/:id  → 204
```

#### Scenario: List events in date range
**Given** un evento existe con start_time="2026-09-25T10:00:00Z" en la BD
**When** se hace GET `/api/events?start=2026-09-25T00:00:00Z&end=2026-09-25T23:59:59Z`
**Then** devuelve 200 con un array que contiene ese evento

#### Scenario: List events — empty range
**Given** no hay eventos en la BD
**When** se hace GET `/api/events?start=2026-09-25T00:00:00Z&end=2026-09-25T23:59:59Z`
**Then** devuelve 200 con un array vacío

#### Scenario: List events — missing query params
**When** se hace GET `/api/events`
**Then** devuelve 422 Unprocessable Entity

#### Scenario: List events — un límite no parseable
**When** se hace GET `/api/events?start=2026-09-25&end=no-es-una-fecha`
**Then** devuelve 422 Unprocessable Entity

#### Scenario: List events — un límite de fecha suelta cubre el último día
**Given** un evento existe con start_time="2026-10-31T08:00:00Z" y end_time="2026-10-31T09:00:00Z"
**When** se hace GET `/api/events?start=2026-09-27&end=2026-10-31`
**Then** devuelve 200 con un array que contiene ese evento

#### Scenario: List events — un límite con offset distinto de UTC
**Given** un evento existe con start_time="2026-10-31T08:00:00Z" en la BD
**When** se hace GET `/api/events?start=2026-10-31T00:00:00%2B02:00&end=2026-10-31T23:59:59%2B01:00`
**Then** devuelve 200 con un array que contiene ese evento

#### Scenario: List events — solape parcial
**Given** un evento existe con start_time="2026-09-26T23:00:00Z" y end_time="2026-09-27T01:00:00Z"
**When** se hace GET `/api/events?start=2026-09-27T00:00:00Z&end=2026-09-28T00:00:00Z`
**Then** devuelve 200 con un array que contiene ese evento

#### Scenario: Create event
**Given** un payload JSON válido con title, start_time, end_time
**When** se hace POST `/api/events`
**Then** devuelve 201 con el evento creado (incluyendo id, created_at, updated_at)

#### Scenario: Create event — normaliza los instantes a UTC canónico
**Given** un payload JSON con start_time="2026-10-06T19:00:00+02:00" y end_time="2026-10-06T20:00:00+02:00"
**When** se hace POST `/api/events`
**Then** devuelve 201 con start_time="2026-10-06T17:00:00Z" y end_time="2026-10-06T18:00:00Z"

#### Scenario: Create event — start_time no parseable
**Given** un payload JSON con start_time="no-es-una-fecha"
**When** se hace POST `/api/events`
**Then** devuelve 422 Unprocessable Entity

#### Scenario: Create event — missing required fields
**When** se hace POST `/api/events` con body `{}`
**Then** devuelve 422 Unprocessable Entity

#### Scenario: Update event
**Given** un evento existe con id="evt-1"
**When** se hace PUT `/api/events/evt-1` con body `{"title": "Updated"}`
**Then** devuelve 200 con el evento actualizado

#### Scenario: Update event — normaliza los instantes a UTC canónico
**Given** un evento existe con id="evt-1"
**When** se hace PUT `/api/events/evt-1` con body `{"start_time": "2026-10-06T19:00:00+02:00"}`
**Then** devuelve 200 con start_time="2026-10-06T17:00:00Z"

#### Scenario: Update event — not found
**When** se hace PUT `/api/events/nonexistent` con body `{"title": "X"}`
**Then** devuelve 404

#### Scenario: Delete event
**Given** un evento existe con id="evt-1"
**When** se hace DELETE `/api/events/evt-1`
**Then** devuelve 204 No Content

#### Scenario: Delete event — not found (idempotent)
**When** se hace DELETE `/api/events/nonexistent`
**Then** devuelve 204 No Content (idempotent)
