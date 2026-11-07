# tools/registry — Delta de especificación (change oximeal-integration)

## MODIFIED Requirements

### Requirement: El registry SHALL registrar todas las herramientas integradas sin nombres duplicados

La construcción del registry de producción SHALL registrar las **treinta y cuatro** herramientas integradas (incluidas `notes`, `unified_search`, `journal`, `render_widget`, las cuatro herramientas `strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta `notifications`, las dos herramientas del blog —`blog_comment_reply` y `blog_comment_delete`—, `zennotes` y `oximeal`), garantizando nombres únicos.

**Given** la aplicación Valet con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** figuran entre ellas `notes`, `unified_search`, `journal`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta `notifications`, las dos herramientas del blog, `zennotes` y `oximeal`
**And** no hay dos herramientas con el mismo nombre

#### Scenario: El registry de producción incluye todas las herramientas integradas
- **Given** la aplicación Valet construida con su registro de herramientas de producción
- **When** se consulta el catálogo de herramientas registradas
- **Then** `notes`, `unified_search`, `journal`, `render_widget`, las cuatro `strava_*`, las tres `timeline_*`, las cuatro `email_*`, las cuatro `rss_*`, la herramienta `notifications`, las dos herramientas del blog, `zennotes` y `oximeal`, figuran entre ellas
- **And** no hay dos herramientas con el mismo nombre
- **And** el número de herramientas registradas es treinta y cuatro

#### Scenario: Las herramientas del correo se ofrecen al modelo
- **Given** el registry de producción con la skill `email` habilitada
- **When** se solicitan las definiciones del subconjunto `email_list_unread`, `email_get_body`, `email_mark_read` y `email_send`
- **Then** la lista contiene las cuatro
- **And** las tres de lectura y marcado declaran un permiso que no requiere aprobación
- **And** `email_send` declara `ExplicitApproval`

#### Scenario: Las herramientas del feed se ofrecen al modelo
- **Given** el registry de producción con la skill `rss` habilitada
- **When** se solicitan las definiciones del subconjunto de las cuatro `rss_*`
- **Then** la lista contiene las cuatro
- **And** las cuatro declaran un permiso que no requiere aprobación

#### Scenario: La herramienta de publicación del blog se ofrece al modelo
- **Given** el registry de producción con la skill `blog` habilitada
- **When** se solicitan las definiciones del subconjunto `blog_comment_reply`
- **Then** la lista contiene esa definición
- **And** declara `ExplicitApproval`

#### Scenario: La herramienta de borrado del blog se ofrece al modelo
- **Given** el registry de producción con la skill `blog` habilitada
- **When** se solicitan las definiciones del subconjunto `blog_comment_delete`
- **Then** la lista contiene esa definición
- **And** declara `ExplicitApproval`

#### Scenario: La herramienta zennotes se ofrece al modelo
- **Given** el registry de producción con la skill `zennotes` habilitada
- **When** se solicitan las definiciones del subconjunto `zennotes`
- **Then** la lista contiene la definición `zennotes`
- **And** declara `NoConfirm` para `list`, `search` y `read`
- **And** declara `ExplicitApproval` para `write`, `trash` y `delete`

#### Scenario: La herramienta oximeal se ofrece al modelo
- **Given** el registry de producción con la skill `menus` habilitada
- **When** se solicitan las definiciones del subconjunto `oximeal`
- **Then** la lista contiene la definición `oximeal`
- **And** declara `NoConfirm` para `recent_recipes` y `search_recipes`
- **And** declara `Notify` para `add_shopping_items` y `create_recipe`
- **And** declara `ExplicitApproval` para `plan_week` y `clear_purchased`
