# orchestrator/skill-router Specification

## Purpose
Enrutado de skills por turno: qué habilidades necesita un turno, con qué herramientas se responde, qué guía de prompt se activa, cómo se comporta el sistema cuando el clasificador falla y con qué instrumento se mide si el enrutado merece la pena.

## Requirements

### Requirement: El catálogo de skills SHALL ser cerrado y cubrir todas las herramientas con sus prerrequisitos

El catálogo SHALL declarar, en código, un conjunto **cerrado de trece skills de dominio amplio**: `agenda`, `pendientes`, `recuerdos`, `entorno`, `web`, `widgets`, `running`, `timeline`, `email`, `rss`, `blog`, `zennotes` y `menus`. Cada skill SHALL declarar un id estable, las instrucciones y las `criteria` por defecto con las que se pregunta por ella, su umbral por defecto, el conjunto de herramientas que cubre —**incluyendo los prerrequisitos**— y la clave y el encabezado de su fragmento de prompt. Las herramientas de un mismo dominio SHALL NOT repartirse entre skills distintas: `pendientes` cubre `tasks`, `reminders` y `notifications`; `recuerdos` cubre `notes`, `unified_search` y `journal`; `entorno` cubre `weather`, `geocode`, `reverse_geocode` y `search_places`; `running` cubre `strava_recent_activities`, `strava_activity_detail`, `strava_activity_streams` y `strava_athlete_stats`; `timeline` cubre `timeline_get_events`, `timeline_add_event` y `timeline_delete_event`; `email` cubre `email_list_unread`, `email_get_body`, `email_mark_read` y `email_send`; `rss` cubre `rss_list_unread`, `rss_get_entry`, `rss_search_entries` y `rss_mark_read`; `blog` cubre `blog_comment_reply` y `blog_comment_delete`; `zennotes` cubre la herramienta `zennotes`, y **solo** ella; `menus` cubre la herramienta `oximeal`, y **solo** ella. Las `criteria` de `recuerdos` SHALL incluir el habla de la bitácora (`bitácora`, `diario`, `cómo fue mi día`); las de `zennotes` SHALL reconocer el **vault de ZenNotes** —sus notas, sus artículos y sus apuntes («mis notas de ZenNotes», «mi vault», «apunta en mis notas del vault»)—, de modo que el router seleccione `zennotes` para leer o escribir en el vault y no lo confunda con la store interna de `recuerdos`; y las de `menus` SHALL reconocer el **recetario y la cocina de OxiMeal** —recetas, menú o plan de la semana, comidas, cena, comida y lista de la compra («prepárame el menú de la semana», «qué cenamos», «añade esto a la compra», «guárdame esta receta»)—, de modo que el router seleccione `menus` para leer o escribir recetas, planificar comidas y gestionar la compra, sin confundirlo con `recuerdos` (que es la store interna) ni con `pendientes` (que son tareas). El conjunto **core** no enrutable SHALL ser `get_current_time` y `get_current_location`. La herramienta `render_widget` SHALL pertenecer a la skill `widgets` y SHALL NOT estar en el core. Una herramienta PUEDE pertenecer a más de una skill. Toda herramienta registrada SHALL pertenecer al core o a al menos una skill.

#### Scenario: Toda herramienta registrada está cubierta
- **Given** el registry de producción con sus treinta y cuatro herramientas
- **When** se contrasta con el catálogo de skills y el conjunto core
- **Then** cada herramienta pertenece al core o a alguna skill
- **And** el test de integridad falla si se registra una herramienta y se olvida en el catálogo

#### Scenario: Las herramientas de un mismo dominio no se reparten en skills distintas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de `pendientes`, `recuerdos`, `entorno`, `running`, `timeline`, `email`, `rss`, `blog`, `zennotes` y `menus`
- **Then** `tasks`, `reminders` y `notifications` están todas en `pendientes`
- **And** `notes`, `unified_search` y `journal` están todas en `recuerdos`
- **And** `weather`, `geocode`, `reverse_geocode` y `search_places` están todas en `entorno`
- **And** las cuatro herramientas `strava_*` están todas en `running`
- **And** las tres herramientas `timeline_*` están todas en `timeline`
- **And** las cuatro herramientas `email_*` están todas en `email`
- **And** las cuatro herramientas `rss_*` están todas en `rss`
- **And** `blog_comment_reply` y `blog_comment_delete` están en `blog`
- **And** ni `blog_comment_reply` ni `blog_comment_delete` están en `pendientes`, donde solo se leen los avisos
- **And** `zennotes` está en `zennotes` y en ninguna otra skill
- **And** `oximeal` está en `menus` y en ninguna otra skill

