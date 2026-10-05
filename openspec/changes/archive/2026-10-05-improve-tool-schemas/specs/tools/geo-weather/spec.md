# Spec Delta

## MODIFIED Requirements

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
