# orchestrator/skill-router Specification

## Purpose
Enrutado de skills por turno: qué habilidades necesita un turno, con qué herramientas se responde, qué guía de prompt se activa, cómo se comporta el sistema cuando el clasificador falla y con qué instrumento se mide si el enrutado merece la pena.

## Requirements

### Requirement: El catálogo de skills SHALL ser cerrado y cubrir todas las herramientas con sus prerrequisitos

El catálogo SHALL declarar, en código, un conjunto **cerrado** de skills. Cada skill SHALL declarar un id estable, las instrucciones y las `criteria` con las que se pregunta por ella, el conjunto de herramientas que cubre —**incluyendo los prerrequisitos**— y la clave y el encabezado de su fragmento de prompt. Una herramienta PUEDE pertenecer a más de una skill. El conjunto **core** (`render_widget`, `get_current_time`) SHALL NOT ser enrutable y SHALL exponerse siempre. Toda herramienta registrada en el registry SHALL pertenecer al core o a al menos una skill.

#### Scenario: Toda herramienta registrada está cubierta
- **Given** el registry de producción con sus trece herramientas
- **When** se contrasta con el catálogo de skills y el conjunto core
- **Then** cada herramienta pertenece al core o a alguna skill
- **And** el test de integridad falla si se registra una herramienta y se olvida en el catálogo

#### Scenario: Una skill cubre los prerrequisitos de sus herramientas
- **Given** la skill del clima, cuyas herramientas necesitan coordenadas
- **When** se inspeccionan las herramientas que cubre
- **Then** incluye `geocode` además de `weather`
- **And** la skill de lugares incluye `geocode` y `reverse_geocode` junto a `search_places`

#### Scenario: El conjunto core no se enruta
- **Given** el catálogo de skills
- **When** se buscan `render_widget` y `get_current_time` entre las skills enrutables
- **Then** no pertenecen a ninguna skill enrutable
- **And** se exponen siempre, con independencia de la selección

### Requirement: El catálogo SHALL poder consultarse por la API

La aplicación SHALL exponer el catálogo cerrado de skills en modo lectura —el id de cada skill, la clave y el encabezado de su fragmento y las herramientas que cubre— junto al conjunto core no enrutable, para que la interfaz no duplique el catálogo en su propio código.

#### Scenario: La consulta devuelve el catálogo completo
- **Given** la aplicación en marcha
- **When** se consulta el catálogo de skills
- **Then** devuelve las ocho skills con las herramientas de cada una
- **And** devuelve el conjunto core por separado
- **And** ninguna herramienta del catálogo deja de existir en el registry

### Requirement: El router SHALL decidir una sola vez por turno, con una pregunta por skill enrutable habilitada

Con el enrutador activo, la decisión SHALL tomarse **una única vez por turno**, antes del bucle ReAct, y SHALL enviarse al clasificador el mensaje actual junto con los últimos `ROUTER_HISTORY_TURNS` turnos. Cada skill enrutable **cuyas herramientas estén al menos parcialmente habilitadas** SHALL dar lugar a una pregunta de tipo `noul`, y todas SHALL viajar en **una sola petición**. Las skills con todas sus herramientas deshabilitadas SHALL NOT generar pregunta. Si no queda ninguna skill enrutable, SHALL NOT invocarse al clasificador.

#### Scenario: Una sola decisión por turno
- **Given** un turno que consume tres iteraciones del bucle ReAct
- **When** se cuenta las invocaciones al clasificador
- **Then** se ha invocado exactamente una vez
- **And** las tres iteraciones usan el mismo conjunto de herramientas

#### Scenario: No se pregunta por lo que no se puede ofrecer
- **Given** la herramienta `calendar` deshabilitada desde la pestaña «Herramientas»
- **When** se construyen las preguntas del clasificador
- **Then** no existe pregunta para la skill de agenda
- **And** las demás skills enrutables siguen preguntándose

#### Scenario: Sin skills enrutables no hay llamada
- **Given** todas las herramientas enrutables deshabilitadas
- **When** se procesa un turno
- **Then** no se invoca al clasificador
- **And** la petición lleva todas las herramientas habilitadas

#### Scenario: Los turnos previos entran en el estado
- **Given** un mensaje elíptico («y mañana también») y un turno previo sobre la agenda
- **When** se construye el estado del clasificador
- **Then** el estado incluye los últimos turnos junto al mensaje actual

