## MODIFIED Requirements

### Requirement: El panel de estadísticas SHALL mostrar los procesos de fondo por separado

La pestaña «Modelos» del panel de estadísticas SHALL incluir una tarjeta «Procesos de fondo» alimentada por `GET /api/stats/llm/background`, con **una fila por pareja (rol, modelo)** —no una por rol—. Cada fila SHALL mostrar el rol con una etiqueta legible y su `kind` crudo, el modelo, sus llamadas, tokens, coste, latencia media, errores y **la fecha de la última llamada**. Los errores SHALL resaltarse cuando sean `> 0`. Las llamadas señaladas como sospechosas (`suspicious_calls`, salida de un token registrada como éxito) SHALL mostrarse con una marca visible. Las filas SHALL respetar la ventana temporal seleccionada en el dashboard.

La etiqueta legible SHALL existir para **todos** los orígenes canónicos que la agregación puede
devolver, incluido el origen de la bitácora (`journal`, «Bitácora»): un origen sin etiqueta caería al
`kind` crudo y la tarjeta dejaría de explicarse sola.

Las tarjetas de **resumen** y **diaria** SHALL NOT incluir las peticiones de los orígenes de fondo. La tarjeta de **modelos** sí las agrega: es una reversión deliberada de la restricción anterior (`design.md`, «Decisión consciente: el panel de modelos deja de ser solo de chat»), porque un modelo configurado en producción quedaba invisible.

#### Scenario: La tarjeta muestra el uso de cada origen
**Given** el endpoint devuelve entradas para las parejas (router, modelo A) y (collapse, modelo B)
**When** se abre la pestaña «Modelos»
**Then** la tarjeta «Procesos de fondo» muestra una fila por pareja (rol, modelo) con sus cifras

#### Scenario: El timeline aparece etiquetado como origen propio
**Given** el endpoint devuelve una entrada con `kind: "timeline"`
**When** se abre la pestaña «Modelos»
**Then** la tarjeta muestra una fila cuya etiqueta legible es «Línea temporal»
**And** se ve el `kind` crudo `timeline` junto a ella

#### Scenario: La bitácora aparece etiquetada como origen propio
**Given** el endpoint devuelve una entrada con `kind: "journal"`
**When** se abre la pestaña «Modelos»
**Then** la tarjeta muestra una fila cuya etiqueta legible es «Bitácora»
**And** se ve el `kind` crudo `journal` junto a ella

#### Scenario: Sin procesos de fondo
**Given** el endpoint devuelve todas las entradas a cero
**When** se abre la pestaña «Modelos»
**Then** la tarjeta muestra un estado vacío sin romper la vista

#### Scenario: Las tarjetas de chat no incluyen los orígenes de fondo
**Given** datos de chat y de los orígenes de fondo
**When** se renderizan las tarjetas de resumen y diaria
**Then** sus cifras no incluyen las peticiones de los orígenes de fondo
**And** la tarjeta de modelos sí las agrega (reversión deliberada)

#### Scenario: Varios modelos del mismo rol son filas distintas
**Given** el endpoint devuelve dos parejas del rol `consolidator`, una por modelo
**When** se abre la pestaña «Modelos»
**Then** la tarjeta muestra dos filas para `consolidator`, una por modelo

#### Scenario: Las llamadas sospechosas se marcan
**Given** una pareja con `suspicious_calls > 0`
**When** se abre la pestaña «Modelos»
**Then** su fila muestra una marca visible de llamadas sospechosas

#### Scenario: Los errores se resaltan
**Given** una pareja con `total_errors > 0`
**When** se abre la pestaña «Modelos»
**Then** la cifra de errores de esa fila aparece resaltada

#### Scenario: La última llamada se muestra
**Given** una pareja con `last_call_at` poblado
**When** se abre la pestaña «Modelos»
**Then** la fila muestra la fecha de la última llamada

### Requirement: SettingsDialog SHALL display Prompt tab

SettingsDialog SHALL display a "Prompts" tab with one sub-tab per editable prompt: System, Archivist, Collapse, Consolidator, Timeline and Journal.

**Given** el SettingsDialog está abierto en la tab "Prompts"
**When** se renderiza
**Then** muestra una sub-pestaña por prompt editable: "System", "Archivist", "Collapse", "Consolidator", "Timeline" y "Journal"
**And** cada sub-pestaña muestra un TextArea de 10 filas con el valor actual de `system_prompt`, `archivist_prompt`, `collapse_prompt`, `consolidator_prompt`, `timeline_prompt` y `journal_prompt` respectivamente
**And** los valores se cargan desde `GET /settings`
**And** un botón "Guardar" persiste los seis valores vía `PUT /settings`

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

#### Scenario: La sub-pestaña de la bitácora está presente
**Given** el SettingsDialog está abierto en la tab "Prompts"
**When** se renderiza
**Then** existe la sub-pestaña "Journal"
**And** al hacer clic en ella se muestra su TextArea de `journal_prompt`

#### Scenario: El prompt de la bitácora se carga desde la BD
**Given** `GET /settings` devuelve `journal_prompt = "Cuenta el día como lo viviste"`
**When** se abre la sub-pestaña "Journal"
**Then** el TextArea muestra "Cuenta el día como lo viviste"

#### Scenario: El prompt de la bitácora se guarda
**Given** el usuario edita el TextArea "Journal"
**When** hace clic en "Guardar"
**Then** `updateSettings` envía `journal_prompt` con el texto editado
**And** el resto de prompts se envían sin cambios
**And** se muestra el mensaje "Ajustes guardados"

#### Scenario: Un prompt de la bitácora personalizado no se pisa con el sembrado
**Given** un `journal_prompt` personalizado y guardado desde la sub-pestaña "Journal"
**When** se reabre el diálogo de ajustes tras un rearranque
**Then** el TextArea "Journal" muestra el valor personalizado
**And** NO se muestra el valor sembrado por la migración (la siembra no pisa un valor presente)
