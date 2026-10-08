## MODIFIED Requirements

### Requirement: El catálogo de skills SHALL ser cerrado y cubrir todas las herramientas con sus prerrequisitos

El catálogo SHALL declarar, en código, un conjunto **cerrado de seis skills de dominio amplio**: `agenda`, `pendientes`, `recuerdos`, `entorno`, `web` y `widgets`. Cada skill SHALL declarar un id estable, las instrucciones y las `criteria` por defecto con las que se pregunta por ella, su umbral por defecto, el conjunto de herramientas que cubre —**incluyendo los prerrequisitos**— y la clave y el encabezado de su fragmento de prompt. Las herramientas de un mismo dominio SHALL NOT repartirse entre skills distintas: `pendientes` cubre `tasks` y `reminders`; `recuerdos` cubre `notes` y `unified_search`; `entorno` cubre `weather`, `geocode`, `reverse_geocode` y `search_places`. El conjunto **core** no enrutable SHALL ser `get_current_time` y `get_current_location`. La herramienta `render_widget` SHALL pertenecer a la skill `widgets` y SHALL NOT estar en el core. Una herramienta PUEDE pertenecer a más de una skill. Toda herramienta registrada SHALL pertenecer al core o a al menos una skill.

#### Scenario: Toda herramienta registrada está cubierta
- **Given** el registry de producción con sus trece herramientas
- **When** se contrasta con el catálogo de skills y el conjunto core
- **Then** cada herramienta pertenece al core o a alguna skill
- **And** el test de integridad falla si se registra una herramienta y se olvida en el catálogo

#### Scenario: Las herramientas de un mismo dominio no se reparten en skills distintas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de `pendientes`, `recuerdos` y `entorno`
- **Then** `tasks` y `reminders` están ambas en `pendientes`
- **And** `notes` y `unified_search` están ambas en `recuerdos`
- **And** `weather`, `geocode`, `reverse_geocode` y `search_places` están todas en `entorno`

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

### Requirement: La configuración del enrutador SHALL vivir en settings, leerse en cada turno y arrancar apagada

La configuración SHALL vivir en la tabla `settings` —`ROUTER_ENABLED` (por defecto `false`), `ROUTER_MODEL`, `ROUTER_THRESHOLD` (por defecto `0.10`), los overrides por skill `ROUTER_THRESHOLD_<ID>`, `ROUTER_TIMEOUT_MS`, `ROUTER_HISTORY_TURNS` (por defecto `6`) y las claves de criterios y fragmentos por skill—, sembrada por una migración **idempotente que SHALL NOT sobrescribir valores existentes**, y SHALL leerse en cada turno de modo que editarla surta efecto sin reiniciar. El umbral efectivo de una skill SHALL ser su override si existe y, si no, su umbral por defecto; un valor ausente o ilegible SHALL caer al default registrando un warning. El endpoint y la credencial SHALL proceder del entorno. Los valores por defecto documentados proceden de la medición del arnés sobre el historial real, no de una elección de gusto.

#### Scenario: Por defecto el enrutado está apagado
- **Given** una base de datos recién migrada
- **When** se procesa un turno
- **Then** `ROUTER_ENABLED` es `false`
- **And** la petición lleva todas las herramientas habilitadas

#### Scenario: El umbral por skill se aplica por encima del global
- **Given** `ROUTER_THRESHOLD` en `0.10` y `ROUTER_THRESHOLD_WIDGETS` en `0.20`
- **When** se resuelve el umbral efectivo de cada skill
- **Then** las skills de dominio usan `0.10`
- **And** `widgets` usa `0.20`

#### Scenario: Un umbral ilegible cae al default
- **Given** `ROUTER_THRESHOLD` o un override `ROUTER_THRESHOLD_<ID>` con un valor no numérico o fuera de `[0, 1]`
- **When** se resuelve el umbral efectivo
- **Then** se usa el umbral por defecto correspondiente
- **And** se registra un warning

#### Scenario: Encender surte efecto sin reiniciar
- **Given** el enrutado apagado
- **When** se pone `ROUTER_ENABLED` a `true` desde los ajustes
- **Then** el turno siguiente ya enruta

### Requirement: El arnés de evaluación SHALL medir la cobertura de las herramientas realmente usadas

El arnés SHALL recorrer los turnos históricos cuyo mensaje de asistente registra herramientas usadas, ejecutar el enrutador sobre el mensaje del usuario y calcular la **cobertura**: la proporción de turnos en los que **toda** herramienta realmente usada figura en el conjunto que el enrutador habría expuesto. SHALL usar los **valores efectivos** de `settings` —criterios, umbrales y número de turnos de historial—, de modo que el bucle de ajuste sea «editar y medir». SHALL **publicar la configuración efectiva** con la que ha medido (las skills, sus umbrales y qué campos están sobrescritos), para que cada cifra sea atribuible. SHALL reportar además las activaciones por skill, la latencia p50 y p95, los tokens y el coste, y SHALL aceptar `--limit`, `--threshold`, `--model` y `--dry-run` (catálogo sin red).

