## MODIFIED Requirements

### Requirement: El registry SHALL registrar todas las herramientas integradas sin nombres duplicados

La construcción del registry de producción SHALL registrar las **treinta y dos** herramientas
integradas (incluidas `notes`, `unified_search`, `journal`, `render_widget`, las cuatro herramientas
`strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta
`notifications` y las dos herramientas del blog, `blog_comment_reply` y `blog_comment_delete`),
garantizando nombres únicos.

**Given** la aplicación Valet con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** figuran entre ellas `notes`, `unified_search`, `journal`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta `notifications` y las dos herramientas del blog, `blog_comment_reply` y `blog_comment_delete`
**And** no hay dos herramientas con el mismo nombre

#### Scenario: El registry de producción incluye todas las herramientas integradas

**Given** la aplicación Valet construida con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** `notes`, `unified_search`, `journal`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta `notifications` y las dos herramientas del blog, `blog_comment_reply` y `blog_comment_delete`, figuran entre ellas
**And** no hay dos herramientas con el mismo nombre
**And** el número de herramientas registradas es treinta y dos

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

## ADDED Requirements

### Requirement: `blog_comment_delete` SHALL declarar su permiso y documentar sus campos obligatorios

`build_tool_registry` SHALL registrar una herramienta `blog_comment_delete` que SHALL exponerse en
`definitions()` y SHALL declarar `Permission::ExplicitApproval`: mandar un comentario a la papelera es
una operación destructiva sobre algo que ve el público, y el precedente del repositorio para una
escritura de este tipo es `email_send`; que sea reversible **no** rebaja el permiso. Su descripción SHALL
estar en español y SHALL documentar de dónde salen los identificadores —**exclusivamente** del
`metadata` de la notificación que anunció el comentario, indicada en `notification_id`— y el hecho de
que el comentario va a la **papelera** y no se borra definitivamente. La herramienta SHALL declarar en
su esquema **solo** `notification_id`, que SHALL ser **obligatorio**: al no existir la vía explícita en
el borrado, los identificadores no pueden llegar por otro camino, y un `comment_id` o un `post_id`
aportado por quien llama SHALL rechazarse en la ejecución con `InvalidArguments`, sin salir a la red.

#### Scenario: La herramienta figura en las definiciones con sus parámetros

**Given** el registro de herramientas de producción
**When** se llama a `definitions()`
**Then** incluye `blog_comment_delete`
**And** su esquema declara el parámetro `notification_id` y **no** declara `comment_id` ni `post_id`
**And** `notification_id` es obligatorio en el esquema

#### Scenario: El permiso es `ExplicitApproval`

**Given** la herramienta `blog_comment_delete` registrada
**When** se consulta su permiso para unos argumentos cualesquiera
**Then** el permiso es `ExplicitApproval`

#### Scenario: La descripción está en español y documenta la papelera y la procedencia de los identificadores

**Given** la definición de `blog_comment_delete`
**When** se inspecciona su descripción
**Then** está en español
**And** dice que el comentario va a la papelera y no se borra definitivamente
**And** dice que los identificadores salen del `metadata` del aviso indicado en `notification_id`
