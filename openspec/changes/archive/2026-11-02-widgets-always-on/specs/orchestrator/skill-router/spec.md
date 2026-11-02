## MODIFIED Requirements

### Requirement: El catálogo de skills SHALL ser cerrado y cubrir todas las herramientas con sus prerrequisitos

El catálogo SHALL declarar, en código, un conjunto **cerrado de doce skills de dominio amplio**: `agenda`, `pendientes`, `recuerdos`, `entorno`, `web`, `widgets`, `running`, `timeline`, `email`, `rss`, `blog` y `zennotes`. Cada skill SHALL declarar un id estable, las instrucciones y las `criteria` por defecto con las que se pregunta por ella, su umbral por defecto, el conjunto de herramientas que cubre —**incluyendo los prerrequisitos**— y la clave y el encabezado de su fragmento de prompt. Las herramientas de un mismo dominio SHALL NOT repartirse entre skills distintas: `pendientes` cubre `tasks`, `reminders` y `notifications`; `recuerdos` cubre `notes`, `unified_search` y `journal`; `entorno` cubre `weather`, `geocode`, `reverse_geocode` y `search_places`; `running` cubre `strava_recent_activities`, `strava_activity_detail`, `strava_activity_streams` y `strava_athlete_stats`; `timeline` cubre `timeline_get_events`, `timeline_add_event` y `timeline_delete_event`; `email` cubre `email_list_unread`, `email_get_body`, `email_mark_read` y `email_send`; `rss` cubre `rss_list_unread`, `rss_get_entry`, `rss_search_entries` y `rss_mark_read`; `blog` cubre `blog_comment_reply` y `blog_comment_delete`; `zennotes` cubre la herramienta `zennotes`, y **solo** ella. Las `criteria` de `recuerdos` SHALL incluir el habla de la bitácora (`bitácora`, `diario`, `cómo fue mi día`); y las de `zennotes` SHALL reconocer el **vault de ZenNotes** —sus notas, sus artículos y sus apuntes («mis notas de ZenNotes», «mi vault», «apunta en mis notas del vault»)—, de modo que el router seleccione `zennotes` para leer o escribir en el vault y no lo confunda con la store interna de `recuerdos`. El conjunto **core** no enrutable SHALL ser `get_current_time` y `get_current_location`. La herramienta `render_widget` SHALL pertenecer a la skill `widgets` y SHALL NOT estar en el core. Una herramienta PUEDE pertenecer a más de una skill. Toda herramienta registrada SHALL pertenecer al core o a al menos una skill.

#### Scenario: Toda herramienta registrada está cubierta
- **Given** el registry de producción con sus treinta y tres herramientas
- **When** se contrasta con el catálogo de skills y el conjunto core
- **Then** cada herramienta pertenece al core o a alguna skill
- **And** el test de integridad falla si se registra una herramienta y se olvida en el catálogo

#### Scenario: Las herramientas de un mismo dominio no se reparten en skills distintas
- **Given** el catálogo de skills
- **When** se inspeccionan las herramientas de `pendientes`, `recuerdos`, `entorno`, `running`, `timeline`, `email`, `rss` y `blog`
- **Then** `tasks`, `reminders` y `notifications` están todas en `pendientes`
- **And** `notes`, `unified_search` y `journal` están todas en `recuerdos`
- **And** `weather`, `geocode`, `reverse_geocode` y `search_places` están todas en `entorno`
- **And** las cuatro herramientas `strava_*` están todas en `running`
- **And** las tres herramientas `timeline_*` están todas en `timeline`
- **And** las cuatro herramientas `email_*` están todas en `email`
- **And** las cuatro herramientas `rss_*` están todas en `rss`
- **And** `blog_comment_reply` y `blog_comment_delete` están en `blog`
- **And** ni `blog_comment_reply` ni `blog_comment_delete` están en `pendientes`, donde solo se leen los avisos

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
- **And** el catálogo sigue declarando once skills: la herramienta nueva no añade una skill

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

### Requirement: La guía de widgets SHALL residir en el fragmento de su skill

