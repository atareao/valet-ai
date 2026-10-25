# tools/agenda Specification

## Purpose
Tool de agenda de Valet: operaciones de eventos (crear, actualizar, eliminar, listar por categoría), esquema unificado basado en `start` + `duration`, soporte de campos nuevos y permiso de aprobación explícita para el borrado.

## Requirements

### Requirement: Calendar tool — add delete_event, list_by_category, support new fields

La tool `calendar` SHALL soportar `delete_event` y `list_by_category` y los campos `category`, `all_day`, `rrule` y `reminder_minutes_before`.

Extender la tool `calendar` con nuevas operaciones y soporte para los campos añadidos (category, all_day, rrule, reminder).

**Contracts:**

```rust
// Nuevas operaciones en CalendarTool

async fn delete_event(&self, args: Value) -> Result<ToolResult, ToolError>;
async fn list_by_category(&self, args: Value) -> Result<ToolResult, ToolError>;
```

```json
// Schema de parámetros expandido
{
  "type": "object",
  "properties": {
    "operation": {
      "type": "string",
      "enum": [
        "get_events",
        "check_availability",
        "create_event",
        "update_event",
        "delete_event",
        "list_by_category"
      ]
    },
    "profile_id": { "type": "string" },
    "duration": { "type": "integer" },
    "title": { "type": "string" },
    "start": { "type": "string" },
    "location": { "type": "string" },
    "scope": { "type": "string", "enum": ["shared", "personal"] },
    "id": { "type": "string" },
    "description": { "type": "string" },
    "category": { "type": "string", "enum": ["default", "work", "personal", "health", "birthday", "holiday"] },
    "all_day": { "type": "boolean" },
    "rrule": { "type": "string" },
    "reminder_minutes_before": { "type": "integer" }
  },
  "required": ["operation"]
}
```

**Scenarios:**

#### Scenario: Delete event via tool
Given an existing event
When `delete_event` is called with the event's id
Then the event is removed from the database
And success is returned

#### Scenario: Create event with all_day flag
When `create_event` is called with title "Cumpleaños Ana", all_day true, category "birthday"
Then the event is created with all_day = true
And category = "birthday"

#### Scenario: Create recurring event
When `create_event` is called with title "Reunión equipo", rrule "FREQ=WEEKLY;BYDAY=MO", start/end on Monday 10:00-11:00
Then the event is created with the rrule stored

#### Scenario: Create event with reminder
When `create_event` is called with reminder_minutes_before = 30
Then the event is stored with the reminder value

#### Scenario: List events filtered by category
Given events with categories "work" and "personal"
When `list_by_category` is called with category "work"
Then only work-category events are returned

#### Scenario: Get events expands recurring events
Given a weekly recurring event "Standup" (rrule FREQ=WEEKLY;BYDAY=MO,WE,FR)
When `get_events` is called with a date range covering 2 weeks
Then the result contains 6 event instances (3 per week)

#### Scenario: Delete non-existent event returns success
When `delete_event` is called with a non-existent id
Then it still returns success (idempotent)

### Requirement: Calendar tool — change delete_event permission to ExplicitApproval

La tool `calendar` SHALL declarar el permiso **por operación**, evaluando el parámetro `operation` de
los argumentos: las operaciones de lectura SHALL ser `NoConfirm`, la creación y la edición SHALL ser
`Notify`, y el borrado SHALL ser `ExplicitApproval`.

Eliminar eventos es destructivo — debe requerir confirmación del usuario antes de ejecutarse.

```rust
fn permission(&self, args: &Value) -> Permission {
    match args.get("operation").and_then(|v| v.as_str()) {
        // get_events, check_availability, list_by_category → NoConfirm
        // create_event, update_event → Notify
        // delete_event → ExplicitApproval
    }
}
```

**Scenarios:**

#### Scenario: Delete event requires explicit approval

**When** el LLM llama a `delete_event`
**Then** el guardrail devuelve `ExplicitApproval`
**And** el orquestador pausa para pedir confirmación al usuario
**And** solo ejecuta el borrado si el usuario aprueba

#### Scenario: Read operations do not require approval

