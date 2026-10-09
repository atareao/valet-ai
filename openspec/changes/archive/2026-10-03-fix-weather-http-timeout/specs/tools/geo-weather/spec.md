# Spec Delta: tools/geo-weather

## ADDED Requirements

### Requirement: Weather tool — HTTP timeout

La tool de tiempo SHALL construir su cliente HTTP con un timeout explícito y SHALL fallar con un
error acotado, en vez de colgarse, cuando OpenWeather no responde.

#### Scenario: El cliente tiene un timeout acotado
**Given** un `WeatherTool`
**When** se construye
**Then** su cliente HTTP DEBE haberse creado con un timeout explícito (30 s)

#### Scenario: OpenWeather no responde
**Given** un `WeatherTool` cuyo cliente HTTP expira
**When** `execute` llama a OpenWeather y la petición supera el timeout
**Then** DEBE devolver `Err(ToolError::ExecutionError)` con un mensaje que mencione el fallo
**And** NO DEBE bloquear el turno indefinidamente
