# Change: timeout HTTP en la tool `weather`

## Why

`WeatherTool` construye su cliente con `reqwest::Client::new()`, **sin timeout**. Si OpenWeather
tarda o se cuelga, `get_current_weather` / `get_forecast` esperan indefinidamente: el turno nunca
termina ni emite `tool_result`, y el stream queda bloqueado. Otras tools del mismo módulo ya fijan un
timeout explícito (p. ej. `ReverseGeocodeTool`, 30 s, añadido en `geo-timeout-fix`); `weather` quedó
fuera de aquel arreglo. Es una asimetría con impacto directo en la disponibilidad del turno.

## What Changes

- **Cliente con timeout.** El cliente HTTP de `WeatherTool` SHALL construirse con
  `reqwest::Client::builder().timeout(...)` (30 s, alineado con geo) en lugar de `Client::new()`.
- **Fallo acotado.** Una petición que exceda el timeout SHALL devolver
  `Err(ToolError::ExecutionError)` con un mensaje que mencione el fallo, en vez de colgarse.

## Capabilities

### Modified Capabilities
- `tools/geo-weather`: nuevo requirement `Weather tool — HTTP timeout`.

## Impact

- Backend: `src/tools/weather.rs` (`WeatherTool::new`).
- Tests: unitario del cliente/timeout.
- Sin cambios de esquema, de contrato LLM ni de firma pública. `docker-compose.prod.yml` no se toca.

### Fuera de alcance

- Reintentos o circuit breaker sobre OpenWeather: solo se acota el timeout.
- H5 (`search_places` con `searchText`): change aparte.