#### Scenario: El conjunto core no se enruta
- **Given** el catálogo de skills
- **When** se busca `get_current_time` y `get_current_location` entre las skills enrutables
- **Then** no pertenecen a ninguna skill enrutable
- **And** se exponen siempre, con independencia de la selección
- **And** `render_widget` no pertenece al core: se enruta con la skill `widgets`

#### Scenario: El widget se enruta con umbral propio
- **Given** la skill `widgets`
- **When** se inspecciona su umbral por defecto
- **Then** es `0`, de modo que la skill no está condicionada por el clasificador
- **And** cubre la herramienta `render_widget`

#### Scenario: Una skill cubre los prerrequisitos de sus herramientas
- **Given** la skill `entorno`, cuyas herramientas meteorológicas necesitan coordenadas
- **When** se inspeccionan las herramientas que cubre
- **Then** incluye `geocode` además de `weather`

#### Scenario: La skill de running cubre sus cuatro herramientas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de la skill `running`
- **Then** cubre `strava_recent_activities`, `strava_activity_detail`, `strava_activity_streams` y `strava_athlete_stats`

#### Scenario: La skill del timeline cubre sus tres herramientas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de la skill `timeline`
- **Then** cubre `timeline_get_events`, `timeline_add_event` y `timeline_delete_event`
- **And** `timeline` es una skill enrutable, no parte del conjunto core

#### Scenario: La skill del correo cubre sus cuatro herramientas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de la skill `email`
- **Then** cubre `email_list_unread`, `email_get_body`, `email_mark_read` y `email_send`
- **And** `email` es una skill enrutable, no parte del conjunto core
- **And** declara el umbral `0.10` de las skills de dominio

#### Scenario: La skill del feed cubre sus cuatro herramientas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de la skill `rss`
- **Then** cubre `rss_list_unread`, `rss_get_entry`, `rss_search_entries` y `rss_mark_read`
- **And** `rss` es una skill enrutable, no parte del conjunto core
- **And** declara el umbral `0.10` de las skills de dominio

#### Scenario: La skill del blog cubre la publicación, y solo ella
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas y las `criteria` de la skill `blog`
- **Then** la skill cubre `blog_comment_reply`, de modo que responder a un comentario del blog —también cuando el comentario llega como aviso entrante— la selecciona
- **And** `blog` es una skill enrutable, no parte del conjunto core
- **And** declara el umbral `0.10` de las skills de dominio
- **And** la cobertura completa de la skill —las **dos** escrituras del blog— la fija el escenario «La skill del blog cubre las dos escrituras del blog», que **sustituye** al presente

#### Scenario: La skill del blog cubre las dos escrituras del blog
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de la skill `blog`
- **Then** cubre `blog_comment_reply` y `blog_comment_delete`, y ninguna otra herramienta
- **And** la herramienta nueva no añade una skill

#### Scenario: La skill del blog reconoce el borrado del comentario notificado
- **Given** el catálogo de skills
- **When** se inspeccionan las `criteria` de la skill `blog`
- **Then** reconocen el caso de mandar a la papelera el comentario que llegó como aviso
- **And** ninguna otra skill cubre `blog_comment_delete`

#### Scenario: La skill de recuerdos cubre la bitácora y su habla
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas y las `criteria` de la skill `recuerdos`
- **Then** cubre la herramienta `journal` además de `notes` y `unified_search`
- **And** `journal` no es parte del conjunto core ni se reparte en otra skill
- **And** sus `criteria` mencionan la bitácora, el diario y cómo fue el día

#### Scenario: La skill de ZenNotes cubre su herramienta, y solo ella
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de la skill `zennotes`
- **Then** cubre la herramienta `zennotes` y ninguna otra
- **And** es una skill enrutable, no parte del conjunto core
- **And** declara el umbral `0.10` de las skills de dominio

#### Scenario: La skill de ZenNotes distingue el vault del apunte interno
- **Given** el catálogo de skills
- **When** se inspeccionan las `criteria` de `zennotes` y de `recuerdos`
- **Then** las de `zennotes` mencionan el vault de ZenNotes, sus artículos y sus notas
- **And** ninguna otra skill cubre la herramienta `zennotes`

