# Spec Delta: tools/geo

## MODIFIED Requirements

### Requirement: Google Places API integration

La tool SHALL leer `google_places_api_key` de settings y llamar a la Google Places New API con las
cabeceras `X-Goog-Api-Key` y `X-Goog-FieldMask`. La elección de endpoint SHALL depender de los
argumentos: si la llamada incluye `radius`, SHALL usar `places:searchNearby`; en caso contrario,
SHALL usar `places:searchText` y enviar la consulta como `textQuery`.

**Given** un `SearchPlacesTool`
**When** se ejecuta `search_places` con query, lat, lon y radius
**Then** la tool DEBE leer `google_places_api_key` de la tabla `settings` (SettingsRepo)
**And** DEBE llamar a la Google Places New API (`places:searchNearby` o `places:searchText`) según los argumentos
**And** DEBE incluir `X-Goog-Api-Key` y `X-Goog-FieldMask` en los headers
**And** DEBE devolver los lugares parseados y el enlace a Google Maps en `ToolResult.data`

#### Scenario: Búsqueda por texto (searchText)
**Given** un `SearchPlacesTool` con API key en settings DB
**When** se ejecuta con `{"query": "restaurantes en Madrid", "latitude": 40.4168, "longitude": -3.7038}` (sin `radius`)
**Then** la tool DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
**And** DEBE incluir `textQuery` con el valor de `query` y `languageCode: "es"` en el body
**And** NO DEBE enviar `query` como `includedTypes`
**And** DEBE devolver los lugares parseados y el enlace a Google Maps

#### Scenario: Búsqueda por cercanía (searchNearby)
**Given** un `SearchPlacesTool` con API key en settings DB
**When** se ejecuta con `{"query": "cafe", "latitude": 40.4168, "longitude": -3.7038, "radius": 500}`
**Then** la tool DEBE llamar a `POST https://places.googleapis.com/v1/places:searchNearby`
**And** DEBE incluir `locationRestriction.circle` con center y radius en el body
**And** DEBE devolver los lugares parseados y el enlace a Google Maps

#### Scenario: Error de API key inválida
**Given** un `SearchPlacesTool` con API key inválida en settings DB
**When** se ejecuta `search_places`
**Then** DEBE retornar `Err(ToolError::ExecutionError)` con mensaje del error HTTP

#### Scenario: Sin API key configurada
**Given** un `SearchPlacesTool` sin API key en settings DB ni en Config/ENV
**When** se ejecuta `search_places`
**Then** DEBE retornar `Err(ToolError::ExecutionError)` indicando que falta la API key
