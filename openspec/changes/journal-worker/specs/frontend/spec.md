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