#### Scenario: La skill de Menús cubre su herramienta, y solo ella
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de la skill `menus`
- **Then** cubre la herramienta `oximeal` y ninguna otra
- **And** es una skill enrutable, no parte del conjunto core
- **And** declara el umbral `0.10` de las skills de dominio

#### Scenario: La skill de Menús distingue el recetario de OxiMeal de la store interna
- **Given** el catálogo de skills
- **When** se inspeccionan las `criteria` de `menus`, `recuerdos` y `pendientes`
- **Then** las de `menus` mencionan las recetas, el menú o plan de la semana, las comidas y la lista de la compra de OxiMeal
- **And** ninguna otra skill cubre la herramienta `oximeal`
- **And** `recuerdos` sigue cubriendo la store interna y `pendientes` las tareas

#### Scenario: La skill de Menús se registra en el catálogo
- **Given** el catálogo cerrado de skills
- **When** se enumeran sus ids
- **Then** figura `menus` junto a los otros doce ids
- **And** su fragmento de prompt usa la clave `SKILL_MENUS_PROMPT` y el encabezado `# SKILL ACTIVA: MENUS`

### Requirement: El catálogo SHALL poder consultarse por la API

La aplicación SHALL exponer el catálogo cerrado de skills en modo lectura —el id de cada skill, la clave y el encabezado de su fragmento, las herramientas que cubre y si está **habilitada**— junto al conjunto core no enrutable, para que la interfaz no duplique el catálogo en su propio código.

#### Scenario: La consulta devuelve el catálogo completo
- **Given** la aplicación en marcha
- **When** se consulta el catálogo de skills
- **Then** devuelve las diez skills con las herramientas de cada una
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

Un **umbral efectivo de cero** SHALL significar que la skill **no está condicionada por el
clasificador**: SHALL seleccionarse en todos los turnos, incluso cuando el clasificador no devuelva
respuesta para ella. Las skills con umbral mayor que cero SHALL seguir necesitando que su
probabilidad lo alcance. Por tanto, mientras exista al menos una skill habilitada con umbral cero,
la selección **nunca** SHALL ser vacía y el turno SHALL exponer, además del core, las herramientas de
esas skills. Solo cuando **ninguna** probabilidad alcanza el umbral efectivo de su skill **y ninguna
skill habilitada tiene umbral cero** la selección SHALL ser vacía y el turno SHALL exponer únicamente
el conjunto core.

La skill `widgets` SHALL declarar umbral por defecto `0`, de modo que `render_widget` SHALL figurar en
el conjunto expuesto en **todos** los turnos y su fragmento SHALL viajar en todos los mensajes de
sistema.

#### Scenario: Un turno conversacional no expone herramientas enrutables
- **Given** el enrutador activo, ninguna skill a umbral cero y un turno conversacional sin petición de acción (p. ej. «gracias, perfecto»)
- **When** todas las probabilidades quedan por debajo del umbral efectivo de su skill
- **Then** la selección es vacía
- **And** la petición lleva solo las herramientas del core

#### Scenario: Una skill a umbral cero se selecciona siempre
- **Given** la skill `widgets` con umbral efectivo `0` y un clasificador que devuelve `0.00` para ella
- **When** se procesa el turno
- **Then** `widgets` figura en la selección
- **And** `render_widget` figura en el conjunto expuesto
- **And** el fragmento de `widgets` figura en el mensaje de sistema

#### Scenario: Sin respuesta no se desactiva una skill a umbral cero
- **Given** la skill `widgets` con umbral efectivo `0` y una respuesta del clasificador **sin** entrada para `widgets`
- **When** se construye la selección
- **Then** `widgets` figura en la selección
- **And** `render_widget` figura en el conjunto expuesto

#### Scenario: Una skill a umbral cero deshabilitada no se selecciona
- **Given** la skill `widgets` con `ROUTER_SKILL_WIDGETS_ENABLED` a `false`
- **When** se procesa el turno
- **Then** `widgets` NO figura en la selección
- **And** `render_widget` NO figura en el conjunto expuesto
- **And** su fragmento NO se inyecta

#### Scenario: Un umbral mayor que cero sigue condicionando
- **Given** la skill `widgets` con `ROUTER_THRESHOLD_WIDGETS` a `0.20`
- **When** el clasificador devuelve `0.10` para ella
- **Then** `widgets` NO figura en la selección
- **And** `render_widget` NO figura en el conjunto expuesto

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