#### Scenario: El arnés mide con los criterios vigentes
- **Given** un criterio sobrescrito en `settings`
- **When** se ejecuta el arnés
- **Then** las peticiones al clasificador usan el criterio sobrescrito
- **And** el informe declara qué campos están sobrescritos

#### Scenario: Un turno no cubierto se reporta
- **Given** un turno histórico que usó `tasks` y el enrutador no expuso `tasks`
- **When** se calcula la cobertura
- **Then** ese turno cuenta como no cubierto
- **And** el informe señala la skill `pendientes` como la que lo habría cubierto

#### Scenario: El modo sin red no llama al servicio
- **Given** el arnés en modo `--dry-run`
- **When** se ejecuta
- **Then** recorre el catálogo y el historial sin ninguna petición de red
- **And** declara la configuración efectiva que habría usado

## ADDED Requirements

### Requirement: Las preguntas del clasificador SHALL ser editables desde settings con el valor compilado como respaldo

Las instrucciones y las `criteria` con las que se pregunta al clasificador SHALL poder sobrescribirse desde `settings`, con una clave por campo y skill (`SKILL_<ID>_QUESTION`, `SKILL_<ID>_CRITERIA_TRUE`, `SKILL_<ID>_CRITERIA_FALSE`). El valor **declarado en el catálogo** SHALL ser el default y el respaldo: una clave ausente, vacía o solo con espacios en blanco SHALL hacer que se use el valor del código, de modo que una edición a medias no pueda enviar una pregunta sin criterios ni degradar el enrutado en silencio. Los valores SHALL leerse en cada turno. La consulta del catálogo por la API SHALL exponer el valor **efectivo** de cada campo y si está sobrescrito.

#### Scenario: Sin override se usa el valor del catálogo
- **Given** una base de datos sin la clave de la pregunta de una skill
- **When** se construye su pregunta para el clasificador
- **Then** se usan las instrucciones y las criteria declaradas en el catálogo

#### Scenario: Un override surte efecto en el mismo turno
- **Given** una pregunta sobrescrita en `settings`
- **When** se procesa un turno
- **Then** la pregunta enviada al clasificador es la sobrescrita
- **And** no hace falta reiniciar

#### Scenario: Un campo en blanco nunca envía una pregunta sin criterios
- **Given** una clave de criterio presente pero con solo espacios en blanco
- **When** se construye la pregunta
- **Then** se usa el valor declarado en el catálogo
- **And** no se envía ningún criterio vacío

#### Scenario: La API expone el valor efectivo y la marca de sobrescrito
- **Given** una skill con su pregunta sobrescrita y sus criterios sin tocar
- **When** se consulta el catálogo por la API
- **Then** la pregunta devuelta es la sobrescrita y figura marcada como sobrescrita
- **And** los criterios devueltos son los del catálogo, sin la marca

### Requirement: La guía de widgets SHALL residir en el fragmento de su skill

La guía de uso de `render_widget` SHALL NOT residir de forma permanente en `settings.system_prompt`, porque la herramienta deja de estar siempre expuesta y el prompt base no puede ordenar el uso de una herramienta ausente. Una migración SHALL retirar ese bloque del prompt base **solo si se encuentra verbatim** (identificado por su encabezado) y SHALL depositarlo como contenido de `SKILL_WIDGETS_PROMPT`; si el bloque no se encuentra verbatim, SHALL NOT retirar nada y el fragmento SHALL sembrarse igualmente. La regla anti-duplicado del ensamblador de fragmentos SHALL seguir vigente como red de seguridad. Con la skill `widgets` inactiva SHALL NOT enviarse ni el esquema de la herramienta ni la guía.

#### Scenario: El bloque verbatim se muda al fragmento
- **Given** un prompt base que contiene el bloque de widgets tal y como lo añadió la migración original
- **When** se aplica la migración
- **Then** el bloque deja de estar en el prompt base
- **And** su contenido queda en `SKILL_WIDGETS_PROMPT`

#### Scenario: Un bloque editado no se retira
- **Given** un prompt base en el que el usuario ha modificado el bloque de widgets
- **When** se aplica la migración
- **Then** el prompt base conserva el texto del usuario
- **And** el fragmento se siembra igualmente
- **And** la regla anti-duplicado impide que la guía viaje dos veces

#### Scenario: Con la skill inactiva no viaja la guía
- **Given** un turno en el que la skill `widgets` no se selecciona
- **When** se compone el mensaje de sistema
- **Then** no contiene la guía de widgets
- **And** la herramienta `render_widget` no figura entre las ofrecidas

#### Scenario: La migración es idempotente
- **Given** una base de datos ya migrada
- **When** se vuelve a aplicar la migración
- **Then** el prompt base y el fragmento quedan igual
