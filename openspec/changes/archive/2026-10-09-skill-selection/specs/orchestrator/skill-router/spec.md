## MODIFIED Requirements

### Requirement: El catálogo SHALL poder consultarse por la API

La aplicación SHALL exponer el catálogo cerrado de skills en modo lectura —el id de cada skill, la clave y el encabezado de su fragmento, las herramientas que cubre y si está **habilitada**— junto al conjunto core no enrutable, para que la interfaz no duplique el catálogo en su propio código.

#### Scenario: La consulta devuelve el catálogo completo
- **Given** la aplicación en marcha
- **When** se consulta el catálogo de skills
- **Then** devuelve las seis skills con las herramientas de cada una
- **And** devuelve el conjunto core por separado
- **And** ninguna herramienta del catálogo deja de existir en el registry

#### Scenario: La consulta expone si cada skill está habilitada
- **Given** una skill con `ROUTER_SKILL_<ID>_ENABLED` ausente y otra con la clave a `false`
- **When** se consulta el catálogo por la API
- **Then** la primera figura habilitada
- **And** la segunda figura deshabilitada

### Requirement: El router SHALL decidir una sola vez por turno, con una pregunta por skill enrutable habilitada

Con el enrutador activo, la decisión SHALL tomarse **una única vez por turno**, antes del bucle ReAct, y SHALL enviarse al clasificador el mensaje actual junto con los últimos `ROUTER_HISTORY_TURNS` turnos. Cada skill enrutable **habilitada** SHALL dar lugar a una pregunta de tipo `noul`, y todas SHALL viajar en **una sola petición**. Las skills **deshabilitadas** SHALL NOT generar pregunta. Si no queda ninguna skill enrutable habilitada, SHALL NOT invocarse al clasificador.

#### Scenario: Una sola decisión por turno
- **Given** un turno que consume tres iteraciones del bucle ReAct
- **When** se cuenta las invocaciones al clasificador
- **Then** se ha invocado exactamente una vez
- **And** las tres iteraciones usan el mismo conjunto de herramientas

#### Scenario: No se pregunta por lo que no se puede ofrecer
- **Given** una skill deshabilitada
- **When** se construyen las preguntas del clasificador
- **Then** no existe pregunta para esa skill
- **And** las demás skills habilitadas siguen preguntándose

#### Scenario: Sin skills enrutables no hay llamada
- **Given** todas las skills deshabilitadas
- **When** se procesa un turno
- **Then** no se invoca al clasificador
- **And** la petición lleva únicamente las herramientas del core

#### Scenario: Los turnos previos entran en el estado
- **Given** un mensaje elíptico («y mañana también») y un turno previo sobre la agenda
- **When** se construye el estado del clasificador
- **Then** el estado incluye los últimos turnos junto al mensaje actual

### Requirement: La selección SHALL gobernar también el turno sin herramientas

Si ninguna probabilidad alcanza **el umbral efectivo de su skill**, la selección SHALL ser vacía y el turno SHALL exponer **únicamente el conjunto core**.

#### Scenario: Un turno conversacional no expone herramientas enrutables
- **Given** el enrutador activo y un turno conversacional sin petición de acción (p. ej. «gracias, perfecto»)
- **When** todas las probabilidades quedan por debajo del umbral efectivo de su skill
- **Then** la selección es vacía
- **And** la petición lleva solo las herramientas del core

### Requirement: El enrutador SHALL fallar abierto

Ante cualquier fallo, el resultado SHALL ser **el comportamiento sin enrutador respetando el filtro de skills deshabilitadas**: el conjunto core más las herramientas de todas las skills **habilitadas**, y el prompt base intacto, registrando un warning. Una skill deshabilitada SHALL NOT exponer sus herramientas ni inyectar su fragmento en ningún caso. Los casos de fallo SHALL ser: enrutador deshabilitado, ausencia de credencial de OpenRouter, ausencia de skills habilitadas, error HTTP, timeout, respuesta ilegible o sin respuestas, e ids desconocidos (que SHALL ignorarse aplicando el resto).

#### Scenario: Un error del clasificador no quita capacidades
- **Given** el enrutador activo y un clasificador que devuelve un error HTTP
- **When** se procesa el turno
- **Then** la petición lleva las herramientas de todas las skills habilitadas más el core
- **And** se registra un warning

#### Scenario: Un timeout no bloquea el turno
- **Given** un clasificador que no responde dentro de `ROUTER_TIMEOUT_MS`
- **When** se procesa el turno
- **Then** el turno continúa con las herramientas de las skills habilitadas
- **And** la latencia añadida no supera el timeout configurado

#### Scenario: Los ids desconocidos se ignoran
- **Given** una respuesta con una skill desconocida y una válida por encima del umbral
- **When** se construye la selección
- **Then** la skill desconocida se ignora
- **And** la válida se activa

