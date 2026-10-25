## MODIFIED Requirements

### Requirement: Calendar tool — unified schema: remove `end` and `date`

Las operaciones SHALL usar `start` + `duration` (minutos) para **crear o reprogramar** eventos,
eliminando `date` del schema. Como **rango de consulta**, `get_events` SHALL aceptar `start` más
`end` **o** `duration`: si viene `end` (ISO 8601 válido) SHALL usarse como límite superior del rango;
si vienen `end` y `duration`, `end` SHALL tener precedencia; si no viene ninguno de los dos,
`duration` SHALL valer 1440 minutos (un día). Un `end` con formato inválido SHALL producir
`ToolError::InvalidArguments`. El parámetro `end` SHALL declararse en el esquema de `parameters()` y
su descripción SHALL explicar que es una alternativa a `duration` en `get_events`. El parámetro
`title` SHALL documentarse como propio de `create_event` y **no** como filtro de `get_events`.

**Contracts:**

```rust
// get_events: start + (end | duration) definen el rango de búsqueda
//   end      = "2026-12-01T23:59:59Z"  → rango start ..= end
//   duration = 1440                     → end = start + 1 día (por defecto)
//   ambos    → gana `end`

// check_availability: start + duration definen la duración del slot buscado
//   start = "2026-09-26T09:00:00Z", duration = 60 → slots libres de 1h desde las 9

// update_event: duration recalcula end. Si solo start, mantiene duración original.
//   Si solo duration, mantiene start original y recalcula end.
```

**Parameter schema:**

```json
{
  "type": "object",
  "properties": {
    "operation": { "type": "string", "enum": ["get_events", "check_availability", "create_event", "update_event", "delete_event", "list_by_category"] },
    "title": { "type": "string", "description": "Título del evento — obligatorio en create_event (no es filtro de get_events)" },
    "start": { "type": "string", "description": "ISO 8601. Inicio del evento/rango. Ej: 2026-09-26T21:00:00Z" },
    "end": { "type": "string", "description": "ISO 8601. Fin del rango en get_events — alternativa a duration; tiene precedencia si vienen ambos" },
    "duration": { "type": "integer", "description": "Duración en minutos. Ej: 60 = 1h, 1440 = 1 día. Opcional en get_events (default: 1440)" },
    "id": { "type": "string", "description": "ID del evento — obligatorio en update_event y delete_event" },
    "category": { "type": "string", "enum": ["default", "work", "personal", "health", "birthday", "holiday"] },
    "scope": { "type": "string", "enum": ["shared", "personal"] },
    "location": { "type": "string" },
    "description": { "type": "string" },
    "all_day": { "type": "boolean" },
    "rrule": { "type": "string" },
    "reminder_minutes_before": { "type": "integer" }
  },
  "required": ["operation"]
}
```

**Scenarios:**

#### Scenario: create_event with start + duration
When `create_event` is called with `start: "2026-09-26T21:00:00Z"`, `duration: 120`
Then the event is created with `end_time = "2026-09-26T23:00:00Z"`

#### Scenario: get_events with start + duration
When `get_events` is called with `start: "2026-09-26T00:00:00Z"`, `duration: 1440`
Then events on 2026-09-26 are returned

#### Scenario: get_events with start + end
**Given** un evento en `start_time` dentro de un rango de varias semanas
**When** `get_events` se llama con `start` y `end` que abarcan todo el rango
**Then** SHALL devolverse el evento (el rango NO se colapsa al día por defecto)

#### Scenario: get_events da precedencia a end sobre duration
**Given** un evento más allá del `duration` indicado pero dentro de `end`
**When** `get_events` se llama con `start`, `duration` y `end`
**Then** SHALL usarse `end` como límite superior y el evento SHALL aparecer

#### Scenario: get_events sin end ni duration usa 1440
**When** `get_events` se llama solo con `start`
**Then** el rango SHALL ser `start .. start + 1440 minutos` (comportamiento por defecto)

#### Scenario: get_events con end inválido
**When** `get_events` se llama con `end: "no-es-una-fecha"`
**Then** SHALL devolver `ToolError::InvalidArguments`

#### Scenario: get_events con límites en offset distinto de UTC
**Given** un evento almacenado normalizado a UTC (sufijo `Z`)
**When** `get_events` se llama con `start` o `end` en RFC 3339 con offset (p. ej. `+02:00` o `-05:00`)
**Then** ambos límites del rango SHALL normalizarse a UTC antes de la comparación
**And** el evento SHALL incluirse o excluirse según el instante real, no según el orden lexicográfico de las cadenas

#### Scenario: update_event with duration
When `update_event` is called with `id`, `duration: 90`
Then the event's end_time is recomputed as start_time + 90 minutes

#### Scenario: check_availability with start + duration
When `check_availability` is called with `start: "2026-09-26T09:00:00Z"`, `duration: 60`
Then free slots of 60 minutes are returned
