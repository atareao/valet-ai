# Spec Delta: tools/time-location

## ADDED Requirements

### Requirement: get_current_time SHALL format the time in the configured timezone

La tool `get_current_time` NO SHALL aceptar parámetros. SHALL leer la clave `timezone` de `settings`
(SettingsRepo) y SHALL formatear la hora actual con esa zona; si `timezone` no está configurada,
SHALL usar **Europe/Madrid** como valor por defecto. SHALL devolver la hora formateada en `data.time`
(y un `message`). Permiso `NoConfirm`.

**Given** una `CurrentTimeTool`
**When** se ejecuta `get_current_time`
**Then** DEBE leer `timezone` de `settings` y formatear la hora con ella
**And** si falta, DEBE usar `Europe/Madrid`

#### Scenario: Usa la timezone configurada
**Given** `settings.timezone = "Atlantic/Canary"`
**When** se ejecuta `get_current_time`
**Then** DEBE devolver la hora formateada en `Atlantic/Canary`

#### Scenario: Fallback a Europe/Madrid
**Given** `settings` sin `timezone`
**When** se ejecuta `get_current_time`
**Then** DEBE devolver la hora formateada en `Europe/Madrid`

### Requirement: get_current_location SHALL reverse-geocode the configured coordinates

La tool `get_current_location` NO SHALL aceptar parámetros. SHALL leer `latitude` y `longitude` de
`settings`; si ambas existen, SHALL resolver la dirección con `geo_utils::reverse_geocode` (Nominatim)
y devolver `data.location` combinando dirección y coordenadas (o solo coordenadas si no hay
dirección). Si faltan coordenadas, SHALL devolver «Ubicación no configurada.». Permiso `NoConfirm`.

**Given** una `CurrentLocationTool`
**When** se ejecuta `get_current_location`
**Then** DEBE leer `latitude`/`longitude` de `settings`
**And** DEBE devolver la ubicación resuelta o «Ubicación no configurada.»

#### Scenario: Con coordenadas configuradas
**Given** `settings.latitude` y `settings.longitude` con valores válidos
**When** se ejecuta `get_current_location`
**Then** DEBE llamar a `geo_utils::reverse_geocode` y devolver la ubicación formateada

#### Scenario: Sin coordenadas configuradas
**Given** `settings` sin `latitude` o sin `longitude`
**When** se ejecuta `get_current_location`
**Then** NO DEBE llamar a Nominatim
**And** DEBE devolver «Ubicación no configurada.»
