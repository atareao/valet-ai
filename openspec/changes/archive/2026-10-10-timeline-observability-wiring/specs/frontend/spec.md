## MODIFIED Requirements

### Requirement: StatsDashboard SHALL display database table sizes

StatsDashboard SHALL mostrar una tabla con el número de filas de cada tabla ordenada de mayor a menor.

**Given** el endpoint `/api/stats/db/sizes` devuelve datos
**When** se renderiza StatsDashboard
**Then** muestra una tabla con nombre de tabla y row count
**And** las tablas se ordenan por row count descendente

#### Scenario: DB sizes con datos
**Given** db/sizes devuelve `[{table: "messages", rows: 1500}, {table: "events", rows: 200}, ...]`
**When** se renderiza
**Then** messages aparece primero (mayor row count)

#### Scenario: La tabla del timeline aparece en el listado
**Given** db/sizes devuelve una entrada `{table: "timeline_events", rows: 12}`
**When** se renderiza
**Then** existe una fila con la tabla `timeline_events` y su recuento

### Requirement: El panel de estadísticas SHALL mostrar los procesos de fondo por separado

La pestaña «Modelos» del panel de estadísticas SHALL incluir una tarjeta «Procesos de fondo» alimentada por `GET /api/stats/llm/background`, con una fila por origen (router, archivist, consolidator, collapse, timeline) y sus llamadas, tokens, coste, latencia media y errores. Cada origen SHALL mostrarse con una etiqueta legible y su `kind` crudo. Las tarjetas de chat (resumen, modelos y diaria) SHALL NOT incluir las peticiones de los orígenes de fondo.

#### Scenario: La tarjeta muestra el uso de cada origen
- **Given** el endpoint devuelve entradas para router y collapse
- **When** se abre la pestaña «Modelos»
- **Then** la tarjeta «Procesos de fondo» muestra una fila por origen con sus cifras

#### Scenario: El timeline aparece etiquetado como origen propio
- **Given** el endpoint devuelve una entrada con `kind: "timeline"`
- **When** se abre la pestaña «Modelos»
- **Then** la tarjeta muestra una fila cuya etiqueta legible es «Línea temporal»
- **And** se ve el `kind` crudo `timeline` junto a ella

#### Scenario: Sin procesos de fondo
- **Given** el endpoint devuelve todas las entradas a cero
- **When** se abre la pestaña «Modelos»
- **Then** la tarjeta muestra un estado vacío sin romper la vista

#### Scenario: Las tarjetas de chat no incluyen los orígenes de fondo
- **Given** datos de chat y de los orígenes de fondo
- **When** se renderizan las tarjetas de chat
- **Then** sus cifras no incluyen las peticiones de los orígenes de fondo

### Requirement: SettingsDialog SHALL display Prompt tab

SettingsDialog SHALL display a "Prompts" tab with one sub-tab per editable prompt: System, Archivist, Collapse, Consolidator and Timeline.

**Given** el SettingsDialog está abierto en la tab "Prompts"
**When** se renderiza
**Then** muestra una sub-pestaña por prompt editable: "System", "Archivist", "Collapse", "Consolidator" y "Timeline"
**And** cada sub-pestaña muestra un TextArea de 10 filas con el valor actual de `system_prompt`, `archivist_prompt`, `collapse_prompt`, `consolidator_prompt` y `timeline_prompt` respectivamente
**And** los valores se cargan desde `GET /settings`
**And** un botón "Guardar" persiste los cinco valores vía `PUT /settings`

#### Scenario: Las tres sub-pestañas están presentes
**Given** el SettingsDialog está abierto en la tab "Prompts"
**When** se renderiza
**Then** existen las sub-pestañas "System", "Archivist" y "Collapse"
**And** al hacer clic en cada una se muestra su TextArea correspondiente

#### Scenario: La sub-pestaña del timeline está presente
**Given** el SettingsDialog está abierto en la tab "Prompts"
**When** se renderiza
**Then** existe la sub-pestaña "Timeline"
**And** al hacer clic en ella se muestra su TextArea de `timeline_prompt`

