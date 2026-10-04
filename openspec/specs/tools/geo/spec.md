# tools/geo Specification

## Purpose
Búsqueda de lugares y geocodificación usando Google Places API (New API) y Nominatim (OSM).

## Requirements

### Requirement: Google Places API integration

La tool SHALL leer `google_places_api_key` de settings y llamar a la Google Places New API con las
cabeceras `X-Goog-Api-Key` y `X-Goog-FieldMask`. La tool SHALL usar siempre el endpoint
`places:searchText` y enviar la consulta como `textQuery`; NO SHALL enviar el texto libre del usuario
como `includedTypes` ni `includedType`. Cuando la llamada incluya `radius`, SHALL añadir
`locationBias.circle` con el centro (lat/lon) y el radio en metros, y SHALL NOT llamar a
`places:searchNearby`.

**Given** un `SearchPlacesTool`
**When** se ejecuta `search_places` con query, lat, lon y un `radius` opcional
**Then** la tool DEBE leer `google_places_api_key` de la tabla `settings` (SettingsRepo)
**And** DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
**And** DEBE incluir `textQuery` con el valor de `query` y `languageCode: "es"` en el body
**And** NO DEBE enviar `query` como `includedTypes` ni `includedType`
**And** DEBE incluir `X-Goog-Api-Key` y `X-Goog-FieldMask` en los headers
**And** DEBE devolver los lugares parseados y el enlace a Google Maps en `ToolResult.data`

#### Scenario: Búsqueda por texto (searchText)
**Given** un `SearchPlacesTool` con API key en settings DB
**When** se ejecuta con `{"query": "restaurantes en Madrid", "latitude": 40.4168, "longitude": -3.7038}` (sin `radius`)
**Then** la tool DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
**And** DEBE incluir `textQuery` con el valor de `query` y `languageCode: "es"` en el body
**And** NO DEBE incluir `locationBias` en el body
**And** NO DEBE enviar `query` como `includedTypes`
**And** DEBE devolver los lugares parseados y el enlace a Google Maps

#### Scenario: Nombre de negocio con radius
**Given** un `SearchPlacesTool` con API key en settings DB
**When** se ejecuta con `{"query": "Querida Jacinta restaurante", "latitude": 39.4676, "longitude": -0.3771, "radius": 5000}`
**Then** la tool DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
**And** DEBE incluir `textQuery` con el valor de `query`
**And** DEBE incluir `locationBias.circle` con `center` `{latitude: 39.4676, longitude: -0.3771}` y `radius: 5000`
**And** NO DEBE incluir `includedTypes` en el body
**And** DEBE devolver los lugares parseados y el enlace a Google Maps

#### Scenario: Búsqueda por cercanía (searchNearby)
**Given** un `SearchPlacesTool` con API key en settings DB
**When** se ejecuta con `{"query": "cafe", "latitude": 40.4168, "longitude": -3.7038, "radius": 500}`
**Then** la tool DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
**And** DEBE incluir `textQuery` con el valor de `query`
**And** DEBE incluir `locationBias.circle` con `center` y `radius: 500`
**And** NO DEBE llamar a `places:searchNearby`
**And** NO DEBE incluir `includedTypes` en el body

#### Scenario: Error de API key inválida
**Given** un `SearchPlacesTool` con API key inválida en settings DB
**When** se ejecuta `search_places`
**Then** DEBE retornar `Err(ToolError::ExecutionError)` con mensaje del error HTTP

#### Scenario: Sin API key configurada
**Given** un `SearchPlacesTool` sin API key en settings DB ni en Config/ENV
**When** se ejecuta `search_places`
**Then** DEBE retornar `Err(ToolError::ExecutionError)` indicando que falta la API key

### Requirement: Estructura Place con campos de Google Places

La deserialización SHALL mapear la respuesta de Google Places a la estructura `Place` con campos opcionales.

**Given** la respuesta de Google Places API
**When** se deserializa
**Then** DEBE mapear a la estructura `Place` con los siguientes campos:
- `id: Option<String>`
- `display_name: Option<LocalizedText>` (text + language_code)
- `formatted_address: Option<String>`
- `location: Option<LatLng>` (latitude + longitude)
- `rating: Option<f64>`
- `user_rating_count: Option<u64>`
- `price_level: Option<String>`
- `website_uri: Option<String>`
- `national_phone_number: Option<String>`
- `regular_opening_hours: Option<OpeningHours>` (open_now + weekday_descriptions)
- `primary_type: Option<String>`
- `types: Option<Vec<String>>`
- `editorial_summary: Option<LocalizedText>`

#### Scenario: Deserialización de Place completo
**Given** un JSON de respuesta de Google Places con todos los campos
**When** se deserializa a `PlacesResponse`
**Then** `places` contiene `Vec<Place>` con todos los campos mapeados correctamente

#### Scenario: Deserialización con campos nulos
**Given** un JSON de respuesta con campos opcionales ausentes
**When** se deserializa a `PlacesResponse`
**Then** los campos ausentes DEBEN ser `None` (no panic)

### Requirement: FIELD_MASK para minimizar coste

La constante `FIELD_MASK` SHALL listar los campos de Google Places usados por la tool para minimizar el coste de la API.

