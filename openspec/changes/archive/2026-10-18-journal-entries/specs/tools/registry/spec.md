## MODIFIED Requirements

### Requirement: El registry SHALL registrar todas las herramientas integradas sin nombres duplicados

La construcción del registry de producción SHALL registrar todas las herramientas integradas
(incluidas `notes`, `unified_search`, `journal`, `render_widget`, las cuatro herramientas `strava_*`, las tres
`timeline_*`, las cuatro `email_*`, las cuatro `rss_*` y la herramienta `notifications`), garantizando nombres únicos.

**Given** la aplicación Valet con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** figuran entre ellas `notes`, `unified_search`, `journal`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*` y la herramienta `notifications`
**And** no hay dos herramientas con el mismo nombre

#### Scenario: El registry de producción incluye todas las herramientas integradas

**Given** la aplicación Valet construida con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** `notes`, `unified_search`, `journal`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*` y la herramienta `notifications` figuran entre ellas
**And** no hay dos herramientas con el mismo nombre

#### Scenario: Las herramientas del correo se ofrecen al modelo

**Given** el registry de producción con la skill `email` habilitada
**When** se solicitan las definiciones del subconjunto `email_list_unread`, `email_get_body`, `email_mark_read` y `email_send`
**Then** la lista contiene las cuatro
**And** las tres de lectura y marcado declaran un permiso que no requiere aprobación
**And** `email_send` declara `ExplicitApproval`

#### Scenario: Las herramientas del feed se ofrecen al modelo

**Given** el registry de producción con la skill `rss` habilitada
**When** se solicitan las definiciones del subconjunto de las cuatro `rss_*`
**Then** la lista contiene las cuatro
**And** las cuatro declaran un permiso que no requiere aprobación