#### Scenario: System prompt se carga desde la BD
**Given** `GET /settings` devuelve `system_prompt = "Eres Valet"`
**When** se abre la sub-pestaña "System"
**Then** el TextArea muestra "Eres Valet"

#### Scenario: Archivist prompt se carga desde la BD
**Given** `GET /settings` devuelve `archivist_prompt = "Eres un archivista"`
**When** se abre la sub-pestaña "Archivist"
**Then** el TextArea muestra "Eres un archivista"

#### Scenario: Collapse prompt se carga desde la BD
**Given** `GET /settings` devuelve `collapse_prompt = "Resume el texto"`
**When** se abre la sub-pestaña "Collapse"
**Then** el TextArea muestra "Resume el texto"

#### Scenario: El prompt del timeline se carga desde la BD
**Given** `GET /settings` devuelve `timeline_prompt = "Extrae hechos fechados"`
**When** se abre la sub-pestaña "Timeline"
**Then** el TextArea muestra "Extrae hechos fechados"

#### Scenario: Los tres prompts se guardan
**Given** el usuario edita los tres TextAreas
**When** hace clic en "Guardar"
**Then** `updateSettings` se llama con `{ system_prompt, archivist_prompt, collapse_prompt }` entre los valores enviados
**And** se muestra mensaje "Ajustes guardados"

#### Scenario: El prompt del timeline se guarda
**Given** el usuario edita el TextArea "Timeline"
**When** hace clic en "Guardar"
**Then** `updateSettings` envía `timeline_prompt` con el texto editado
**And** se muestra el mensaje "Ajustes guardados"

#### Scenario: Prompt se guarda
**Given** el usuario escribe "Eres un asistente útil" en el TextArea "System"
**When** hace clic en "Guardar"
**Then** `updateSettings` se llama con `{ system_prompt: "Eres un asistente útil", ... }`

#### Scenario: Editar un prompt no borra los otros
**Given** el usuario modifica solo el TextArea "Archivist"
**When** hace clic en "Guardar"
**Then** los demás prompts se envían con sus valores actuales sin cambios

### Requirement: SettingsDialog SHALL display a Generación tab with the generation knobs

SettingsDialog SHALL display a "Generación" tab with five blocks —Chat, Colapso, Fichas,
Consolidación y Línea temporal—, y dentro de cada bloque tres campos: temperatura (numérico),
razonamiento (selector) y tokens máximos (numérico). El selector de razonamiento SHALL ofrecer al
menos `default` (no enviar), `off`, `minimal`, `low`, `medium`, `high`, `xhigh` y `max`. Los valores
SHALL cargarse de `GET /settings` y guardarse con `PUT /settings`, sin rutas nuevas de API. Al
ajustarse en caliente, un cambio guardado SHALL surtir efecto sin reiniciar.

**Given** el SettingsDialog está abierto en la tab "Generación"  
**When** se renderiza  
**Then** muestra cinco bloques, uno por rol  
**And** cada bloque muestra temperatura, razonamiento y tokens máximos  
**And** cada campo muestra el valor actual cargado de `GET /settings`  
**And** un botón "Guardar" persiste las quince claves vía `PUT /settings`

#### Scenario: La pestaña muestra los cuatro roles y sus tres campos
**Given** el SettingsDialog abierto en la tab "Generación"  
**When** se renderiza  
**Then** existen los bloques Chat, Colapso, Fichas y Consolidación  
**And** cada bloque tiene temperatura, razonamiento y tokens máximos

#### Scenario: El rol del timeline aparece con sus tres campos
**Given** el SettingsDialog abierto en la tab "Generación"  
**When** se renderiza  
**Then** existe el bloque "Línea temporal"  
**And** tiene temperatura, razonamiento y tokens máximos

#### Scenario: Los valores se cargan desde la BD
**Given** `GET /settings` devuelve `GENERATION_CHAT_TEMPERATURE = 0.7` y `GENERATION_SEMANTIC_REASONING = low`  
**When** se abre la tab "Generación"  
**Then** el campo de temperatura del chat muestra `0.7`  
**And** el selector de razonamiento de consolidación muestra `low`

