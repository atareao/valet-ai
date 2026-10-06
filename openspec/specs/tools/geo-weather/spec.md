# geo/weather tools spec

## Purpose

Especificación de las tools de geolocalización y meteorología: la tool de tiempo basada en latitud/longitud y las tools `geocode`, `reverse_geocode` y `search_places` separadas de la tool geo original.

## Requirements

### Requirement: Weather tool — remove operation, add required lat/lon

La tool de tiempo SHALL requerir `latitude` y `longitude` y SHALL NOT exponer un parámetro `operation`.

#### Scenario: Weather tool has no operation parameter
**Given** the weather tool definition
**When** `parameters()` is called
**Then** the schema must NOT have an `operation` property
**And** `required` must contain `["latitude", "longitude"]`

#### Scenario: Weather tool called with lat/lon returns data
**Given** a weather tool with a valid API key
**When** `execute({"latitude": 40.41, "longitude": -3.70})` is called
**Then** it succeeds and returns weather data

#### Scenario: Weather tool called missing lat returns error
**Given** a weather tool
**When** `execute({"longitude": -3.70})` is called (no latitude)
**Then** it returns `Err(ToolError::InvalidArguments)`

### Requirement: geocode tool (split from geo)

La tool `geocode` SHALL exponer únicamente el parámetro `query` para resolver nombres de lugar a coordenadas.

#### Scenario: geocode tool has only query parameter
**Given** the geocode tool definition
**When** `parameters()` is called
**Then** `required` contains `["query"]`
**And** `properties` only has `query` (string)

#### Scenario: geocode resolves a city name
**Given** a geocode tool
**When** `execute({"query": "Madrid"})` is called
**Then** it returns coordinates with lat ~40.41 and lon ~-3.70

### Requirement: reverse_geocode tool (split from geo)

La tool `reverse_geocode` SHALL requerir `latitude` y `longitude` para resolver coordenadas a una dirección.

#### Scenario: reverse_geocode tool has lat/lon parameters
**Given** the reverse_geocode tool definition
**When** `parameters()` is called
**Then** `required` contains `["latitude", "longitude"]`

### Requirement: search_places tool (split from geo)

La tool `search_places` SHALL requerir únicamente `query`; `latitude`, `longitude` y `radius` SHALL ser opcionales y usarse solo para sesgar geográficamente la búsqueda cuando estén presentes.

#### Scenario: search_places tool has query + lat/lon + radius

- **Given** la definición de la tool `search_places`
- **When** se llama a `parameters()`
- **Then** `properties` contiene `query`, `latitude`, `longitude` y `radius`
- **And** solo `query` figura en `required`

#### Scenario: search_places tool requires only query

- **Given** la definición de la tool `search_places`
- **When** se llama a `parameters()`
- **Then** `required` contiene `["query"]`
- **And** `latitude`, `longitude` y `radius` figuran en `properties` como opcionales
- **And** la descripción de la tool y la de sus parámetros están en español

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
