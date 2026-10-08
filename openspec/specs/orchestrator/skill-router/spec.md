# orchestrator/skill-router Specification

## Purpose
Enrutado de skills por turno: qué habilidades necesita un turno, con qué herramientas se responde, qué guía de prompt se activa, cómo se comporta el sistema cuando el clasificador falla y con qué instrumento se mide si el enrutado merece la pena.

## Requirements

### Requirement: El catálogo de skills SHALL ser cerrado y cubrir todas las herramientas con sus prerrequisitos

El catálogo SHALL declarar, en código, un conjunto **cerrado de seis skills de dominio amplio**: `agenda`, `pendientes`, `recuerdos`, `entorno`, `web` y `widgets`. Cada skill SHALL declarar un id estable, las instrucciones y las `criteria` por defecto con las que se pregunta por ella, su umbral por defecto, el conjunto de herramientas que cubre —**incluyendo los prerrequisitos**— y la clave y el encabezado de su fragmento de prompt. Las herramientas de un mismo dominio SHALL NOT repartirse entre skills distintas: `pendientes` cubre `tasks` y `reminders`; `recuerdos` cubre `notes` y `unified_search`; `entorno` cubre `weather`, `geocode`, `reverse_geocode` y `search_places`. El conjunto **core** no enrutable SHALL ser `get_current_time` y `get_current_location`. La herramienta `render_widget` SHALL pertenecer a la skill `widgets` y SHALL NOT estar en el core. Una herramienta PUEDE pertenecer a más de una skill. Toda herramienta registrada SHALL pertenecer al core o a al menos una skill.

#### Scenario: Toda herramienta registrada está cubierta
- **Given** el registry de producción con sus trece herramientas
- **When** se contrasta con el catálogo de skills y el conjunto core
- **Then** cada herramienta pertenece al core o a alguna skill
- **And** el test de integridad falla si se registra una herramienta y se olvida en el catálogo

#### Scenario: Las herramientas de un mismo dominio no se reparten en skills distintas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de `pendientes`, `recuerdos` y `entorno`
- **Then** `tasks` y `reminders` están ambas en `pendientes`
- **And** `notes` y `unified_search` están ambas en `recuerdos`
- **And** `weather`, `geocode`, `reverse_geocode` y `search_places` están todas en `entorno`

#### Scenario: El conjunto core no se enruta
- **Given** el catálogo de skills
- **When** se busca `get_current_time` y `get_current_location` entre las skills enrutables
- **Then** no pertenecen a ninguna skill enrutable
- **And** se exponen siempre, con independencia de la selección
- **And** `render_widget` no pertenece al core: se enruta con la skill `widgets`

#### Scenario: El widget se enruta con umbral propio
- **Given** la skill `widgets`
- **When** se inspecciona su umbral por defecto
- **Then** es más alto que el de las skills de dominio
- **And** cubre la herramienta `render_widget`

#### Scenario: Una skill cubre los prerrequisitos de sus herramientas
- **Given** la skill `entorno`, cuyas herramientas meteorológicas necesitan coordenadas
- **When** se inspeccionan las herramientas que cubre
- **Then** incluye `geocode` además de `weather`

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

La configuración SHALL vivir en la tabla `settings` —`ROUTER_ENABLED` (por defecto `false`), `ROUTER_MODEL`, `ROUTER_THRESHOLD` (por defecto `0.10`), los overrides por skill `ROUTER_THRESHOLD_<ID>`, `ROUTER_TIMEOUT_MS`, `ROUTER_HISTORY_TURNS` (por defecto `6`) y las claves de criterios y fragmentos por skill—, sembrada por una migración **idempotente que SHALL NOT sobrescribir valores existentes**, y SHALL leerse en cada turno de modo que editarla surta efecto sin reiniciar. El umbral efectivo de una skill SHALL ser su override si existe y, si no, su umbral por defecto; un valor ausente o ilegible SHALL caer al default registrando un warning. El endpoint y la credencial SHALL proceder del entorno. Los valores por defecto documentados proceden de la medición del arnés sobre el historial real, no de una elección de gusto.

#### Scenario: Por defecto el enrutado está apagado
- **Given** una base de datos recién migrada
- **When** se procesa un turno
- **Then** `ROUTER_ENABLED` es `false`
- **And** la petición lleva todas las herramientas habilitadas

#### Scenario: El umbral por skill se aplica por encima del global
- **Given** `ROUTER_THRESHOLD` en `0.10` y `ROUTER_THRESHOLD_WIDGETS` en `0.20`
- **When** se resuelve el umbral efectivo de cada skill
- **Then** las skills de dominio usan `0.10`
- **And** `widgets` usa `0.20`

#### Scenario: Un umbral ilegible cae al default
- **Given** `ROUTER_THRESHOLD` o un override `ROUTER_THRESHOLD_<ID>` con un valor no numérico o fuera de `[0, 1]`
- **When** se resuelve el umbral efectivo
- **Then** se usa el umbral por defecto correspondiente
- **And** se registra un warning

