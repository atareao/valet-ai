## MODIFIED Requirements

### Requirement: StatsDashboard SHALL display cost and tokens by model

StatsDashboard SHALL mostrar el coste y los tokens por modelo mediante un gráfico de barras y una tabla, sobre **todos los orígenes** (chat y procesos de fondo), de modo que un modelo que solo trabaja en roles de fondo también aparezca. El panel SHALL dejar de rotularse como exclusivo de chat y su título/etiqueta SHALL reflejar que agrega todos los orígenes.

**Given** el endpoint `/api/stats/llm/by-model?days=<ventana>` devuelve datos
**When** se renderiza StatsDashboard
**Then** muestra un gráfico de barras (Chart.js) con coste por modelo
**And** una tabla con modelo, calls, tokens, coste
**And** un modelo que solo se usa en roles de fondo aparece con su coste agregado

#### Scenario: Gráfico con 2 modelos
**Given** by-model devuelve `[{model: "gpt-4o", calls: 100, total_tokens: 40000, total_cost: 1.0}, {model: "claude-3", calls: 50, total_tokens: 10000, total_cost: 0.25}]`
**When** se renderiza
**Then** el gráfico tiene 2 barras
**And** la tabla tiene 2 filas ordenadas por coste descendente

#### Scenario: Un modelo de un rol de fondo aparece en el panel
**Given** by-model devuelve `[{model: "qwen/qwen3-235b-a22b-2507", calls: 33, total_tokens: 242522, total_cost: 0.12}]`
**When** se renderiza
**Then** el gráfico incluye una barra para ese modelo
**And** la tabla incluye su fila con el coste agregado

### Requirement: El panel de estadísticas SHALL mostrar los procesos de fondo por separado

La pestaña «Modelos» del panel de estadísticas SHALL incluir una tarjeta «Procesos de fondo» alimentada por `GET /api/stats/llm/background`, con **una fila por pareja (rol, modelo)** —no una por rol—. Cada fila SHALL mostrar el rol con una etiqueta legible y su `kind` crudo, el modelo, sus llamadas, tokens, coste, latencia media, errores y **la fecha de la última llamada**. Los errores SHALL resaltarse cuando sean `> 0`. Las llamadas señaladas como sospechosas (`suspicious_calls`, salida de un token registrada como éxito) SHALL mostrarse con una marca visible. Las filas SHALL respetar la ventana temporal seleccionada en el dashboard.

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

## ADDED Requirements

### Requirement: El selector de ventana del panel de estadísticas SHALL gobernar los tres paneles

El panel de estadísticas SHALL exponer un selector de ventana temporal con **7 días** como valor por
defecto. El valor seleccionado SHALL propagarse a las tres consultas del panel —resumen diario
(`by-day`), procesos de fondo (`background`) y modelos (`by-model`)— de modo que las tres tarjetas
compartan la misma ventana. Al abrir el panel sin selección previa, SHALL usarse 7 días.

#### Scenario: El valor por defecto es 7 días
**Given** un usuario que abre el panel de estadísticas sin haber elegido ventana
**When** se cargan las tarjetas
**Then** las tres consultas se hacen con `days=7`

#### Scenario: Cambiar la ventana gobierna los tres paneles
**Given** el panel de estadísticas abierto con la ventana en 7 días
**When** el usuario selecciona 30 días
**Then** las consultas de diaria, procesos de fondo y modelos se repiten con `days=30`

#### Scenario: La ventana llega al endpoint de fondo
**Given** la ventana seleccionada en 7 días
**When** se carga la tarjeta «Procesos de fondo»
**Then** la petición a `/api/stats/llm/background` incluye `days=7`

### Requirement: La fecha de última llamada SHALL interpretarse como UTC y mostrar la ausencia sin ambigüedad

El campo `last_call_at` que alimenta la tarjeta «Procesos de fondo» SHALL interpretarse como **UTC** cuando llegue en el formato crudo de SQLite (`"YYYY-MM-DD HH:MM:SS"`: separador de espacio, sin `T` y sin zona horaria; es el valor que emite `sqlite3` y que el backend reenvía tal cual). Un valor **no parseable** SHALL mostrarse como ausencia (`—`) y nunca como «Invalid Date». La marca temporal SHALL ser independiente de la zona horaria de la máquina del usuario, de modo que el hecho mostrado no dependa del navegador (WebKit rechaza el crudo de SQLite; V8 lo interpreta como hora local).

#### Scenario: El crudo de SQLite se interpreta como UTC
**Given** una pareja con `last_call_at` en el formato crudo de SQLite `"2026-10-12 19:08:45"`
**When** se abre la pestaña «Modelos»
**Then** la fila muestra ese instante como UTC (`2026-10-12T19:08:45Z`)
**And** la marca no depende de la zona horaria de la máquina

#### Scenario: Un valor no parseable se muestra como ausencia
**Given** una pareja con un `last_call_at` no parseable
**When** se abre la pestaña «Modelos»
**Then** la celda muestra `—`
**And** nunca muestra «Invalid Date»

#### Scenario: La ausencia se distingue de una fecha válida
**Given** una pareja con `last_call_at` nulo y otra con una fecha válida
**When** se abre la pestaña «Modelos»
**Then** la primera muestra `—`
**And** la segunda nunca se degrada a `—` por culpa de la zona horaria del navegador