### Requirement: La selección SHALL gobernar también el turno sin herramientas

Si ninguna probabilidad alcanza `ROUTER_THRESHOLD`, la selección SHALL ser vacía y el turno SHALL exponer **únicamente el conjunto core**.

#### Scenario: Un turno conversacional no expone herramientas enrutables
- **Given** el enrutador activo y un saludo sin petición de acción
- **When** todas las probabilidades quedan por debajo del umbral
- **Then** la selección es vacía
- **And** la petición lleva solo las herramientas del core

### Requirement: El enrutador SHALL fallar abierto

Ante cualquier fallo, el resultado SHALL ser **el comportamiento sin enrutador**: todas las herramientas habilitadas y el prompt base intacto, registrando un warning. Los casos SHALL ser: enrutador deshabilitado, ausencia de credencial de OpenRouter, ausencia de skills enrutables, error HTTP, timeout, respuesta ilegible o sin respuestas, e ids desconocidos (que SHALL ignorarse aplicando el resto).

#### Scenario: Un error del clasificador no quita capacidades
- **Given** el enrutador activo y un clasificador que devuelve un error HTTP
- **When** se procesa el turno
- **Then** la petición lleva todas las herramientas habilitadas
- **And** se registra un warning

#### Scenario: Un timeout no bloquea el turno
- **Given** un clasificador que no responde dentro de `ROUTER_TIMEOUT_MS`
- **When** se procesa el turno
- **Then** el turno continúa con todas las herramientas habilitadas
- **And** la latencia añadida no supera el timeout configurado

#### Scenario: Los ids desconocidos se ignoran
- **Given** una respuesta con una skill desconocida y una válida por encima del umbral
- **When** se construye la selección
- **Then** la skill desconocida se ignora
- **And** la válida se activa

### Requirement: El conjunto expuesto SHALL ser el core más las skills seleccionadas y SHALL NOT acotar la ejecución

El conjunto anunciado al modelo SHALL ser `core ∪ skills_seleccionadas ∩ habilitadas`, y SHALL NOT incluir jamás una herramienta deshabilitada. El filtrado SHALL afectar **solo al anuncio**: `ToolRegistry::execute` SHALL seguir resolviendo cualquier herramienta habilitada y SHALL NOT ser una frontera de seguridad.

#### Scenario: Una herramienta deshabilitada no se expone aunque su skill se seleccione
- **Given** la skill de tareas seleccionada y la herramienta `tasks` deshabilitada
- **When** se compone el conjunto expuesto
- **Then** `tasks` no figura en él
- **And** el resto de la selección se expone con normalidad

#### Scenario: El enrutado no es una capa de seguridad
- **Given** una herramienta habilitada que no se expuso por no estar seleccionada su skill
- **When** se invoca su ejecución en el registry
- **Then** la ejecución SHALL resolverse con normalidad
- **And** los permisos y guardrails SHALL seguir siendo la única capa que autoriza

#### Scenario: Activar una skill añade sus herramientas al conjunto expuesto
- **Given** una selección que activa la skill de agenda
- **When** se compone el conjunto expuesto
- **Then** figuran `calendar` junto a las herramientas del core
- **And** si la selección activa además la skill de clima, figuran también `weather` y `geocode`
- **And** las herramientas de las skills no activadas no figuran

### Requirement: Los fragmentos de prompt SHALL inyectarse solo para las skills activas y sin duplicar

Los fragmentos SHALL leerse de `settings.SKILL_<ID>_PROMPT` y SHALL añadirse al mensaje de sistema **solo para las skills activas**, en el orden del catálogo, después del prompt base y antes de las secciones compuestas en código. Un fragmento vacío o en blanco SHALL NOT dejar rastro alguno. Un fragmento cuyo encabezado **ya esté presente en el prompt base** SHALL NOT inyectarse. El prompt de personalidad (`settings.system_prompt`) SHALL NOT modificarse.

#### Scenario: Solo se inyectan los fragmentos de las skills activas
- **Given** una selección con la skill de agenda y la de tareas
- **When** se compone el mensaje de sistema
- **Then** contiene el fragmento de agenda y el de tareas, en el orden del catálogo
- **And** no contiene los fragmentos de las skills no seleccionadas

