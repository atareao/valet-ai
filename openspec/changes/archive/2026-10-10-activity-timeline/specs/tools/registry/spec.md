## MODIFIED Requirements

### Requirement: El registry SHALL registrar todas las herramientas integradas sin nombres duplicados

La construcción del registry de producción SHALL registrar todas las herramientas integradas
(incluidas `notes`, `unified_search`, `render_widget`, las cuatro herramientas `strava_*` y las tres
`timeline_*`), garantizando nombres únicos.

**Given** la aplicación Valet con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** figuran entre ellas `notes`, `unified_search`, `render_widget`, las cuatro `strava_*` y las tres `timeline_*`
**And** no hay dos herramientas con el mismo nombre

#### Scenario: El registry de producción incluye todas las herramientas integradas

**Given** la aplicación Valet construida con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** `notes`, `unified_search`, `render_widget`, las cuatro `strava_*` y las tres `timeline_*` figuran entre ellas
**And** no hay dos herramientas con el mismo nombre