#### Scenario: Encender surte efecto sin reiniciar
- **Given** el enrutado apagado
- **When** se pone `ROUTER_ENABLED` a `true` desde los ajustes
- **Then** el turno siguiente ya enruta

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

El arnés SHALL recorrer los turnos históricos cuyo mensaje de asistente registra herramientas usadas, ejecutar el enrutador sobre el mensaje del usuario y calcular la **cobertura**: la proporción de turnos en los que **toda** herramienta realmente usada figura en el conjunto que el enrutador habría expuesto. SHALL usar los **valores efectivos** de `settings` —criterios, umbrales y número de turnos de historial—, de modo que el bucle de ajuste sea «editar y medir». SHALL **publicar la configuración efectiva** con la que ha medido (las skills, sus umbrales y qué campos están sobrescritos), para que cada cifra sea atribuible. SHALL reportar además las activaciones por skill, la latencia p50 y p95, los tokens y el coste, y SHALL aceptar `--limit`, `--threshold`, `--model`, `--overrides` y `--dry-run` (catálogo sin red).

El arnés SHALL medir además sobre el **estado que el clasificador recibe en producción**, y SHALL publicar la **palanca** que el enrutado mueve y la **varianza** de la medida:

- El contenido de cada mensaje SHALL ser `collapsed_content` cuando exista y `content` en caso contrario, igual que `orchestrator::agent`.
- La ventana SHALL ser el presupuesto de tokens de `settings.max_window_tokens` (por defecto `10000`) contado hacia atrás desde el turno, y SHALL NOT reiniciarse cuando dos turnos consecutivos no emparejen: un contexto que producción sí tendría no puede descartarse.
- SHALL instrumentar las **herramientas por turno** y el tamaño —en bytes y en tokens estimados con el estimador del propio proyecto— del bloque de definiciones expuesto frente al conjunto completo habilitado, y SHALL publicar el **ahorro** resultante. La medida SHALL tomarse de las **mismas definiciones** que viajan en la petición, no de una tabla de tamaños escrita a mano.
- `--repeat <N>` SHALL repetir la medición y publicar la cobertura de cada repetición junto al mínimo, la media y el máximo, de modo que la varianza se lea como rango.

El arnés SHALL además **explicar** cada turno no cubierto: por cada herramienta realmente usada que no se expuso, SHALL publicar la skill que la habría cubierto, la **probabilidad** que el clasificador le asignó, el **umbral efectivo** con el que se comparó y la **fuente** de la selección, y SHALL agregar la **proximidad al umbral** de los fallos para distinguir un near-miss de un miss semántico.

El arnés SHALL aceptar un **fichero de overrides** con umbrales (global y por skill) y criterios por skill, de modo que una variante de la campaña sea un artefacto versionado y reproducible que SHALL NOT requerir mutar `settings`. La precedencia SHALL ser la CLI por encima del fichero y el fichero por encima de `settings`, y el informe SHALL declarar qué overrides estaban activos. Un fichero declarado y ausente o ilegible SHALL fallar de forma ruidosa.

#### Scenario: El arnés mide con los criterios vigentes
- **Given** un criterio sobrescrito en `settings`
- **When** se ejecuta el arnés
- **Then** las peticiones al clasificador usan el criterio sobrescrito
- **And** el informe declara qué campos están sobrescritos

#### Scenario: Un turno no cubierto se reporta
- **Given** un turno histórico que usó `tasks` y el enrutador no expuso `tasks`
- **When** se calcula la cobertura
- **Then** ese turno cuenta como no cubierto
- **And** el informe señala la skill `pendientes` como la que lo habría cubierto

#### Scenario: El modo sin red no llama al servicio
- **Given** el arnés en modo `--dry-run`
- **When** se ejecuta
- **Then** recorre el catálogo y el historial sin ninguna petición de red
- **And** declara la configuración efectiva que habría usado
- **And** declara la ventana que habría usado

#### Scenario: El estado del clasificador es el que recibe producción
- **Given** un turno histórico cuyo historial incluye un mensaje con `collapsed_content`
- **When** el arnés construye el estado para el clasificador
- **Then** ese mensaje viaja con el contenido colapsado, no con el crudo
- **And** la ventana la fija `max_window_tokens` contada hacia atrás desde el turno
- **And** el historial no se reinicia porque la pareja anterior no empareje

#### Scenario: El informe publica las herramientas por turno y el ahorro
- **Given** una ejecución medida
- **When** el arnés informa
- **Then** publica las herramientas medias por turno
- **And** publica los bytes y los tokens estimados del bloque expuesto frente al conjunto completo habilitado
- **And** publica el ahorro resultante

#### Scenario: La varianza se publica
- **Given** el arnés invocado con `--repeat 3`
- **When** mide
- **Then** repite la medición tres veces
- **And** publica la cobertura de cada repetición y el mínimo, la media y el máximo