La guía de uso de `render_widget` SHALL residir en `settings.SKILL_WIDGETS_PROMPT` y SHALL NOT
residir de forma permanente en `settings.system_prompt`. La migración `20261008000001` SHALL retirar
ese bloque del prompt base **solo si se encuentra verbatim** (identificado por su encabezado) y SHALL
depositarlo como contenido de `SKILL_WIDGETS_PROMPT`; si el bloque no se encuentra verbatim, SHALL NOT
retirar nada y el fragmento SHALL sembrarse igualmente. La regla anti-duplicado del ensamblador de
fragmentos SHALL seguir vigente como red de seguridad para un prompt base que ya contuviera la guía.
Como la skill `widgets` tiene umbral cero (ver «La selección SHALL gobernar también el turno sin
herramientas»), la guía SHALL viajar en el mensaje de sistema de **todos** los turnos, también con el
enrutador caído o apagado. Con la skill `widgets` **deshabilitada** SHALL NOT enviarse ni el esquema de
la herramienta ni la guía.

La guía SHALL ser **prescriptiva y autosuficiente**, porque es la única instrucción que el modelo
recibe sobre la herramienta. SHALL cumplir, como mínimo:

1. Declarar que un widget **solo** aparece si se ejecuta `render_widget` en ese mismo turno, y
   prohibir afirmar lo contrario.
2. Enunciar la **regla de las opciones**: si el modelo va a escribir dos o más opciones etiquetadas
   (A/B/C, 1/2/3) o una lista de alternativas para que el usuario elija, NO SHALL escribirlas en
   prosa y SHALL renderizar un widget con ellas.
3. Nombrar los **tres** nombres admitidos de `widget_name` —`QuickForm`, `Checklist` y
   `LocationWidget`— con un ejemplo mínimo de `data` para cada uno, incluidas las claves que cada
   widget lee.
4. Retirar toda autorización genérica de responder en prosa que pueda amparar una lista de opciones.
5. Acotar los casos **sin** widget a dos: una explicación conceptual sin opciones ni listas que
   marcar, y un dato único directo.
6. Conservar la regla de procesamiento de la respuesta del widget: al recibir los datos elegidos, se
   procesan de inmediato y se confirma el resultado sin volver a renderizar, salvo petición explícita
   de modificación.

La migración `20261101000001` SHALL sustituir el fragmento sembrado por la guía nueva **solo si su
valor sigue siendo byte a byte el texto sembrado**; si el usuario lo ha editado, SHALL NOT pisarlo.

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

#### Scenario: La guía nombra los tres widgets y la regla de las opciones
- **Given** el fragmento `SKILL_WIDGETS_PROMPT` por defecto
- **When** se inspecciona su contenido
- **Then** menciona `QuickForm`, `Checklist` y `LocationWidget`
- **And** menciona `render_widget`
- **And** enuncia la regla de no escribir en prosa dos o más opciones etiquetadas

#### Scenario: La guía no ampara la prosa con opciones
- **Given** el fragmento `SKILL_WIDGETS_PROMPT` por defecto
- **When** se inspecciona su contenido
- **Then** no autoriza responder en prosa sin condicionar esa autorización a que no haya opciones ni listas que marcar
- **And** declara la regla de las opciones prioritaria sobre los casos sin widget

#### Scenario: Con la skill inactiva no viaja la guía
- **Given** un turno en el que la skill `widgets` está deshabilitada (`ROUTER_SKILL_WIDGETS_ENABLED` a `false`)
- **When** se compone el mensaje de sistema
- **Then** no contiene la guía de widgets
- **And** la herramienta `render_widget` no figura entre las ofrecidas

#### Scenario: Un fragmento editado por el usuario no se pisa
- **Given** un `SKILL_WIDGETS_PROMPT` con texto del usuario
- **When** se aplica la migración
- **Then** el valor del usuario se conserva

#### Scenario: La migración es idempotente
- **Given** una base de datos ya migrada
- **When** se vuelve a aplicar la migración
- **Then** el umbral y el fragmento quedan igual

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
