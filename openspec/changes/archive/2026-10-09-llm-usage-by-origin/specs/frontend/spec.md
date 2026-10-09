## ADDED Requirements

### Requirement: El panel de estadísticas SHALL mostrar los procesos de fondo por separado

La pestaña «Modelos» del panel de estadísticas SHALL incluir una tarjeta «Procesos de fondo» alimentada por `GET /api/stats/llm/background`, con una fila por origen (router, archivist, consolidator, collapse) y sus llamadas, tokens, coste, latencia media y errores. Las tarjetas de chat (resumen, modelos y diaria) SHALL NOT incluir las peticiones de los orígenes de fondo.

#### Scenario: La tarjeta muestra el uso de cada origen
- **Given** el endpoint devuelve entradas para router y collapse
- **When** se abre la pestaña «Modelos»
- **Then** la tarjeta «Procesos de fondo» muestra una fila por origen con sus cifras

#### Scenario: Sin procesos de fondo
- **Given** el endpoint devuelve todas las entradas a cero
- **When** se abre la pestaña «Modelos»
- **Then** la tarjeta muestra un estado vacío sin romper la vista

#### Scenario: Las tarjetas de chat no incluyen los orígenes de fondo
- **Given** datos de chat y de los orígenes de fondo
- **When** se renderizan las tarjetas de chat
- **Then** sus cifras no incluyen las peticiones de los orígenes de fondo