#### Scenario: Un fallo se explica con su probabilidad y su umbral
- **Given** un turno no cubierto cuya herramienta faltante habría necesitado una skill
- **When** el arnés informa
- **Then** publica esa skill, la probabilidad que el clasificador le dio, el umbral efectivo aplicado y la fuente de la selección

#### Scenario: La proximidad al umbral se agrega
- **Given** varios turnos no cubiertos con distinta distancia al umbral
- **When** el arnés agrega el diagnóstico
- **Then** cuenta cuántos fallos quedaron a menos de cada distancia declarada

#### Scenario: Una variante de overrides no toca la base de datos
- **Given** un fichero de overrides con umbrales y criterios
- **When** se ejecuta el arnés
- **Then** la medición usa esos valores
- **And** `settings` no se modifica

#### Scenario: La precedencia de overrides es CLI > fichero > settings
- **Given** un umbral presente en la CLI, en el fichero y en `settings` con valores distintos
- **When** se resuelve el umbral efectivo
- **Then** se usa el de la CLI
- **And** sin flag, se usa el del fichero; y sin fichero, el de `settings`

### Requirement: Las preguntas del clasificador SHALL ser editables desde settings con el valor compilado como respaldo

Las instrucciones y las `criteria` con las que se pregunta al clasificador SHALL poder sobrescribirse desde `settings`, con una clave por campo y skill (`SKILL_<ID>_QUESTION`, `SKILL_<ID>_CRITERIA_TRUE`, `SKILL_<ID>_CRITERIA_FALSE`). El valor **declarado en el catálogo** SHALL ser el default y el respaldo: una clave ausente, vacía o solo con espacios en blanco SHALL hacer que se use el valor del código, de modo que una edición a medias no pueda enviar una pregunta sin criterios ni degradar el enrutado en silencio. Los valores SHALL leerse en cada turno. La consulta del catálogo por la API SHALL exponer el valor **efectivo** de cada campo y si está sobrescrito.

#### Scenario: Sin override se usa el valor del catálogo
- **Given** una base de datos sin la clave de la pregunta de una skill
- **When** se construye su pregunta para el clasificador
- **Then** se usan las instrucciones y las criteria declaradas en el catálogo

#### Scenario: Un override surte efecto en el mismo turno
- **Given** una pregunta sobrescrita en `settings`
- **When** se procesa un turno
- **Then** la pregunta enviada al clasificador es la sobrescrita
- **And** no hace falta reiniciar

#### Scenario: Un campo en blanco nunca envía una pregunta sin criterios
- **Given** una clave de criterio presente pero con solo espacios en blanco
- **When** se construye la pregunta
- **Then** se usa el valor declarado en el catálogo
- **And** no se envía ningún criterio vacío

#### Scenario: La API expone el valor efectivo y la marca de sobrescrito
- **Given** una skill con su pregunta sobrescrita y sus criterios sin tocar
- **When** se consulta el catálogo por la API
- **Then** la pregunta devuelta es la sobrescrita y figura marcada como sobrescrita
- **And** los criterios devueltos son los del catálogo, sin la marca

### Requirement: La guía de widgets SHALL residir en el fragmento de su skill

La guía de uso de `render_widget` SHALL NOT residir de forma permanente en `settings.system_prompt`, porque la herramienta deja de estar siempre expuesta y el prompt base no puede ordenar el uso de una herramienta ausente. Una migración SHALL retirar ese bloque del prompt base **solo si se encuentra verbatim** (identificado por su encabezado) y SHALL depositarlo como contenido de `SKILL_WIDGETS_PROMPT`; si el bloque no se encuentra verbatim, SHALL NOT retirar nada y el fragmento SHALL sembrarse igualmente. La regla anti-duplicado del ensamblador de fragmentos SHALL seguir vigente como red de seguridad. Con la skill `widgets` inactiva SHALL NOT enviarse ni el esquema de la herramienta ni la guía.

#### Scenario: El bloque verbatim se muda al fragmento
- **Given** un prompt base que contiene el bloque de widgets tal y como lo añadió la migración original
- **When** se aplica la migración
- **Then** el bloque deja de estar en el prompt base
- **And** su contenido queda en `SKILL_WIDGETS_PROMPT`

#### Scenario: Un bloque editado no se retira
- **Given** un prompt base en el que el usuario ha modificado el bloque de widgets
- **When** se aplica la migración
- **Then** el prompt base conserva el texto del usuario
- **And** el fragmento se siembra igualmente
- **And** la regla anti-duplicado impide que la guía viaje dos veces

#### Scenario: Con la skill inactiva no viaja la guía
- **Given** un turno en el que la skill `widgets` no se selecciona
- **When** se compone el mensaje de sistema
- **Then** no contiene la guía de widgets
- **And** la herramienta `render_widget` no figura entre las ofrecidas

#### Scenario: La migración es idempotente
- **Given** una base de datos ya migrada
- **When** se vuelve a aplicar la migración
- **Then** el prompt base y el fragmento quedan igual
