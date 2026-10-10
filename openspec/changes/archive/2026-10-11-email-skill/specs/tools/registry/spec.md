## MODIFIED Requirements

### Requirement: Las operaciones destructivas SHALL requerir aprobación explícita

Toda operación de borrado de una tool multioperación SHALL declarar `ExplicitApproval`. En concreto, la tool `tasks` SHALL devolver `ExplicitApproval` para `delete_task` y `NoConfirm` para el resto de sus operaciones. La tool `email_send` SHALL declarar `ExplicitApproval`, porque enviar correo es irreversible y sale del sistema.

#### Scenario: delete_task requiere aprobación explícita

**Given** un registry con la tool `tasks`
**When** se consulta su permiso con `{"operation": "delete_task", "id": "..."}`
**Then** el permiso es `ExplicitApproval`

#### Scenario: list_tasks no requiere aprobación

**Given** un registry con la tool `tasks`
**When** se consulta su permiso con `{"operation": "list_tasks"}`
**Then** el permiso es `NoConfirm`

#### Scenario: email_send requiere aprobación explícita

**Given** el registry de producción con la tool `email_send`
**When** se consulta su permiso con unos argumentos cualesquiera
**Then** el permiso es `ExplicitApproval`

### Requirement: El registry SHALL registrar todas las herramientas integradas sin nombres duplicados

La construcción del registry de producción SHALL registrar todas las herramientas integradas
(incluidas `notes`, `unified_search`, `render_widget`, las cuatro herramientas `strava_*`, las tres
`timeline_*` y las cuatro `email_*`), garantizando nombres únicos.

**Given** la aplicación Valet con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** figuran entre ellas `notes`, `unified_search`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*` y las cuatro `email_*`
**And** no hay dos herramientas con el mismo nombre

#### Scenario: El registry de producción incluye todas las herramientas integradas

**Given** la aplicación Valet construida con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** `notes`, `unified_search`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*` y las cuatro `email_*` figuran entre ellas
**And** no hay dos herramientas con el mismo nombre

#### Scenario: Las herramientas del correo se ofrecen al modelo

**Given** el registry de producción con la skill `email` habilitada
**When** se solicitan las definiciones del subconjunto `email_list_unread`, `email_get_body`, `email_mark_read` y `email_send`
**Then** la lista contiene las cuatro
**And** las tres de lectura y marcado declaran un permiso que no requiere aprobación
**And** `email_send` declara `ExplicitApproval`
