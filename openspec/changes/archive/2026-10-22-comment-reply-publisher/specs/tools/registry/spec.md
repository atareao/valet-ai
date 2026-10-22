## MODIFIED Requirements

### Requirement: El registry SHALL registrar todas las herramientas integradas sin nombres duplicados

La construcción del registry de producción SHALL registrar todas las herramientas integradas
(incluidas `notes`, `unified_search`, `journal`, `render_widget`, las cuatro herramientas `strava_*`, las tres
`timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta `notifications` y la
herramienta `blog_comment_reply`), garantizando nombres únicos.

**Given** la aplicación Valet con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** figuran entre ellas `notes`, `unified_search`, `journal`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta `notifications` y la herramienta `blog_comment_reply`
**And** no hay dos herramientas con el mismo nombre

#### Scenario: El registry de producción incluye todas las herramientas integradas

**Given** la aplicación Valet construida con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** `notes`, `unified_search`, `journal`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta `notifications` y la herramienta `blog_comment_reply` figuran entre ellas
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

#### Scenario: La herramienta de publicación del blog se ofrece al modelo

**Given** el registry de producción con la skill `blog` habilitada
**When** se solicitan las definiciones del subconjunto `blog_comment_reply`
**Then** la lista contiene esa definición
**And** declara `ExplicitApproval`

## ADDED Requirements

### Requirement: `blog_comment_reply` SHALL declarar su permiso y documentar sus campos obligatorios

`build_tool_registry` SHALL registrar una herramienta `blog_comment_reply` que SHALL exponerse en
`definitions()` y SHALL declarar `Permission::ExplicitApproval`: publicar una respuesta en el blog es
público e irreversible, y el precedente del repositorio para una escritura de este tipo es
`email_send`. Su descripción SHALL estar en español y SHALL documentar los campos obligatorios por
operación —`content`, y `comment_id` con `post_id` salvo que se indique `notification_id`— y el hecho
de que publicar no se puede deshacer. La herramienta SHALL declarar en su esquema `content`,
`comment_id`, `post_id` y `notification_id`, siendo `content` el único campo obligatorio del esquema
(los identificadores se validan en la ejecución, porque su obligatoriedad depende de cuál de las dos
vías se use).

#### Scenario: La herramienta figura en las definiciones con sus parámetros

**Given** el registro de herramientas de producción
**When** se llama a `definitions()`
**Then** incluye `blog_comment_reply`
**And** su esquema declara los parámetros `content`, `comment_id`, `post_id` y `notification_id`
**And** `content` es obligatorio

#### Scenario: El permiso es `ExplicitApproval`

**Given** la herramienta `blog_comment_reply` registrada
**When** se consulta su permiso para unos argumentos cualesquiera
**Then** el permiso es `ExplicitApproval`

#### Scenario: La descripción está en español y documenta los obligatorios

**Given** la definición de `blog_comment_reply`
**When** se inspecciona su descripción
**Then** está en español
**And** nombra `content` y los identificadores del comentario y de la entrada
