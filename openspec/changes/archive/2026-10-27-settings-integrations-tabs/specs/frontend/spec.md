## REMOVED Requirements

### Requirement: SettingsDialog SHALL display API Keys tab

**Reason**: la pestaña «API Keys» mezclaba dos conceptos distintos —credenciales de integraciones
que la app **consume** y claves de ingesta que la app **emite**— y no informaba de qué integraciones
estaban configuradas. El requisito se sustituye por dos deltas: uno para la pestaña renombrada
«Claves» (solo claves de ingesta) y otro para la organización de «Integraciones» en sub-pestañas.

**Migration**: los campos de credenciales (`openweather_api_key`, `google_places_api_key`,
`brave_search_api_key`, `apimail_base_url`, `apimail_api_key`, `miniflux_base_url`,
`miniflux_api_key`) y el token de ZenNotes pasan a las sub-pestañas correspondientes de la pestaña
«Integraciones», sin cambiar las claves de ajuste ni los endpoints (`PUT /api/settings` para todos
salvo ZenNotes, que conserva `PUT /api/zennotes/token`). Los escenarios de esos campos se conservan
en el nuevo requisito «SettingsDialog SHALL organize the Integraciones tab in sub-tabs».

## ADDED Requirements

### Requirement: SettingsDialog SHALL display the Claves tab for ingestion keys only

SettingsDialog SHALL mostrar una pestaña de nivel superior llamada **«Claves»** que contiene
**exclusivamente** la gestión de **claves de ingesta** (`IngestKeysSection`): crear una clave,
listar las existentes con su prefijo y su fecha de uso, y revocarlas.

La pestaña «Claves» **NO SHALL** contener campos de credenciales de integraciones: ni las API keys
de OpenWeatherMap, Google Places o Brave Search, ni las URL base de apimail o Miniflux, ni el token
de ZenNotes. Esas credenciales SHALL vivir en la pestaña «Integraciones» (ver
«SettingsDialog SHALL organize the Integraciones tab in sub-tabs»).

El nombre «Claves» refleja que la app **emite** estas credenciales para que agentes externos
ingieran eventos por la API, a diferencia de las credenciales de integraciones, que la app
**consume**.

#### Scenario: La pestaña «Claves» solo muestra la gestión de claves de ingesta

- **Given** el SettingsDialog está abierto
- **When** el usuario hace clic en la tab «Claves»
- **Then** se muestra el listado de claves de ingesta con su prefijo y su fecha de uso
- **And** se muestra el control para crear una clave nueva
- **And** NO se muestra ningún campo de API key de integración
- **And** NO se muestra ninguna URL base de apimail ni de Miniflux
- **And** NO se muestra el campo del token de ZenNotes

#### Scenario: Crear una clave de ingesta muestra el token una única vez

- **Given** el SettingsDialog está abierto en la tab «Claves»
- **When** el usuario crea una clave de ingesta
- **Then** se muestra el token generado completo una única vez
- **And** la clave aparece en el listado identificada por su prefijo
- **And** volver a renderizar la pestaña ya no muestra el token completo

#### Scenario: La lista nunca revela el token completo

- **Given** una clave de ingesta ya creada con anterioridad
- **When** se renderiza la tab «Claves»
- **Then** el listado muestra solo el prefijo de la clave
- **And** el token completo NO aparece en ningún punto del listado

#### Scenario: Revocar una clave de ingesta pide confirmación

- **Given** el SettingsDialog está abierto en la tab «Claves» con al menos una clave
- **When** el usuario pulsa «Revocar» sobre una clave
- **Then** se pide confirmación antes de ejecutar el borrado
- **And** si el usuario cancela, la clave sigue en el listado
- **And** si el usuario confirma, la clave desaparece del listado

### Requirement: SettingsDialog SHALL organize the Integraciones tab in sub-tabs

La pestaña «Integraciones» del SettingsDialog SHALL organizarse en **sub-pestañas horizontales**
(antd `Tabs` anidado, sin `tabPosition`), en este orden exacto:

1. **Strava**
2. **OpenWeatherMap**
3. **Google Places**
4. **Brave Search**
5. **apimail**
6. **Miniflux**
7. **ZenNotes**

Cada sub-pestaña SHALL contener únicamente los campos y acciones de su integración y SHALL tener
**su propio botón «Guardar»**; no habrá un botón de guardado común a todas las sub-pestañas.

Los campos de clave SHALL moverse desde la antigua pestaña «API Keys» **sin cambiar las claves de
ajuste** ni los endpoints:

