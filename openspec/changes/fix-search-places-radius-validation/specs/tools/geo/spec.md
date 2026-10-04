# Spec Delta: tools/geo

## ADDED Requirements

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
