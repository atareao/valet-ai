# Spec Delta

## MODIFIED Requirements

### Requirement: Google Places API integration

La tool SHALL leer `google_places_api_key` de settings y llamar a la Google Places New API con las cabeceras `X-Goog-Api-Key` y `X-Goog-FieldMask`. La tool SHALL usar siempre el endpoint `places:searchText` y enviar la consulta como `textQuery`; NO SHALL enviar el texto libre del usuario como `includedTypes` ni `includedType`. `query` SHALL ser el único argumento obligatorio; `latitude` y `longitude` SHALL ser opcionales. Cuando la llamada incluya `latitude`, `longitude` y un `radius` válido, SHALL añadir `locationBias.circle` con el centro (lat/lon) y el radio en metros; en caso contrario NO SHALL incluir `locationBias` y SHALL NOT llamar a `places:searchNearby`.

#### Scenario: Búsqueda por texto (searchText)

- **Given** un `SearchPlacesTool` con API key en settings DB
- **When** se ejecuta con `{"query": "restaurantes en Madrid", "latitude": 40.4168, "longitude": -3.7038}` (sin `radius`)
- **Then** la tool DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
- **And** DEBE incluir `textQuery` con el valor de `query` y `languageCode: "es"` en el body
- **And** NO DEBE incluir `locationBias` en el body
- **And** NO DEBE enviar `query` como `includedTypes`
- **And** DEBE devolver los lugares parseados y el enlace a Google Maps

#### Scenario: Nombre de negocio con radius

- **Given** un `SearchPlacesTool` con API key en settings DB
- **When** se ejecuta con `{"query": "Querida Jacinta restaurante", "latitude": 39.4676, "longitude": -0.3771, "radius": 5000}`
- **Then** la tool DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
- **And** DEBE incluir `textQuery` con el valor de `query`
- **And** DEBE incluir `locationBias.circle` con `center` `{latitude: 39.4676, longitude: -0.3771}` y `radius: 5000`
- **And** NO DEBE incluir `includedTypes` en el body
- **And** DEBE devolver los lugares parseados y el enlace a Google Maps

#### Scenario: Búsqueda por cercanía (searchNearby)

- **Given** un `SearchPlacesTool` con API key en settings DB
- **When** se ejecuta con `{"query": "cafe", "latitude": 40.4168, "longitude": -3.7038, "radius": 500}`
- **Then** la tool DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
- **And** DEBE incluir `textQuery` con el valor de `query`
- **And** DEBE incluir `locationBias.circle` con `center` y `radius: 500`
- **And** NO DEBE llamar a `places:searchNearby`
- **And** NO DEBE incluir `includedTypes` en el body

#### Scenario: Búsqueda por texto sin coordenadas

- **Given** un `SearchPlacesTool` con API key en settings DB
- **When** se ejecuta con `{"query": "museos en Madrid"}` (sin `latitude`, `longitude` ni `radius`)
- **Then** DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
- **And** DEBE incluir `textQuery` con el valor de `query` y `languageCode: "es"`
- **And** NO DEBE incluir `locationBias` ni `includedTypes`
- **And** DEBE devolver los lugares parseados y el enlace a Google Maps

#### Scenario: Error de API key inválida

- **Given** un `SearchPlacesTool` con API key inválida en settings DB
- **When** se ejecuta `search_places`
- **Then** DEBE retornar `Err(ToolError::ExecutionError)` con mensaje del error HTTP

#### Scenario: Sin API key configurada

- **Given** un `SearchPlacesTool` sin API key en settings DB ni en Config/ENV
- **When** se ejecuta `search_places`
- **Then** DEBE retornar `Err(ToolError::ExecutionError)` indicando que falta la API key

### Requirement: search_places SHALL validate and clamp the radius argument

La tool `search_places` SHALL validar el argumento opcional `radius` antes de construir el `locationBias`, y SHALL aplicar `locationBias` únicamente cuando `latitude` y `longitude` estén presentes. Si `radius` es numérico (entero o decimal) y estrictamente mayor que 0, SHALL redondearlo al entero más cercano y acotarlo al rango `[1, 50000]` metros antes de enviarlo. Si `radius` es 0, negativo, `null` o no numérico, o si falta `latitude` o `longitude`, la tool SHALL ignorarlo y NO SHALL incluir `locationBias`, comportándose como una llamada sin `radius`.

#### Scenario: Radio por encima del máximo

- **Given** un `SearchPlacesTool` con API key en settings DB
- **When** se ejecuta con `{"query": "restaurantes", "latitude": 40.4168, "longitude": -3.7038, "radius": 100000}`
- **Then** DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
- **And** DEBE incluir `locationBias.circle.radius` con el valor `50000`

#### Scenario: Radio decimal

- **Given** un `SearchPlacesTool` con API key en settings DB
- **When** se ejecuta con `{"query": "cafe", "latitude": 40.4168, "longitude": -3.7038, "radius": 2500.6}`
- **Then** DEBE incluir `locationBias.circle.radius` con el valor `2501`

#### Scenario: Radio no numérico

- **Given** un `SearchPlacesTool` con API key en settings DB
- **When** se ejecuta con `{"query": "cafe", "latitude": 40.4168, "longitude": -3.7038, "radius": "cerca"}`
- **Then** DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
- **And** NO DEBE incluir `locationBias` en el body

#### Scenario: Radio cero o negativo

- **Given** un `SearchPlacesTool` con API key en settings DB
- **When** se ejecuta con `{"query": "cafe", "latitude": 40.4168, "longitude": -3.7038, "radius": 0}`
- **Then** NO DEBE incluir `locationBias` en el body

#### Scenario: Radio sin coordenadas no aplica sesgo

- **Given** un `SearchPlacesTool` con API key en settings DB
- **When** se ejecuta con `{"query": "cafe", "radius": 500}`
- **Then** DEBE llamar a `POST https://places.googleapis.com/v1/places:searchText`
- **And** NO DEBE incluir `locationBias` en el body
