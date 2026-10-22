## MODIFIED Requirements

### Requirement: El catálogo de skills SHALL ser cerrado y cubrir todas las herramientas con sus prerrequisitos

El catálogo SHALL declarar, en código, un conjunto **cerrado de once skills de dominio amplio**: `agenda`, `pendientes`, `recuerdos`, `entorno`, `web`, `widgets`, `running`, `timeline`, `email`, `rss` y `blog`. Cada skill SHALL declarar un id estable, las instrucciones y las `criteria` por defecto con las que se pregunta por ella, su umbral por defecto, el conjunto de herramientas que cubre —**incluyendo los prerrequisitos**— y la clave y el encabezado de su fragmento de prompt. Las herramientas de un mismo dominio SHALL NOT repartirse entre skills distintas: `pendientes` cubre `tasks`, `reminders` y `notifications`; `recuerdos` cubre `notes`, `unified_search` y `journal`; `entorno` cubre `weather`, `geocode`, `reverse_geocode` y `search_places`; `running` cubre `strava_recent_activities`, `strava_activity_detail`, `strava_activity_streams` y `strava_athlete_stats`; `timeline` cubre `timeline_get_events`, `timeline_add_event` y `timeline_delete_event`; `email` cubre `email_list_unread`, `email_get_body`, `email_mark_read` y `email_send`; `rss` cubre `rss_list_unread`, `rss_get_entry`, `rss_search_entries` y `rss_mark_read`; `blog` cubre `blog_comment_reply`. Una capacidad de **escritura** SHALL NOT ensanchar la skill de otro dominio: `notifications` —que solo lee avisos— permanece en `pendientes` y la publicación en el blog vive en la skill `blog`, de modo que un turno sobre tareas o recordatorios no gane capacidad de publicar en el sitio. Las `criteria` de `recuerdos` SHALL incluir el habla de la bitácora (`bitácora`, `diario`, `cómo fue mi día`), de modo que el router seleccione la skill cuando el usuario pida leer o apuntar su diario; y las de `blog` SHALL cubrir el caso legítimo de responder a un comentario del blog, **incluido el que llega como aviso entrante**. El conjunto **core** no enrutable SHALL ser `get_current_time` y `get_current_location`. La herramienta `render_widget` SHALL pertenecer a la skill `widgets` y SHALL NOT estar en el core. Una herramienta PUEDE pertenecer a más de una skill. Toda herramienta registrada SHALL pertenecer al core o a al menos una skill.

#### Scenario: Toda herramienta registrada está cubierta
- **Given** el registry de producción con sus treinta y una herramientas
- **When** se contrasta con el catálogo de skills y el conjunto core
- **Then** cada herramienta pertenece al core o a alguna skill
- **And** el test de integridad falla si se registra una herramienta y se olvida en el catálogo

#### Scenario: Las herramientas de un mismo dominio no se reparten en skills distintas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de `pendientes`, `recuerdos`, `entorno`, `running`, `timeline`, `email`, `rss` y `blog`
- **Then** `tasks`, `reminders` y `notifications` están todas en `pendientes`
- **And** `notes`, `unified_search` y `journal` están todas en `recuerdos`
- **And** `weather`, `geocode`, `reverse_geocode` y `search_places` están todas en `entorno`
- **And** las cuatro herramientas `strava_*` están todas en `running`
- **And** las tres herramientas `timeline_*` están todas en `timeline`
- **And** las cuatro herramientas `email_*` están todas en `email`
- **And** las cuatro herramientas `rss_*` están todas en `rss`
- **And** `blog_comment_reply` está en `blog`
- **And** `blog_comment_reply` NO está en `pendientes`, donde solo se leen los avisos

#### Scenario: El conjunto core no se enruta
- **Given** el catálogo de skills
- **When** se busca `get_current_time` y `get_current_location` entre las skills enrutables
- **Then** no pertenecen a ninguna skill enrutable
- **And** se exponen siempre, con independencia de la selección
- **And** `render_widget` no pertenece al core: se enruta con la skill `widgets`

#### Scenario: El widget se enruta con umbral propio
- **Given** la skill `widgets`
- **When** se inspecciona su umbral por defecto
- **Then** es más alto que el de las skills de dominio
- **And** cubre la herramienta `render_widget`

#### Scenario: Una skill cubre los prerrequisitos de sus herramientas
- **Given** la skill `entorno`, cuyas herramientas meteorológicas necesitan coordenadas
- **When** se inspeccionan las herramientas que cubre
- **Then** incluye `geocode` además de `weather`

#### Scenario: La skill de running cubre sus cuatro herramientas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de la skill `running`
- **Then** cubre `strava_recent_activities`, `strava_activity_detail`, `strava_activity_streams` y `strava_athlete_stats`

#### Scenario: La skill del timeline cubre sus tres herramientas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de la skill `timeline`
- **Then** cubre `timeline_get_events`, `timeline_add_event` y `timeline_delete_event`
- **And** `timeline` es una skill enrutable, no parte del conjunto core

#### Scenario: La skill del correo cubre sus cuatro herramientas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de la skill `email`
- **Then** cubre `email_list_unread`, `email_get_body`, `email_mark_read` y `email_send`
- **And** `email` es una skill enrutable, no parte del conjunto core
- **And** declara el umbral `0.10` de las skills de dominio

#### Scenario: La skill del feed cubre sus cuatro herramientas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de la skill `rss`
- **Then** cubre `rss_list_unread`, `rss_get_entry`, `rss_search_entries` y `rss_mark_read`
- **And** `rss` es una skill enrutable, no parte del conjunto core
- **And** declara el umbral `0.10` de las skills de dominio

#### Scenario: La skill del blog cubre la publicación, y solo ella
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas y las `criteria` de la skill `blog`
- **Then** cubre `blog_comment_reply` y ninguna otra herramienta
- **And** `blog` es una skill enrutable, no parte del conjunto core
- **And** declara el umbral `0.10` de las skills de dominio
- **And** su `criteria_true` cubre responder a un comentario del blog, también cuando el comentario llega como aviso entrante

#### Scenario: La skill de recuerdos cubre la bitácora y su habla
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas y las `criteria` de la skill `recuerdos`
- **Then** cubre la herramienta `journal` además de `notes` y `unified_search`
- **And** `journal` no es parte del conjunto core ni se reparte en otra skill
- **And** sus `criteria` mencionan la bitácora, el diario y cómo fue el día