- OpenWeatherMap → `openweather_api_key`
- Google Places → `google_places_api_key`
- Brave Search → `brave_search_api_key`
- apimail → `apimail_base_url` y `apimail_api_key`
- Miniflux → `miniflux_base_url` y `miniflux_api_key`

Las sub-pestañas de OpenWeatherMap, Google Places, Brave Search, apimail y Miniflux SHALL reutilizar
el mismo `Form` de ajustes y SHALL persistir con `PUT /api/settings`. Strava SHALL conservar su flujo
actual y ZenNotes SHALL conservar su endpoint dedicado `PUT /api/zennotes/token`; `zennotes_token`
NO SHALL enviarse en `PUT /api/settings`.

Los campos de URL base SHALL conservar sus placeholders por defecto
(`https://apimail.territoriolinux.es` y `https://miniflux.territoriolinux.es`) y los campos de
clave SHALL conservar el placeholder «Dejar vacío para usar variable de entorno».

#### Scenario: La pestaña Integraciones muestra las sub-pestañas en orden

- **Given** el SettingsDialog está abierto
- **When** el usuario hace clic en la tab «Integraciones»
- **Then** se muestran siete sub-pestañas en horizontal
- **And** su orden es Strava, OpenWeatherMap, Google Places, Brave Search, apimail, Miniflux
  y ZenNotes
- **And** la sub-pestaña activa por defecto es la primera (Strava)

#### Scenario: Cada sub-pestaña tiene sus propios campos y su propio botón Guardar

- **Given** el SettingsDialog está abierto en «Integraciones»
- **When** el usuario abre la sub-pestaña «OpenWeatherMap»
- **Then** se muestra el campo de la API key de OpenWeatherMap y un botón «Guardar»
- **And** NO se muestran los campos de otras integraciones
- **And** al pulsar «Guardar» se llama a `updateSettings` con `openweather_api_key`

#### Scenario: apimail muestra sus dos campos y guarda solo con su botón

- **Given** `settings` contiene `apimail_base_url` y `apimail_api_key`
- **When** el usuario abre la sub-pestaña «apimail»
- **Then** el campo de URL base muestra el valor vigente
- **And** el campo de la API key está enmascarado
- **And** al editar la API key y pulsar «Guardar», `updateSettings` se llama con `apimail_api_key`

#### Scenario: Las URL base conservan su placeholder por defecto

- **Given** `settings` sin `apimail_base_url` ni `miniflux_base_url`
- **When** el usuario abre las sub-pestañas «apimail» y «Miniflux»
- **Then** el placeholder de la URL base de apimail es `https://apimail.territoriolinux.es`
- **And** el placeholder de la URL base de Miniflux es `https://miniflux.territoriolinux.es`

#### Scenario: Miniflux guarda con su propia sub-pestaña

- **Given** `settings` contiene `miniflux_base_url` y `miniflux_api_key`
- **When** el usuario abre la sub-pestaña «Miniflux», edita la API key y pulsa «Guardar»
- **Then** `updateSettings` se llama con `miniflux_api_key`
- **And** NO se envía `zennotes_token`

#### Scenario: ZenNotes conserva su endpoint dedicado

- **Given** el usuario escribe un token en la sub-pestaña «ZenNotes»
- **When** pulsa «Guardar»
- **Then** se llama a `PUT /api/zennotes/token` con el token
- **And** `zennotes_token` NO se envía en `PUT /api/settings`

#### Scenario: Al cerrar y reabrir el diálogo, el contenido se monta de nuevo y los indicadores se releen

- **Given** el SettingsDialog abierto, con las sub-pestañas de «Integraciones» ya visitadas
- **When** el usuario cierra el diálogo y lo vuelve a abrir
- **Then** el `Modal` desmonta su contenido al cerrar (`destroyOnHidden`) y lo monta de nuevo al
  abrir
- **And** la pestaña superior vuelve a su estado inicial («Perfil») y la sub-pestaña activa de
  «Integraciones» vuelve a ser la primera (Strava)
- **And** se relanza una lectura de `GET /api/strava/status` y de `GET /api/zennotes/status`, una
  por apertura, de modo que los indicadores reflejan el estado remoto vigente y no el de la
  apertura anterior

### Requirement: SettingsDialog SHALL show a configuration indicator per integration

Cada sub-pestaña de «Integraciones» SHALL mostrar un **indicador de configuración** con dos
ubicaciones complementarias:

- en la **etiqueta de la sub-pestaña** (Tag o punto de color acompañado de **texto accesible**, de
  modo que el estado no se transmita solo por color);
- en una **línea de estado** dentro del panel de la sub-pestaña.

El criterio de estado SHALL ser:

