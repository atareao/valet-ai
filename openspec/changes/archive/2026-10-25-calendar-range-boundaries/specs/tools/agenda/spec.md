## ADDED Requirements

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