#### Scenario: Una skill deshabilitada no aparece ni al fallar abierto
- **Given** una skill deshabilitada y un clasificador que devuelve un error
- **When** se procesa el turno
- **Then** las herramientas de esa skill NO figuran en la petición
- **And** su fragmento NO se inyecta

## REMOVED Requirements

### Requirement: El conjunto expuesto SHALL ser el core más las skills seleccionadas y SHALL NOT acotar la ejecución

### Requirement: La configuración del enrutador SHALL vivir en settings, leerse en cada turno y arrancar encendida

## ADDED Requirements

### Requirement: El conjunto expuesto SHALL ser el core más las skills seleccionadas y habilitadas y SHALL NOT acotar la ejecución

El conjunto anunciado al modelo SHALL ser `core ∪ (skills_seleccionadas ∩ skills_habilitadas)`, y SHALL NOT incluir jamás una herramienta de una skill deshabilitada. El filtrado SHALL afectar **solo al anuncio**: `ToolRegistry::execute` SHALL seguir resolviendo cualquier herramienta registrada y SHALL NOT ser una frontera de seguridad.

#### Scenario: Una skill deshabilitada no se expone aunque fuera seleccionada
- **Given** una respuesta del clasificador que activa una skill deshabilitada
- **When** se compone el conjunto expuesto
- **Then** las herramientas de esa skill no figuran en él
- **And** el resto de la selección se expone con normalidad

#### Scenario: El enrutado no es una capa de seguridad
- **Given** una herramienta registrada que no se expuso por no estar seleccionada su skill
- **When** se invoca su ejecución en el registry
- **Then** la ejecución SHALL resolverse con normalidad
- **And** los permisos y guardrails SHALL seguir siendo la única capa que autoriza

#### Scenario: Activar una skill añade sus herramientas al conjunto expuesto
- **Given** una selección que activa la skill de agenda
- **When** se compone el conjunto expuesto
- **Then** figuran `calendar` junto a las herramientas del core
- **And** si la selección activa además la skill de entorno, figuran también `weather` y `geocode`
- **And** las herramientas de las skills no activadas no figuran

### Requirement: La configuración del enrutador SHALL vivir en settings con habilitación por skill y umbral por skill

La configuración SHALL vivir en la tabla `settings` —`ROUTER_ENABLED` (sembrado como `true`; un valor **ausente o ilegible** SHALL caer al default compilado `false`), `ROUTER_MODEL`, **el umbral por skill `ROUTER_THRESHOLD_<ID>` (sin ningún umbral global)**, la habilitación por skill `ROUTER_SKILL_<ID>_ENABLED` (por defecto habilitada; ausencia = habilitada), `ROUTER_TIMEOUT_MS`, `ROUTER_HISTORY_TURNS` (por defecto `6`) y las claves de criterios y fragmentos por skill—, sembrada y ajustada por migraciones **idempotentes** que SHALL fijar un valor solo cuando siga siendo **exactamente el sembrado**. SHALL leerse en cada turno de modo que editarla surta efecto sin reiniciar. El umbral efectivo de una skill SHALL ser su `ROUTER_THRESHOLD_<ID>` si existe y, si no, su umbral por defecto compilado; un valor ausente o ilegible SHALL caer al default registrando un warning. El endpoint y la credencial SHALL proceder del entorno.

#### Scenario: Por defecto el enrutado está encendido
- **Given** una base de datos recién migrada
- **When** se lee la configuración del enrutador
- **Then** `ROUTER_ENABLED` es `true`
- **And** el enrutador queda habilitado

#### Scenario: Por defecto el enrutado está apagado
- **Given** una tabla `settings` sin la clave `ROUTER_ENABLED`
- **When** se lee la configuración del enrutador
- **Then** el enrutador queda apagado

#### Scenario: Por defecto una skill está habilitada
- **Given** una base de datos sin `ROUTER_SKILL_<ID>_ENABLED` para una skill
- **When** se lee la configuración del enrutador
- **Then** esa skill queda habilitada

#### Scenario: El umbral por skill no depende de un global
- **Given** `ROUTER_THRESHOLD_WIDGETS` en `0.20` y ninguna otra clave de umbral
- **When** se resuelve el umbral efectivo de cada skill
- **Then** `widgets` usa `0.20`
- **And** las skills sin clave usan su umbral por defecto compilado

#### Scenario: Un umbral ilegible cae al default
- **Given** un `ROUTER_THRESHOLD_<ID>` con un valor no numérico o fuera de `[0, 1]`
- **When** se resuelve el umbral efectivo
- **Then** se usa el umbral por defecto de la skill
- **And** se registra un warning

#### Scenario: Encender surte efecto sin reiniciar
- **Given** el enrutado apagado
- **When** se pone `ROUTER_ENABLED` a `true` desde los ajustes
- **Then** el turno siguiente ya enruta