### Requirement: Los fragmentos de prompt SHALL inyectarse solo para las skills activas y sin duplicar

Los fragmentos SHALL leerse de `settings.SKILL_<ID>_PROMPT` y SHALL añadirse al mensaje de sistema
**solo para las skills activas**, en el orden del catálogo, después del prompt base y antes de las
secciones compuestas en código. Una skill SHALL considerarse **activa** si figura en la selección
**o** si está habilitada y su umbral efectivo es **cero**. Un fragmento vacío o en blanco SHALL NOT
dejar rastro alguno. Un fragmento cuyo encabezado **ya esté presente en el prompt base** SHALL NOT
inyectarse. El prompt de personalidad (`settings.system_prompt`) SHALL NOT modificarse.

El fragmento de una skill a umbral cero SHALL inyectarse también en los caminos de **fallo abierto**
—enrutador deshabilitado, sin clasificador, sin skills enrutables habilitadas, error, timeout o
respuesta inútil—, donde la selección va vacía pero el conjunto expuesto sí lleva sus herramientas.
Una skill **deshabilitada** SHALL NOT inyectar su fragmento en ningún caso.

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

#### Scenario: La skill a umbral cero inyecta su fragmento aunque no esté en la selección
- **Given** un fallo abierto del enrutador y la skill `widgets` habilitada con umbral `0`
- **When** se compone el mensaje de sistema
- **Then** la selección va vacía
- **And** el mensaje de sistema contiene el fragmento de `widgets`
- **And** la petición lleva `render_widget` entre las herramientas ofrecidas

#### Scenario: Una skill deshabilitada no inyecta ni al fallar abierto
- **Given** la skill `widgets` deshabilitada y un clasificador que devuelve un error
- **When** se compone el mensaje de sistema
- **Then** no contiene su fragmento
- **And** `render_widget` no figura entre las herramientas ofrecidas

### Requirement: El enrutador SHALL registrar su decisión y su coste sin alterar las estadísticas del chat

Cada decisión SHALL registrarse con `tracing`, incluyendo las skills seleccionadas, sus probabilidades, la fuente (clasificador o fallo abierto), la latencia y el coste devuelto por el servicio. Además, cuando hubo llamada al clasificador (fuente `Router` o `Error`), la selección SHALL exponer su telemetría —modelo, tokens de entrada y salida, coste, latencia y estado— para que el orquestador la persista (ver `orchestrator/agent`). El enrutador SHALL NOT escribir por sí mismo en la tabla de estadísticas ni alterar `last_api_call`; las agregaciones del chat excluyen las filas que no son `kind='chat'`, de modo que las estadísticas del chat no se contaminan.

#### Scenario: La decisión queda registrada
- **Given** un turno enrutado
- **When** se inspecciona el log
- **Then** figuran las skills seleccionadas, sus probabilidades, la fuente, la latencia y el coste
- **And** la selección expone la telemetría de la llamada al clasificador

#### Scenario: Las estadísticas no se contaminan
- **Given** un turno con enrutado y una llamada de chat
- **When** se consultan las estadísticas
- **Then** la petición del clasificador figura como una fila `kind='router'`
- **And** las agregaciones de chat (resumen, por modelo y por día) no la incluyen
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

### Requirement: Las criteria por defecto del briefing SHALL reconocer el saludo de apertura del día

Las `criteria_true` por defecto de las skills `agenda`, `pendientes`, `entorno` y `widgets` SHALL declarar el **saludo de apertura del día** («buenos días», «¿qué tal?») como caso afirmativo, porque en este asistente un saludo abre el **briefing matutino**: la agenda del día, lo pendiente, la previsión y un panel con los datos. Las skills `recuerdos` y `web` SHALL NOT declararlo. El resto de cada criterio SHALL seguir describiendo su dominio.

#### Scenario: Las cuatro skills del briefing declaran el saludo
- **Given** el catálogo por defecto
- **When** se leen las `criteria_true` de `agenda`, `pendientes`, `entorno` y `widgets`
- **Then** las cuatro declaran el saludo de apertura del día

#### Scenario: Las skills ajenas al briefing no lo declaran
- **Given** el catálogo por defecto
- **When** se leen las `criteria_true` de `recuerdos` y `web`
- **Then** ninguna declara el saludo de apertura del día

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