#### Scenario: Un fragmento vacío no deja rastro
- **Given** una skill activa cuyo fragmento está vacío o en blanco
- **When** se compone el mensaje de sistema
- **Then** no aparece su encabezado, ni un separador, ni una línea en blanco de más

#### Scenario: Un fragmento ya presente en el prompt base no se duplica
- **Given** un prompt base que ya contiene el encabezado del fragmento de una skill activa
- **When** se compone el mensaje de sistema
- **Then** ese fragmento no se añade
- **And** el resto de fragmentos activos sí

#### Scenario: El prompt de personalidad queda intacto
- **Given** un `settings.system_prompt` personalizado por el usuario
- **When** se procesa un turno con el enrutador activo
- **Then** el prompt base llega al modelo sin modificaciones

### Requirement: La configuración del enrutador SHALL vivir en settings, leerse en cada turno y arrancar apagada

La configuración SHALL vivir en la tabla `settings` —`ROUTER_ENABLED` (por defecto `false`), `ROUTER_MODEL`, `ROUTER_THRESHOLD` (por defecto `0.3`), `ROUTER_TIMEOUT_MS`, `ROUTER_HISTORY_TURNS` y un `SKILL_<ID>_PROMPT` por skill—, sembrada por una migración **idempotente que SHALL NOT sobrescribir valores existentes**, y SHALL leerse en cada turno de modo que editarla surta efecto sin reiniciar. Un valor ilegible SHALL caer al default registrando un warning. El endpoint y la credencial SHALL proceder del entorno.

#### Scenario: Por defecto el enrutado está apagado
- **Given** una base de datos recién migrada
- **When** se procesa un turno
- **Then** `ROUTER_ENABLED` es `false`
- **And** la petición lleva todas las herramientas habilitadas

#### Scenario: Encender surte efecto sin reiniciar
- **Given** el enrutado apagado
- **When** se pone `ROUTER_ENABLED` a `true` desde los ajustes
- **Then** el turno siguiente ya enruta

#### Scenario: Un umbral ilegible cae al default
- **Given** `ROUTER_THRESHOLD` con un valor no numérico
- **When** se lee la configuración
- **Then** se usa `0.3`
- **And** se registra un warning

### Requirement: El enrutador SHALL registrar su decisión y su coste sin alterar las estadísticas del chat

Cada decisión SHALL registrarse con `tracing`, incluyendo las skills seleccionadas, sus probabilidades, la fuente (clasificador o fallo abierto), la latencia y el coste devuelto por el servicio. El enrutador SHALL NOT escribir en la tabla de estadísticas ni alterar `last_api_call`, para no falsear la latencia y el coste del modelo de chat.

#### Scenario: La decisión queda registrada
- **Given** un turno enrutado
- **When** se inspecciona el log
- **Then** figuran las skills seleccionadas, sus probabilidades, la fuente, la latencia y el coste

#### Scenario: Las estadísticas no se contaminan
- **Given** un turno con enrutado y una llamada de chat
- **When** se consultan las estadísticas
- **Then** no existe ninguna fila correspondiente al clasificador
- **And** `last_api_call` sigue describiendo el chat

### Requirement: El arnés de evaluación SHALL medir la cobertura de las herramientas realmente usadas

El arnés SHALL recorrer los turnos históricos cuyo mensaje de asistente registra herramientas usadas, ejecutar el enrutador sobre el mensaje del usuario y calcular la **cobertura**: la proporción de turnos en los que **toda** herramienta realmente usada figura en el conjunto que el enrutador habría expuesto. SHALL reportar además las activaciones por skill, la latencia p50 y p95, los tokens y el coste. SHALL aceptar `--limit`, `--threshold`, `--model` y `--dry-run` (catálogo sin red).

#### Scenario: Un turno no cubierto se reporta
- **Given** un turno histórico que usó `calendar::create_event`
- **When** el enrutador no selecciona la skill de agenda para ese mensaje
- **Then** ese turno cuenta como no cubierto
- **And** el informe lo señala junto a la cobertura agregada

#### Scenario: El modo sin red no llama al servicio
- **Given** el arnés en modo `--dry-run`
- **When** se ejecuta
- **Then** recorre el catálogo y el historial sin ninguna petición de red
