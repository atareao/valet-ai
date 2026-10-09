## REMOVED Requirements

### Requirement: El registry SHALL omitir las herramientas deshabilitadas de las definiciones

### Requirement: El registry SHALL rechazar la ejecución de herramientas deshabilitadas

### Requirement: El estado habilitado SHALL proceder de la tabla tools y actualizarse en caliente

### Requirement: La herramienta `render_widget` SHALL gestionarse como el resto desde la tabla `tools`

### Requirement: El registry SHALL poder devolver las definiciones de un subconjunto

## ADDED Requirements

### Requirement: El registry SHALL exponer las definiciones de todas las herramientas registradas

`definitions()` SHALL devolver las definiciones de **todas** las herramientas registradas, sin filtrar por ningún estado de habilitación por herramienta. El filtrado por dominio SHALL ser responsabilidad del enrutador, que pide el subconjunto de las skills seleccionadas y habilitadas.

#### Scenario: Todas las herramientas registradas se ofrecen por el registro completo
- **Given** un registry con las herramientas `weather` y `tasks`
- **When** se solicitan las definiciones
- **Then** la lista contiene `weather` y `tasks`

### Requirement: El registry SHALL devolver las definiciones de un subconjunto pedido

El registry SHALL exponer un método que devuelva las definiciones correspondientes a una lista de nombres, en un **orden determinista por nombre**, de modo que la petición al LLM sea reproducible. Los nombres desconocidos SHALL ignorarse sin error. El subconjunto lo decide el enrutador (`core ∪ (skills seleccionadas ∩ habilitadas)`); el registry SHALL NOT filtrar por ningún estado de habilitación por herramienta.

#### Scenario: El subconjunto devuelve exactamente las definiciones pedidas
- **Given** un registry con `weather` y `tasks`
- **When** se solicitan las definiciones del subconjunto `weather` y `tasks`
- **Then** la lista contiene ambas

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
