## MODIFIED Requirements

### Requirement: Las operaciones destructivas SHALL requerir aprobación explícita

Toda operación de borrado de una tool multioperación SHALL declarar `ExplicitApproval`. En concreto, la tool `tasks` SHALL devolver `ExplicitApproval` para `delete_task` y `NoConfirm` para el resto de sus operaciones. La tool `email_send` SHALL declarar `ExplicitApproval`, porque enviar correo es irreversible y sale del sistema. La tool `zennotes` SHALL declarar `ExplicitApproval` para `write`, `trash` y `delete` —sobrescribir un cuerpo y borrar (a la papelera o de forma definitiva) son destructivos— y `NoConfirm` para `list`, `search` y `read`; el resto de sus escrituras aditivas (`create`, `append`, `replace`, `move`, `archive`, `unarchive`, `rename`, `restore`) SHALL declarar `Notify`. La operación `empty-trash` SHALL NO exponerse.

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

#### Scenario: Las lecturas de zennotes no requieren aprobación

**Given** el registry de producción con la tool `zennotes`
**When** se consulta su permiso con `{"operation": "read"}`
**Then** el permiso es `NoConfirm`

#### Scenario: La sobrescritura y el borrado de zennotes requieren aprobación explícita

**Given** el registry de producción con la tool `zennotes`
**When** se consulta su permiso con `{"operation": "write"}` y con `{"operation": "delete"}`
**Then** ambos permisos son `ExplicitApproval`

#### Scenario: Las escrituras aditivas de zennotes avisan

**Given** el registry de producción con la tool `zennotes`
**When** se consulta su permiso con `{"operation": "create"}` y con `{"operation": "append"}`
**Then** ambos permisos son `Notify`

### Requirement: El registry SHALL registrar todas las herramientas integradas sin nombres duplicados

La construcción del registry de producción SHALL registrar las **treinta y tres** herramientas
integradas (incluidas `notes`, `unified_search`, `journal`, `render_widget`, las cuatro herramientas
`strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta
`notifications`, las dos herramientas del blog —`blog_comment_reply` y `blog_comment_delete`— y
`zennotes`), garantizando nombres únicos.

**Given** la aplicación Valet con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** figuran entre ellas `notes`, `unified_search`, `journal`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta `notifications`, las dos herramientas del blog, `blog_comment_reply` y `blog_comment_delete`, y `zennotes`
**And** no hay dos herramientas con el mismo nombre

#### Scenario: El registry de producción incluye todas las herramientas integradas

**Given** la aplicación Valet construida con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** `notes`, `unified_search`, `journal`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta `notifications`, las dos herramientas del blog, `blog_comment_reply` y `blog_comment_delete`, y `zennotes`, figuran entre ellas
**And** no hay dos herramientas con el mismo nombre
**And** el número de herramientas registradas es treinta y tres

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

#### Scenario: La herramienta de publicación del blog se ofrece al modelo

**Given** el registry de producción con la skill `blog` habilitada
**When** se solicitan las definiciones del subconjunto `blog_comment_reply`
**Then** la lista contiene esa definición
**And** declara `ExplicitApproval`

#### Scenario: La herramienta de borrado del blog se ofrece al modelo

**Given** el registry de producción con la skill `blog` habilitada
**When** se solicitan las definiciones del subconjunto `blog_comment_delete`
**Then** la lista contiene esa definición
**And** declara `ExplicitApproval`

#### Scenario: La herramienta zennotes se ofrece al modelo

**Given** el registry de producción con la skill `zennotes` habilitada
**When** se solicitan las definiciones del subconjunto `zennotes`
**Then** la lista contiene la definición `zennotes`
**And** declara `NoConfirm` para `list`, `search` y `read`
**And** declara `ExplicitApproval` para `write`, `trash` y `delete`