- **Strava** → «Conectada» / «No conectada», derivado de `GET /api/strava/status`.
- **OpenWeatherMap, Google Places, Brave Search, apimail y Miniflux** → «Configurada» si la clave
  correspondiente tiene valor no vacío en `settings`, «Sin configurar» si está vacía o ausente.
  Para apimail y Miniflux el criterio SHALL ser la API key (la URL base tiene valor por defecto y no
  determina el estado).
- **ZenNotes** → «Configurado» / «Sin configurar», derivado del campo `configured` de
  `GET /api/zennotes/status`.

El indicador de la etiqueta SHALL ser correcto **desde el primer render** del panel Integraciones,
es decir, **NO SHALL** depender de que el usuario abra la sub-pestaña correspondiente (el montaje
perezoso de las sub-pestañas no puede dejar los indicadores sin calcular). Tras guardar en una
sub-pestaña, su indicador SHALL actualizarse sin necesidad de recargar la página.

#### Scenario: El indicador de Strava refleja el estado de conexión

- **Given** `GET /api/strava/status` responde con Strava conectada
- **When** se abre la pestaña «Integraciones»
- **Then** la sub-pestaña «Strava» muestra el indicador «Conectada»
- **And** el panel de Strava muestra una línea de estado acorde

#### Scenario: Strava sin conectar muestra el estado negativo

- **Given** `GET /api/strava/status` responde con Strava no conectada
- **When** se abre la pestaña «Integraciones»
- **Then** la sub-pestaña «Strava» muestra el indicador «No conectada»

#### Scenario: Las claves configuradas se marcan como configuradas

- **Given** `settings` contiene `openweather_api_key` con valor no vacío y
  `brave_search_api_key` vacía
- **When** se abre la pestaña «Integraciones»
- **Then** la sub-pestaña «OpenWeatherMap» muestra «Configurada»
- **And** la sub-pestaña «Brave Search» muestra «Sin configurar»

#### Scenario: apimail y Miniflux se evalúan por su API key

- **Given** `settings` con `apimail_base_url` presente pero `apimail_api_key` vacía
- **When** se abre la pestaña «Integraciones»
- **Then** la sub-pestaña «apimail» muestra «Sin configurar»

#### Scenario: El indicador de ZenNotes sale del endpoint de estado

- **Given** `GET /api/zennotes/status` responde `{ configured: true, ... }`
- **When** se abre la pestaña «Integraciones»
- **Then** la sub-pestaña «ZenNotes» muestra «Configurado»

#### Scenario: Los indicadores son correctos sin abrir cada sub-pestaña

- **Given** el SettingsDialog recién abierto en «Integraciones» sin haber visitado ninguna
  sub-pestaña
- **When** se renderiza el panel
- **Then** todas las sub-pestañas muestran ya su indicador correcto
- **And** el estado NO depende de que la sub-pestaña se haya montado

#### Scenario: El indicador se actualiza tras guardar

- **Given** la sub-pestaña «Google Places» muestra «Sin configurar»
- **When** el usuario introduce una API key y pulsa «Guardar»
- **Then** el indicador pasa a «Configurada» sin recargar la página

#### Scenario: El estado se transmite con texto, no solo con color

- **Given** una sub-pestaña con indicador de configuración
- **When** un usuario con lector de pantalla navega por las sub-pestañas
- **Then** el estado se anuncia con texto (o `aria-label`) además del color del indicador

### Requirement: SettingsDialog SHALL keep the API Keys settings test coverage aligned with the new layout

Los tests de `frontend/src/test/SettingsDialog.test.tsx` SHALL alinearse con la nueva estructura:

- los tests que hoy abren la pestaña «API Keys» para comprobar campos de credenciales SHALL navegar
  por «Integraciones» y, después, por la sub-pestaña correspondiente;
- SHALL existir una comprobación explícita de que la pestaña «Claves» contiene **solo** gestión de
  claves de ingesta y ningún campo de credencial.

#### Scenario: Los tests navegan por Integraciones y su sub-pestaña

- **Given** un test que verifica el campo de la API key de OpenWeatherMap
- **When** abre el SettingsDialog
- **Then** navega a «Integraciones» y a la sub-pestaña «OpenWeatherMap»
- **And** NO depende de la existencia de una pestaña llamada «API Keys»

#### Scenario: «Claves» no contiene campos de credencial

- **Given** el SettingsDialog abierto en la pestaña «Claves»
- **When** el test consulta los campos del formulario
- **Then** NO encuentra «OpenWeatherMap API Key», «Google Places API Key», «Brave Search API Key»,
  «apimail · URL base», «apimail · API Key», «Miniflux · URL base», «Miniflux · API Key» ni el token
  de ZenNotes
- **And** sí encuentra la gestión de claves de ingesta