La configuración SHALL vivir en la tabla `settings` —`ROUTER_ENABLED` (sembrado como `true`; un valor
**ausente o ilegible** SHALL caer al default compilado `false`), `ROUTER_MODEL`, **el umbral por
skill `ROUTER_THRESHOLD_<ID>` (sin ningún umbral global)**, la habilitación por skill
`ROUTER_SKILL_<ID>_ENABLED` (por defecto habilitada; ausencia = habilitada), `ROUTER_TIMEOUT_MS`,
`ROUTER_HISTORY_TURNS` (por defecto `6`) y las claves de criterios y fragmentos por skill—, sembrada
y ajustada por migraciones **idempotentes** que SHALL fijar un valor solo cuando siga siendo
**exactamente el sembrado**. SHALL leerse en cada turno de modo que editarla surta efecto sin
reiniciar. El umbral efectivo de una skill SHALL ser su `ROUTER_THRESHOLD_<ID>` si existe y, si no,
su umbral por defecto compilado; un valor ausente o ilegible SHALL caer al default registrando un
warning.

La skill `widgets` SHALL tener umbral por defecto **`0`**, y su override sembrado
(`ROUTER_THRESHOLD_WIDGETS` a `0.20`) SHALL retirarse con una migración que SHALL borrar la clave
**solo si sigue valiendo `0.20`**. Un valor distinto SHALL conservarse: subir el umbral es la forma
de volver a condicionar la skill por el clasificador, y es una decisión del usuario.

El endpoint y la credencial SHALL proceder del entorno.

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
- **Given** ninguna clave `ROUTER_THRESHOLD_<ID>` en `settings`
- **When** se resuelve el umbral efectivo de cada skill
- **Then** `widgets` usa `0`, su umbral compilado
- **And** las demás skills usan su umbral por defecto compilado

#### Scenario: El override sembrado de widgets se retira
- **Given** `ROUTER_THRESHOLD_WIDGETS` con el valor sembrado `0.20`
- **When** se aplica la migración
- **Then** la clave deja de existir
- **And** el umbral efectivo de `widgets` es `0`

#### Scenario: Un umbral subido a propósito se respeta
- **Given** `ROUTER_THRESHOLD_WIDGETS` con un valor distinto de `0.20`
- **When** se aplica la migración
- **Then** el valor del usuario se conserva
- **And** el umbral efectivo de `widgets` es el suyo

#### Scenario: Un umbral ilegible cae al default
- **Given** un `ROUTER_THRESHOLD_<ID>` con un valor no numérico o fuera de `[0, 1]`
- **When** se resuelve el umbral efectivo
- **Then** se usa el umbral por defecto de la skill
- **And** se registra un warning

#### Scenario: Encender surte efecto sin reiniciar
- **Given** el enrutado apagado
- **When** se pone `ROUTER_ENABLED` a `true` desde los ajustes
- **Then** el turno siguiente ya enruta

### Requirement: El fragmento de la skill widgets SHALL ser un recordatorio breve que remita a render_widget

El fragmento `settings.SKILL_WIDGETS_PROMPT` SHALL NOT residir de forma permanente en
`settings.system_prompt`. La migración `20261008000001` SHALL retirar ese bloque del prompt base **solo si se
encuentra verbatim** (identificado por su encabezado) y SHALL depositarlo como contenido de
`SKILL_WIDGETS_PROMPT`; si el bloque no se encuentra verbatim, SHALL NOT retirar nada y el fragmento SHALL
sembrarse igualmente. La regla anti-duplicado del ensamblador de fragmentos SHALL seguir vigente como red de
seguridad. Como la skill `widgets` tiene umbral cero, la guía SHALL viajar en el mensaje de sistema de
**todos** los turnos, también con el enrutador caído o apagado. Con la skill `widgets` **deshabilitada** SHALL
NOT enviarse ni el esquema de la herramienta ni la guía.

El fragmento SHALL **declarar la forma** que el servidor sabe leer, no decidir la presentación. La proyección
determinista al cerrar el turno (ver `orchestrator/agent`) lee la tabla con cabecera de opciones que el modelo
escribe, de modo que el fragmento SHALL pedir esa forma como **instrucción de formato**: alternativas como
tabla Markdown con cabecera de opciones, una fila por alternativa, con la etiqueta y el texto de la
alternativa en la misma fila. El fragmento SHALL NOT enumerar los tres nombres de `widget_name` con ejemplos
de `data`, ni exponer un catálogo de widgets; SHALL limitarse a esa forma, a pedir que no se anuncie el widget,
no se pida permiso y no se repita la lista en prosa, y a señalar que `render_widget` sigue disponible para lo
que el servidor no puede inferir del texto (varios datos en un mismo formulario, un plan con pasos, un mapa
cuyos datos no aparezcan escritos).