#### Scenario: Los parámetros del timeline se cargan desde la BD
**Given** `GET /settings` devuelve `GENERATION_TIMELINE_TEMPERATURE = 0.2` y `GENERATION_TIMELINE_MAX_TOKENS = 2048`  
**When** se abre el bloque del timeline  
**Then** su temperatura muestra `0.2`  
**And** sus tokens máximos muestran `2048`

#### Scenario: El selector de razonamiento ofrece las opciones esperadas
**Given** el SettingsDialog abierto en la tab "Generación"  
**When** se abre el selector de razonamiento de cualquier rol  
**Then** ofrece `default`, `off`, `minimal`, `low`, `medium`, `high`, `xhigh` y `max`

#### Scenario: Guardar envía las doce claves y muestra confirmación
**Given** el usuario edita la temperatura del chat  
**When** hace clic en "Guardar"  
**Then** `updateSettings` se llama con las claves de los roles, las editadas y las demás con su valor actual  
**And** se muestra el mensaje "Ajustes guardados"

#### Scenario: Editar un campo no borra los demás
**Given** el usuario modifica solo los tokens máximos del consolidador  
**When** hace clic en "Guardar"  
**Then** las demás claves se envían con sus valores actuales sin cambios

### Requirement: SettingsDialog SHALL organize the Generación roles in sub-tabs

Dentro de la pestaña «Generación», SettingsDialog SHALL mostrar un `Tabs` anidado con una sub-pestaña
por rol —**Chat**, **Colapso**, **Fichas**, **Consolidación** y **Línea temporal**— siguiendo el
patrón de la pestaña «Prompts». La sub-pestaña «Consolidación» corresponde a las claves
`GENERATION_SEMANTIC_*` y la sub-pestaña «Línea temporal» a las claves `GENERATION_TIMELINE_*`. Cada
sub-pestaña SHALL contener los tres campos del rol (temperatura numérica, razonamiento por selector
y tokens máximos numéricos), manteniendo `name`/`label` iguales a la clave cruda. Las sub-pestañas
SHALL usar `forceRender` para que los quince campos permanezcan registrados en el formulario aunque
su sub-pestaña no esté activa, de modo que el guardado siga enviando las quince claves.

**Given** el SettingsDialog abierto en la pestaña "Generación"  
**When** se renderiza  
**Then** existe un `Tabs` anidado con las sub-pestañas Chat, Colapso, Fichas, Consolidación y Línea temporal  
**And** la sub-pestaña activa muestra temperatura, razonamiento y tokens máximos de su rol

#### Scenario: Existen las cuatro sub-pestañas de rol
**Given** el SettingsDialog abierto en la pestaña "Generación"  
**When** se renderiza  
**Then** existen sub-pestañas con nombre Chat, Colapso, Fichas y Consolidación

#### Scenario: La sub-pestaña del timeline muestra sus tres claves
**Given** el SettingsDialog abierto en la pestaña "Generación"  
**When** el usuario selecciona la sub-pestaña "Línea temporal"  
**Then** quedan visibles `GENERATION_TIMELINE_TEMPERATURE`, `GENERATION_TIMELINE_REASONING` y `GENERATION_TIMELINE_MAX_TOKENS`

#### Scenario: Cambiar de sub-pestaña muestra los campos del rol
**Given** el SettingsDialog abierto en "Generación" con la sub-pestaña "Chat" activa  
**When** el usuario selecciona la sub-pestaña "Colapso"  
**Then** los campos de colapso (`GENERATION_COLLAPSE_*`) quedan visibles  
**And** la sub-pestaña "Chat" deja de estar visible

#### Scenario: Los doce campos siguen registrados con forceRender
**Given** el usuario abrió la pestaña "Generación"  
**When** guarda sin haber abierto todas las sub-pestañas  
**Then** `updateSettings` recibe las claves `GENERATION_*` sin cambios en las no editadas
