## ADDED Requirements

### Requirement: El registry SHALL poder devolver las definiciones de un subconjunto

El registry SHALL exponer un método que devuelva las definiciones correspondientes a una lista de nombres, incluyendo **únicamente** las que estén habilitadas y en un **orden determinista por nombre**, de modo que la petición al LLM sea reproducible. Los nombres desconocidos SHALL ignorarse sin error.

#### Scenario: El subconjunto omite las deshabilitadas
- **Given** un registry con `weather` y `tasks`, y `weather` deshabilitada
- **When** se solicitan las definiciones del subconjunto `weather` y `tasks`
- **Then** la lista contiene `tasks`
- **And** no contiene `weather`

#### Scenario: El orden es estable entre llamadas
- **Given** un subconjunto de varios nombres
- **When** se solicitan sus definiciones dos veces
- **Then** ambas llamadas devuelven el mismo orden
- **And** el orden es el alfabético por nombre

#### Scenario: Un nombre desconocido no es un error
- **Given** un subconjunto que incluye un nombre no registrado
- **When** se solicitan sus definiciones
- **Then** el nombre desconocido se ignora
- **And** el resto se devuelve con normalidad