La migración `20261102000001_widget_projection.sql` SHALL sustituir la guía extensa sembrada por
`20261101000001_widgets_always_on.sql` por el recordatorio **solo si su valor sigue siendo byte a byte** el
texto sembrado; si el usuario lo ha editado, SHALL NOT pisarlo.

La migración `20261104000001_widget_options_contract.sql` SHALL sustituir el fragmento por el texto que declara
la forma **solo si su valor es byte a byte uno de los dos textos conocidos**: el recordatorio sembrado por
`20261102000001` o la guía imperativa de 3.812 caracteres que quedó en producción al editarse aquel valor.
Cualquier otro valor es contenido del usuario y SHALL NOT pisarse.

El texto exacto del fragmento SHALL ser:

> # Cómo presentar alternativas
>
> Cuando tu respuesta ofrezca alternativas para elegir —dos o más opciones, un «¿cuál prefieres?»— escríbelas
> como una **tabla Markdown con cabecera de opciones**, una fila por alternativa, con la etiqueta y el texto
> en la misma fila:
>
> | Opción | Acción |
> |---|---|
> | **A** | **Marcar los 38 como leídos** — bandeja a cero |
> | **B** | **Marcar solo los 4 de IA** que ya hemos visto |
>
> El servidor convierte esa tabla en un formulario de elección: no lo anuncies, no pidas permiso y no repitas
> la lista en prosa. Escribe solo la frase que la acompaña.
>
> Las listas de cosas que marcar (tareas, avisos, correos) y un punto en el mapa también los proyecta el
> servidor a partir de tu texto.
>
> Para lo que el servidor no puede inferir —pedir **varios datos** en un mismo formulario, un **plan con
> pasos** o un mapa cuyos datos no aparezcan escritos— usa la herramienta `render_widget`, que sigue
> disponible.

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

#### Scenario: La guía viaja en todos los turnos
- **Given** la skill `widgets` habilitada con umbral `0` y un turno cualquiera
- **When** se compone el mensaje de sistema
- **Then** contiene la guía de widgets
- **And** la petición ofrece `render_widget`

#### Scenario: Con la skill inactiva no viaja la guía
- **Given** un turno en el que la skill `widgets` está deshabilitada (`ROUTER_SKILL_WIDGETS_ENABLED` a `false`)
- **When** se compone el mensaje de sistema
- **Then** no contiene la guía de widgets
- **And** la herramienta `render_widget` no figura entre las ofrecidas

#### Scenario: Un fragmento editado por el usuario no se pisa
- **Given** un `SKILL_WIDGETS_PROMPT` con texto del usuario
- **When** se aplica la migración `20261102000001_widget_projection.sql`
- **Then** el valor del usuario se conserva

#### Scenario: La migración es idempotente
- **Given** una base de datos ya migrada
- **When** se vuelve a aplicar la migración
- **Then** el prompt base y el fragmento quedan igual

#### Scenario: La guía se recorta al recordatorio y no prescribe la regla de las opciones
- **Given** el fragmento `SKILL_WIDGETS_PROMPT` con la guía extensa sembrada por `20261101000001`
- **When** se aplica la migración `20261102000001_widget_projection.sql`
- **Then** el fragmento pasa a ser un recordatorio breve
- **And** NO enumera los tres widgets con ejemplos de `data`
- **And** NO enuncia la regla de las opciones como una instrucción que el modelo deba aplicar
- **And** remite a que `render_widget` sigue disponible como vía explícita

#### Scenario: El fragmento declara la forma que el servidor sabe leer
**Given** el fragmento `SKILL_WIDGETS_PROMPT` con el recordatorio sembrado por `20261102000001`
**When** se aplica la migración `20261104000001_widget_options_contract.sql`
**Then** el fragmento pide escribir las alternativas como tabla Markdown con cabecera de opciones
**And** NO enumera los tres nombres de `widget_name` ni ejemplos de `data`
**And** remite a `render_widget` para lo que el servidor no puede inferir

#### Scenario: Un fragmento editado no se pisa
**Given** un fragmento `SKILL_WIDGETS_PROMPT` con un texto distinto de los dos textos conocidos
**When** se aplica la migración `20261104000001_widget_options_contract.sql`
**Then** el fragmento conserva el texto del usuario