**Given** la constante `FIELD_MASK`
**When** se usa en requests a Google Places
**Then** DEBE incluir: `places.id`, `places.displayName`, `places.formattedAddress`, `places.location`, `places.rating`, `places.userRatingCount`, `places.priceLevel`, `places.websiteUri`, `places.nationalPhoneNumber`, `places.regularOpeningHours`, `places.primaryType`, `places.types`, `places.editorialSummary`

#### Scenario: FIELD_MASK incluye los campos usados
**Given** la constante `FIELD_MASK`
**When** se usa en un request a Google Places
**Then** incluye `places.id`, `places.displayName` y `places.formattedAddress`

### Requirement: Enlace a Google Maps

`maps_link(places)` SHALL generar un enlace `https://www.google.com/maps/dir/...` filtrando los lugares sin coordenadas.

**Given** un `SearchPlacesTool` con una lista de `Place`
**When** se llama a `maps_link(places)`
**Then** DEBE generar un URL `https://www.google.com/maps/dir/{lat,lon|lat,lon...}`
**And** DEBE filtrar lugares sin coordenadas
**And** DEBE retornar string vacía si no hay lugares con coordenadas

#### Scenario: Maps link con un solo lugar
**Given** un lugar con coordenadas (40.3520, 18.1715)
**When** se genera maps_link
**Then** retorna `"https://www.google.com/maps/dir/40.35200,18.17150"`

#### Scenario: Maps link con múltiples lugares
**Given** dos lugares con coordenadas
**When** se genera maps_link
**Then** retorna URLs separados por `|`

#### Scenario: Maps link vacío
**Given** lista vacía de lugares
**When** se genera maps_link
**Then** retorna `""`

### Requirement: GeocodeTool (Nominatim)

`GeocodeTool` SHALL llamar a la API de Nominatim `/search` y devolver coordenadas y dirección formateada.

**Given** un `GeocodeTool`
**When** se ejecuta `geocode` con una dirección
**Then** DEBE llamar a Nominatim API (`/search`)
**And** DEBE devolver coordenadas y dirección formateada

#### Scenario: Geocode exitoso
**Given** un `GeocodeTool`
**When** se ejecuta con `{"query": "Plaza Mayor, Madrid"}`
**Then** DEBE retornar `ToolResult` con `latitude`, `longitude`, `display_name`, `address`

#### Scenario: Geocode sin resultados
**Given** un `GeocodeTool`
**When** se ejecuta con una dirección inexistente
**Then** DEBE retornar `Err(ToolError::NotFound)`

### Requirement: ReverseGeocodeTool (Nominatim)

`ReverseGeocodeTool` SHALL llamar a la API de Nominatim `/reverse` y devolver la dirección formateada.

**Given** un `ReverseGeocodeTool`
**When** se ejecuta `reverse_geocode` con coordenadas
**Then** DEBE llamar a Nominatim API (`/reverse`)
**And** DEBE devolver la dirección formateada

#### Scenario: Reverse geocode exitoso
**Given** un `ReverseGeocodeTool`
**When** se ejecuta con `{"latitude": 40.4155, "longitude": -3.7074}`
**Then** DEBE retornar `ToolResult` con dirección formateada

#### Scenario: Reverse geocode sin resultados
**Given** un `ReverseGeocodeTool`
**When** se ejecuta con coordenadas en medio del océano
**Then** DEBE retornar `Err(ToolError::NotFound)`

### Requirement: search_places SHALL validate and clamp the radius argument

La tool `search_places` SHALL validar el argumento opcional `radius` antes de construir el
`locationBias`. Si `radius` es numérico (entero o decimal) y estrictamente mayor que 0, SHALL
redondearlo al entero más cercano y acotarlo al rango `[1, 50000]` metros antes de enviarlo. Si
`radius` es 0, negativo, `null` o no numérico, la tool SHALL ignorarlo y NO SHALL incluir
`locationBias`, comportándose como una llamada sin `radius`.

**Given** un `SearchPlacesTool` con API key en settings DB
**When** se ejecuta `search_places` con lat, lon y un `radius` opcional
**Then** DEBE validar `radius` y, si procede, acotarlo a `[1, 50000]` en `locationBias.circle.radius`
**And** con un `radius` inválido NO DEBE incluir `locationBias`

#### Scenario: Radio por encima del máximo
**Given** un `SearchPlacesTool` con API key en settings DB
**When** se ejecuta con `{"query": "restaurantes", "latitude": 40.4168, "longitude": -3.7038, "radius": 100000}`
**Then** DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
**And** DEBE incluir `locationBias.circle.radius` con el valor `50000`

#### Scenario: Radio decimal
**Given** un `SearchPlacesTool` con API key en settings DB
**When** se ejecuta con `{"query": "cafe", "latitude": 40.4168, "longitude": -3.7038, "radius": 2500.6}`
**Then** DEBE incluir `locationBias.circle.radius` con el valor `2501`

#### Scenario: Radio no numérico
**Given** un `SearchPlacesTool` con API key en settings DB
**When** se ejecuta con `{"query": "cafe", "latitude": 40.4168, "longitude": -3.7038, "radius": "cerca"}`
**Then** DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
**And** NO DEBE incluir `locationBias` en el body

#### Scenario: Radio cero o negativo
**Given** un `SearchPlacesTool` con API key en settings DB
**When** se ejecuta con `{"query": "cafe", "latitude": 40.4168, "longitude": -3.7038, "radius": 0}`
**Then** NO DEBE incluir `locationBias` en el body
