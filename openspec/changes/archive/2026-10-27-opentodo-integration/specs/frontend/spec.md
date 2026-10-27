## MODIFIED Requirements

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
8. **OpenTodo**

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

La sub-pestaña «OpenTodo» SHALL mostrar un campo de URL base (`opentodo_base_url`, con placeholder
`https://todo.territoriolinux.es`) que se persiste con `PUT /api/settings`, y un campo de token
(`opentodo_token`) que SHALL arrancar **siempre vacío** y NO SHALL releer el valor guardado, de modo
que el token solo SHALL persistirse con `PUT /api/opentodo/token` y NO SHALL enviarse nunca en
`PUT /api/settings`.

La sub-pestaña «OpenTodo» SHALL incluir un botón **«Probar conexión»** que SHALL consultar el
endpoint de sonda `GET /api/opentodo/check` (no `GET /api/opentodo/status`) y SHALL mostrar el
resultado —`detail`— como **texto** en la línea de estado de la región, que ya es una región viva
(`aria-live`); el resultado NO SHALL comunicarse solo mediante un toast. Cuando el estado no se haya
podido leer, la línea de estado del panel y el indicador de la etiqueta SHALL mostrar **la misma**
etiqueta («Sin datos»), y no «Sin configurar»: «no lo sé» y «no está configurado» no son lo mismo.

Los campos de URL base SHALL conservar sus placeholders por defecto
(`https://apimail.territoriolinux.es` y `https://miniflux.territoriolinux.es`) y los campos de
clave SHALL conservar el placeholder «Dejar vacío para usar variable de entorno».

#### Scenario: La pestaña Integraciones muestra las sub-pestañas en orden

- **Given** el SettingsDialog está abierto
- **When** el usuario hace clic en la tab «Integraciones»
- **Then** se muestran ocho sub-pestañas en horizontal
- **And** su orden es Strava, OpenWeatherMap, Google Places, Brave Search, apimail, Miniflux,
  ZenNotes y OpenTodo
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

#### Scenario: OpenTodo guarda el token por su endpoint dedicado

- **Given** el usuario escribe un token en la sub-pestaña «OpenTodo»
- **When** pulsa «Guardar»
- **Then** se llama a `PUT /api/opentodo/token` con el token
- **And** `opentodo_token` NO se envía en `PUT /api/settings`

#### Scenario: El campo del token de OpenTodo arranca vacío

- **Given** `opentodo_token` ya guardado en el servidor
- **When** el usuario abre la sub-pestaña «OpenTodo»
- **Then** el campo del token está vacío
- **And** el estado «Configurado» se muestra en la línea de estado, no en el campo

#### Scenario: La URL base de OpenTodo se guarda con los ajustes

- **Given** el usuario edita la URL base en la sub-pestaña «OpenTodo»
- **When** pulsa «Guardar»
- **Then** `updateSettings` se llama con `opentodo_base_url`

#### Scenario: Probar conexión sondea de verdad

- **Given** el usuario está en la sub-pestaña «OpenTodo» con un token guardado pero caducado
- **When** pulsa «Probar conexión»
- **Then** se llama a `GET /api/opentodo/check`
- **And** NO se llama a `GET /api/opentodo/status`
- **And** la línea de estado muestra el `detail` devuelto como texto

#### Scenario: El panel y la etiqueta coinciden cuando el estado no se puede leer

- **Given** `GET /api/opentodo/status` no responde
- **When** se abre la pestaña «Integraciones»
- **Then** el indicador de la sub-pestaña «OpenTodo» muestra «Sin datos»
- **And** la línea de estado del panel muestra también «Sin datos»
- **And** NO muestra «Sin configurar»

#### Scenario: Al cerrar y reabrir el diálogo, el contenido se monta de nuevo y los indicadores se releen

- **Given** el SettingsDialog abierto, con las sub-pestañas de «Integraciones» ya visitadas
- **When** el usuario cierra el diálogo y lo vuelve a abrir
- **Then** el `Modal` desmonta su contenido al cerrar (`destroyOnHidden`) y lo monta de nuevo al
  abrir
- **And** la pestaña superior vuelve a su estado inicial («Perfil») y la sub-pestaña activa de
  «Integraciones» vuelve a ser la primera (Strava)
- **And** se relanza una lectura de `GET /api/strava/status`, de `GET /api/zennotes/status` y de
  `GET /api/opentodo/status`, una por apertura, de modo que los indicadores reflejan el estado
  remoto vigente y no el de la apertura anterior

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
- **OpenTodo** → «Configurado» / «Sin configurar», derivado del campo `configured` de
  `GET /api/opentodo/status`.

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

#### Scenario: El indicador de OpenTodo sale de su endpoint de estado

- **Given** `GET /api/opentodo/status` responde `{ configured: true, base_url: "…" }`
- **When** se abre la pestaña «Integraciones»
- **Then** la sub-pestaña «OpenTodo» muestra «Configurado»

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