**Given** una llamada a `get_events`, `check_availability` o `list_by_category`
**When** el guardrail consulta el permiso con esos argumentos
**Then** el permiso es `NoConfirm`

#### Scenario: Create and update are allowed with notification

**Given** una llamada a `create_event` o `update_event`
**When** el guardrail consulta el permiso con esos argumentos
**Then** el permiso es `Notify`
**And** la operación se ejecuta sin aprobación explícita

### Requirement: Calendar tool — update_event supports start_time/end_time

`update_event` SHALL aceptar y persistir `start` y `end` para reprogramar eventos.

The `update_event` operation must accept and persist `start` and `end` parameters so the LLM can reschedule events.

**Contracts:**

```rust
// EventsRepo::update — new signature
pub async fn update(
    pool: &SqlitePool,
    id: &str,
    title: Option<&str>,
    description: Option<&str>,
    location: Option<&str>,
    category: Option<&str>,
    all_day: Option<bool>,
    rrule: Option<&str>,
    reminder_minutes_before: Option<i32>,
    start_time: Option<&str>,   // NEW
    end_time: Option<&str>,     // NEW
) -> Result<bool, sqlx::Error>;
```

```rust
// UpdateEventRequest — new fields
pub struct UpdateEventRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub category: Option<String>,
    pub all_day: Option<bool>,
    pub rrule: Option<String>,
    pub reminder_minutes_before: Option<i32>,
    pub start_time: Option<String>,   // NEW
    pub end_time: Option<String>,     // NEW
}
```

**Scenarios:**

#### Scenario: Update event date via tool
Given an existing event on 2026-10-03
When `update_event` is called with id, start "2026-10-10T12:00:00Z", end "2026-10-10T13:30:00Z"
Then the event's start_time and end_time are updated to the new values
And `get_events` for 2026-10-03 returns no events
And `get_events` for 2026-10-10 returns the event

#### Scenario: Update event without changing dates (backward compat)
Given an existing event
When `update_event` is called with id and title only (no start/end)
Then the event's start_time and end_time remain unchanged
And the title is updated

#### Scenario: Update event via HTTP API
Given an existing event
When PUT /events/:id is called with start_time and end_time in the JSON body
Then the event's start_time and end_time are updated
And the response includes the updated event

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

### Requirement: Calendar tool — las escrituras SHALL guardar instantes en UTC canónico

Las operaciones de escritura de la tool `calendar` (`create_event` y `update_event`) SHALL guardar
`start_time` y `end_time` normalizados a UTC canónico (RFC 3339 con sufijo `Z`) antes de persistirlos.
Hoy `create_event` guarda `start` tal cual llega del modelo (mientras `end_time` ya se normaliza) y
`update_event` guarda `start` tal cual, de modo que en producción conviven filas con offset local
(`+02:00`) y filas con `Z`. Un `start` que no sea RFC 3339 SHALL producir
`ToolError::InvalidArguments`. `update_event` sin `start` SHALL dejar el `start_time` existente
intacto.

**Contracts:**

```rust
// create_event: start = "2026-10-06T19:00:00+02:00" → start_time = "2026-10-06T17:00:00Z"
//               end_time ya se normalizaba; se conserva.
// update_event: start = "2026-10-06T19:00:00+02:00" → start_time = "2026-10-06T17:00:00Z"
//               sin start → start_time no cambia.
```

**Scenarios:**

#### Scenario: create_event guarda el inicio en UTC canónico
**Given** un evento creado con `start: "2026-10-06T19:00:00+02:00"`
**When** se lee el evento de la base de datos
**Then** `start_time` SHALL ser `"2026-10-06T17:00:00Z"`

#### Scenario: update_event guarda el inicio en UTC canónico
**Given** un evento existente
**When** `update_event` se llama con `start: "2026-10-06T19:00:00+02:00"`
**Then** `start_time` SHALL quedar en `"2026-10-06T17:00:00Z"`

#### Scenario: update_event sin start conserva el inicio
**Given** un evento existente
**When** `update_event` se llama solo con `title`
**Then** `start_time` NO SHALL cambiar

#### Scenario: create_event con un inicio no parseable
**When** `create_event` se llama con `start: "no-es-una-fecha"`
**Then** SHALL devolver `ToolError::InvalidArguments`
