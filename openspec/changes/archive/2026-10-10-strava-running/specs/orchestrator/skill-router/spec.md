## MODIFIED Requirements

### Requirement: El catálogo de skills SHALL ser cerrado y cubrir todas las herramientas con sus prerrequisitos

El catálogo SHALL declarar, en código, un conjunto **cerrado de siete skills de dominio amplio**: `agenda`, `pendientes`, `recuerdos`, `entorno`, `web`, `widgets` y `running`. Cada skill SHALL declarar un id estable, las instrucciones y las `criteria` por defecto con las que se pregunta por ella, su umbral por defecto, el conjunto de herramientas que cubre —**incluyendo los prerrequisitos**— y la clave y el encabezado de su fragmento de prompt. Las herramientas de un mismo dominio SHALL NOT repartirse entre skills distintas: `pendientes` cubre `tasks` y `reminders`; `recuerdos` cubre `notes` y `unified_search`; `entorno` cubre `weather`, `geocode`, `reverse_geocode` y `search_places`; `running` cubre `strava_recent_activities`, `strava_activity_detail`, `strava_activity_streams` y `strava_athlete_stats`. El conjunto **core** no enrutable SHALL ser `get_current_time` y `get_current_location`. La herramienta `render_widget` SHALL pertenecer a la skill `widgets` y SHALL NOT estar en el core. Una herramienta PUEDE pertenecer a más de una skill. Toda herramienta registrada SHALL pertenecer al core o a al menos una skill.

#### Scenario: Toda herramienta registrada está cubierta
- **Given** el registry de producción con sus diecisiete herramientas
- **When** se contrasta con el catálogo de skills y el conjunto core
- **Then** cada herramienta pertenece al core o a alguna skill
- **And** el test de integridad falla si se registra una herramienta y se olvida en el catálogo

#### Scenario: Las herramientas de un mismo dominio no se reparten en skills distintas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de `pendientes`, `recuerdos`, `entorno` y `running`
- **Then** `tasks` y `reminders` están ambas en `pendientes`
- **And** `notes` y `unified_search` están ambas en `recuerdos`
- **And** `weather`, `geocode`, `reverse_geocode` y `search_places` están todas en `entorno`
- **And** las cuatro herramientas `strava_*` están todas en `running`

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

### Requirement: El catálogo SHALL poder consultarse por la API

La aplicación SHALL exponer el catálogo cerrado de skills en modo lectura —el id de cada skill, la clave y el encabezado de su fragmento, las herramientas que cubre y si está **habilitada**— junto al conjunto core no enrutable, para que la interfaz no duplique el catálogo en su propio código.

#### Scenario: La consulta devuelve el catálogo completo
- **Given** la aplicación en marcha
- **When** se consulta el catálogo de skills
- **Then** devuelve las siete skills con las herramientas de cada una
- **And** devuelve el conjunto core por separado
- **And** ninguna herramienta del catálogo deja de existir en el registry

#### Scenario: La consulta expone si cada skill está habilitada
- **Given** una skill con `ROUTER_SKILL_<ID>_ENABLED` ausente y otra con la clave a `false`
- **When** se consulta el catálogo por la API
- **Then** la primera figura habilitada
- **And** la segunda figura deshabilitada
