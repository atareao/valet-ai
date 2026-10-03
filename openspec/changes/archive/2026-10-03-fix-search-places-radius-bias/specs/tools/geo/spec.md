# Spec Delta: tools/geo

## MODIFIED Requirements

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
