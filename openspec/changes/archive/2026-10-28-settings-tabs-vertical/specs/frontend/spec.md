## ADDED Requirements

### Requirement: El diálogo de ajustes SHALL mostrar todas sus pestañas en vertical

Todos los `Tabs` del diálogo de ajustes SHALL renderizarse con `tabPosition="left"`, de modo que la
lista de pestañas quede como un índice lateral a la izquierda del panel. El alcance SHALL cubrir la
pestaña superior y **cada uno** de los grupos anidados:

- **nivel superior**: Perfil, Interfaz, Prompts, Claves, Memoria, Generación, Skills, Enrutador de
  skills, Integraciones y Notificaciones;
- **«Prompts»**: System, Archivist, Collapse, Consolidator, Timeline y Journal;
- **«Memoria»**: Episódica y Persistente;
- **«Generación»**: Chat, Colapso, Fichas, Consolidación y Línea temporal;
- **«Skills»**: una sub-pestaña por skill del catálogo;
- **«Integraciones»**: Strava, OpenWeatherMap, Google Places, Brave Search, apimail, Miniflux,
  ZenNotes y OpenTodo.

El cambio SHALL ser **solo de orientación**: NO SHALL alterar las etiquetas, el orden de las
pestañas, los `forceRender`, los `aria-label` de contenedor, el cableado de los formularios
(`settingsForm`, `profileForm` y los formularios propios de `SkillsTab` e `IntegrationsTab`) ni los
endpoints que invoca cada sub-pestaña. Cada `Tabs` conserva su propio estado de pestaña activa y
antd sigue montando solo el panel de la pestaña seleccionada, salvo donde ya se exige `forceRender`.

El `Modal` SHALL conservar un ancho declarado de 1000 px: con la lista lateral, el ancho útil del
panel es el del diálogo menos el de la lista.

**Given** el SettingsDialog abierto  
**When** se renderiza  
**Then** el `tablist` superior tiene `aria-orientation="vertical"`  
**And** las diez pestañas superiores siguen presentes y en el mismo orden

#### Scenario: Las pestañas superiores se anuncian como verticales

**Given** el SettingsDialog abierto  
**When** se renderiza  
**Then** el elemento `role="tablist"` que contiene «Perfil», «Interfaz», «Prompts», «Claves»,
«Memoria», «Generación», «Skills», «Enrutador de skills», «Integraciones» y «Notificaciones» tiene
`aria-orientation="vertical"`  
**And** contiene las diez pestañas, sin desplegable de desbordamiento

#### Scenario: Las sub-pestañas de «Prompts», «Memoria» y «Generación» son verticales

**Given** el SettingsDialog abierto  
**When** el usuario abre «Prompts»  
**Then** el `tablist` anidado de System, Archivist, Collapse, Consolidator, Timeline y Journal tiene
`aria-orientation="vertical"`  
**And** al abrir «Memoria» el `tablist` de Episódica y Persistente es también vertical  
**And** al abrir «Generación» el `tablist` de Chat, Colapso, Fichas, Consolidación y Línea temporal
es también vertical

#### Scenario: Las sub-pestañas de «Skills» y «Integraciones» son verticales

**Given** el SettingsDialog abierto  
**When** el usuario abre «Skills»  
**Then** el `tablist` con una pestaña por skill tiene `aria-orientation="vertical"`  
**And** al abrir «Integraciones» el `tablist` de las ocho integraciones tiene
`aria-orientation="vertical"`  
**And** el orden de las ocho se mantiene y la sub-pestaña activa por defecto sigue siendo «Strava»

#### Scenario: La orientación no altera el comportamiento de los formularios

**Given** el SettingsDialog abierto en «Integraciones» → «OpenWeatherMap»  
**When** el usuario escribe una clave y pulsa el «Guardar» de esa sub-pestaña  
**Then** `updateSettings` recibe `openweather_api_key` con el valor escrito  
**And** el panel activo sigue sin mostrar los campos de las otras integraciones  
**And** guardar desde «Interfaz» sigue enviando el formulario completo de ajustes

## MODIFIED Requirements
### Requirement: SettingsDialog SHALL organize the Integraciones tab in sub-tabs

La pestaña «Integraciones» del SettingsDialog SHALL organizarse en **sub-pestañas verticales**
(antd `Tabs` anidado con `tabPosition="left"`), en este orden exacto:

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
- **Then** se muestran ocho sub-pestañas en vertical, con `aria-orientation="vertical"` en su `tablist`
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


## REMOVED Requirements

### Requirement: SettingsDialog SHALL fit all its top-level tabs without overflow

**Reason**: la orientación vertical elimina el problema que este requisito resolvía. Su texto
exigía un `Modal` de al menos 960 px «para que sus siete pestañas superiores quepan en una sola
fila, sin que ninguna se oculte en el desplegable de desbordamiento»; con la lista lateral no hay
fila que desbordar. Además su enunciado estaba obsoleto: enumeraba siete pestañas —Perfil,
Interfaz, Prompts, API Keys, Memoria, Generación y Herramientas— cuando la pestaña «API Keys» se
renombró a «Claves» y «Herramientas» desapareció, y hoy son diez.

**Migration**: el ancho de 1000 px se conserva y queda fijado por el requisito añadido «El diálogo
de ajustes SHALL mostrar todas sus pestañas en vertical». La comprobación de que existen exactamente
diez pestañas superiores ya la cubre el test `shows exactly ten top-level tabs and no Herramientas
tab` de `SettingsDialog.test.tsx`, que se mantiene.
