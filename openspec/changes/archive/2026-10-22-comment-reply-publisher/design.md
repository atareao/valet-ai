# Design

## Context

El otro lado del cable ya está hecho y verificado (repo `/data/php/atareao.es`, change
`valet-comment-replies`): el aviso de un comentario aprobado llega a Valet con `metadata.comment_id`
y `metadata.post_id`, y el asistente puede leerlo con la herramienta `notifications`. Este
repositorio, en cambio, no tiene **ninguna** vía de escritura hacia WordPress (comprobado por CLI:
cero apariciones de `wp/v2/comments` o `wp-json` en `src/`). Ver el proposal para la motivación.

Tres restricciones del repositorio condicionan el diseño:

1. **`IngestRequest` descarta en silencio los campos de primer nivel que no declara** (`message`,
   `origin`, `author`, `level`, `metadata`). Los identificadores **solo** pueden venir del
   `metadata`, y allí llegan como **cadena JSON** (`notifications.metadata` es `TEXT`).
2. **No hay ninguna tabla que sirva de registro de publicaciones.** Inventario de tablas migradas:
   `contacts`, `events`, `habit_logs`, `habits`, `ingest_keys`, `journal_entries`, `llm_requests`,
   `memories`, `memory`, `message_embeddings`, `memory_embeddings`, `messages`, `notes`,
   `notifications`, `persistent_memory`, `profiles`, `push_subscriptions`, `reminders`, `settings`,
   `shopping_list`, `tasks`, `timeline_events`, `tools`, `vec_memory`. Ninguna encaja: `settings` es
   clave-valor de valor único, `tools` es el catálogo del registry y `notifications` es la bandeja.
3. **La vía de escritura de WordPress tiene cota** (5 respuestas por 3600 s, `429` con el código
   `atareao_valet_reply_rate_limited`, sin crear el comentario) y **no ejecuta su anti-abuse**. Un
   bucle de respuestas es, con diferencia, el fallo más caro de esta funcionalidad.

## Goals / Non-Goals

- **Goal:** publicar **una** respuesta a un comentario, con los identificadores que ya llegaron, sin
  interacción manual y sin responder dos veces.
- **Goal:** que cualquier fallo de la vía —credencial, cota, petición, red— se traduzca en un
  resultado **tipado** que el modelo pueda explicar, sin `panic!` y sin tumbar el proceso.
- **Non-goal (de diseño):** no se verifica contra WordPress que el `parent` pertenezca a la entrada
  (`GET /wp-json/wp/v2/comments/<id>`): costaría una petición de lectura por respuesta y los dos
  identificadores salen del mismo `metadata`, que es quien los conoce de primera mano.
- **Non-goal (de diseño):** no se construye un mecanismo genérico de «operaciones externas
  idempotentes»; el registro es específico de esta vía.
- **Non-goal (de diseño):** no se enruta el contenido por plantilla ni se valida su estilo: el texto
  lo redacta el modelo.

## Decisions

### Tabla `comment_replies` propia, y no `settings` ni memoria del proceso

No hay tabla donde encaje (ver Context). Alternativas descartadas:

- **Un `HashSet` en memoria** (`AppState` o un `OnceLock`): barato, pero un reinicio del proceso
  olvida lo respondido y la siguiente notificación —que sigue sin leer— reabre el bucle. El fallo más
  caro no puede depender de que nadie reinicie nada.
- **Reutilizar `settings`**: es clave-valor de valor único, y lo que hace falta es una **colección**
  que crece con cada respuesta.
- **Una columna en `notifications`**: la notificación es la bandeja de entrada, no el registro de lo
  publicado, y una notificación se puede purgar sin que la respuesta deje de existir.

Consecuencia: una migración **aditiva e idempotente** (`CREATE TABLE IF NOT EXISTS`, sin reconstruir
ninguna tabla) y un repositorio `CommentRepliesRepo` con la semántica de los demás.

### La reserva es de dos fases, y solo se suelta ante un «no se creó nada» demostrable

El `INSERT ... ON CONFLICT(comment_id) DO NOTHING` es **atómico** y devuelve `rows_affected()`: si es
`0`, otra llamada (u otra anterior) ya reservó ese comentario y **no se sale a la red**. Si es `1`,
la reserva se materializa y solo entonces se hace la petición.

Al terminar la petición:

| Desenlace | ¿Se creó el comentario? | Estado final de la fila | Motivo |
|---|---|---|---|
| `201` | Sí | `published` (+ `wp_comment_id`) | Publicado. |
| `400` | No (petición rechazada) | reserva **liberada** (`DELETE`) | Se puede corregir y reintentar. |
| `401` / `403` | No (rechazada antes de crear) | reserva **liberada** | Es credencial/rol, no el comentario. |
| `429` | **No** — WordPress lo declara explícitamente | reserva **liberada** | La cota se recupera con el tiempo; el comentario sigue sin responder. |
| timeout / error de red / `5xx` | **Indeterminado** | `uncertain`: la reserva **se conserva** | El servidor pudo haber creado el comentario y perderse la respuesta. |
| proceso caído entre reserva y desenlace | Indeterminado | `reserved`: la reserva **se conserva** | Fail-closed: mejor no responder que responder dos veces. |

Las tres situaciones bloquean una segunda publicación (`reserved`, `published` y `uncertain` se
tratan igual: **no se publica**). El coste asumido es explícito: una reserva `uncertain` o `reserved`
necesita una intervención manual del operador para volver a intentarlo
(`DELETE FROM comment_replies WHERE comment_id = '<id>'`), y esa orden se documenta en la cabecera de
la migración. Se prefiere eso a un doble comentario en el sitio.

La herramienta **no reintenta** en ningún caso: un solo `POST` por invocación.

### Los identificadores se resuelven del `metadata`, y el parseo es una función pura

`metadata` llega como **cadena JSON** (o `NULL`). El parseo se encapsula en una función pura
(`reply_ids_from_metadata(Option<&str>) -> Result<ReplyIds, MetadataError>`) para poder probarla
aislada y sin base de datos:

- ausente o `NULL` → error accionable («la notificación no trae `comment_id`/`post_id`»);
- no es JSON válido, o es JSON pero **no es un objeto** → error accionable;
- falta `comment_id` o `post_id` → error que **nombra el que falta**;
- el valor se acepta como **número entero positivo** o como **cadena de dígitos** (se recorta y se
  normaliza a su forma decimal); un flotante, un negativo, un cero, `null`, un booleano, un array, un
  objeto o una cadena vacía → `InvalidArguments`.

**Nunca se inventa un identificador** (no hay valor por defecto, ni `0`, ni `1`). Los dos
identificadores de una misma respuesta salen **siempre** del mismo `metadata`, que es lo que hace
innecesaria la comprobación de coherencia `parent`/`post` que WordPress no hace.

Sobre la forma de la entrada: `notification_id` y los identificadores explícitos son **mutuamente
excluyentes** y declarar los dos devuelve `InvalidArguments`; el modelo no tiene que adivinar cuál
manda. Sin `notification_id`, los identificadores explícitos son la vía mínima que pide el contrato
(el modelo ya los leyó del `metadata` con `notifications.list_unread`).

### Credenciales por entorno, no por `settings`

Las Application Passwords **caducan y se revocan por su cuenta**, y son una credencial de servicio,
no un ajuste del usuario. Alternativa descartada: sembrarlas en `settings` y editarlas desde la
pestaña «API Keys» —eso obligaría a añadirlas a `SENSITIVE_KEYS` para que `GET /api/settings` no las
devuelva, y abriría una pestaña de escritura para algo que se rota desde el panel de WordPress—. Se
sigue el precedente de apimail y Miniflux leyendo el entorno **en el servicio**, sin tocar
`src/config.rs`.

La resolución de configuración es una función con base inyectable y **un `Debug` que redacta la
contraseña** (`{ base_url, service_user, app_password: "***" }`), igual que `ApimailConfig` y
`MinifluxConfig`. La cabecera `Authorization` se construye en el momento de la petición y **no** se
guarda en la estructura ni se registra. Sin usuario o sin contraseña **no se envía ninguna petición**.

### Timeout propio de 15 s y sin reintentos

apimail y Miniflux usan 30 s porque una lectura puede tardar. Aquí la llamada ocurre **dentro de un
turno de chat**, y el usuario está esperando: 15 s son de sobra para un `INSERT` de un comentario en
el mismo tipo de alojamiento, y acotan lo que un servidor colgado puede bloquear el turno. El cliente
`reqwest` **no** reintenta por su cuenta: un `POST` con `reqwest` se envía una sola vez.

### El mapa de errores distingue `401` de `403` y nombra la cota en el `429`

`401` y `403` significan cosas distintas y el mensaje debe decirlo: `401` es «la credencial no vale o
ha caducado» (regenerar la Application Password) y `403` es «la credencial vale pero no puede
publicar» (revisar el rol del usuario de servicio: hace falta `moderate_comments`). El `429` no es un
fallo genérico: el mensaje dice que la **cota de escritura** está agotada, que la respuesta
**no** se publicó y que **no** se debe reintentar en bucle.

Cualquier otro estado se traduce a `Unexpected { status, code }`, con el código de error de WordPress
(`code`) y **sin** su `message` libre: un texto de terceros en un mensaje de error es superficie de
inyección en el prompt del modelo. Un `201` con cuerpo ilegible **no** es un fallo: el comentario está
creado, así que se registra como `published` con `wp_comment_id` nulo y se informa.

### Skill propia `blog`, y no dentro de `pendientes`

`notifications` vive en `pendientes` porque **lee** avisos. Publicar en el sitio es otra cosa: si
`blog_comment_reply` entrase en `pendientes`, cualquier turno sobre recordatorios o tareas tendría a
mano publicar en el blog. El precedente del repositorio es unánime —`email_send` (escribir) vive en
`email`, no suelto; `rss_mark_read` vive en `rss`— y el catálogo admite una skill más sin ninguna
cirugía estructural: es una variante del enum, una entrada en `CATALOG` y su migración de prompt.

El router pregunta por cada skill **por separado**, así que un turno «responde a este comentario»
selecciona `blog` (y, si el mensaje también habla del aviso entrante, `pendientes`): las dos
capacidades conviven en el mismo turno sin que ninguna ensanche a la otra. Si el router falla, falla
**abierto** y expone todas las skills habilitadas, incluida `blog`.

### `ExplicitApproval` (decisión aprobada)

Publicar es público e irreversible y no hay «deshacer» en el flujo del agente; el precedente exacto
es `email_send`. **Aprobado el 2026-10-22**: el permiso de `blog_comment_reply` es
`Permission::ExplicitApproval`, de modo que la herramienta pide confirmación al usuario antes de
publicar (la variante `NoConfirm`, como `rss_mark_read`, queda descartada). En la misma aprobación se
fijaron los nombres: skill nueva `blog` (la undécima del catálogo) y variables de entorno
`BLOG_BASE_URL` / `BLOG_SERVICE_USER` / `BLOG_APP_PASSWORD`.

### Techo de contenido

`content` se recorta, no puede quedar vacío y **no** puede superar los **4000 caracteres**
(`InvalidArguments` sin salir a la red). Es una respuesta a un comentario, no un artículo: el techo
protege al sitio público de una generación desbocada, y quien escribe el texto es el modelo.

## Risks / Trade-offs

- **Una reserva `uncertain` bloquea una respuesta legítima hasta que alguien la borre a mano** →
  mitigación: la migración documenta el `DELETE` exacto, el mensaje de la herramienta explica que
  hay un registro previo y que la intervención es manual, y el caso queda cubierto por un test.
- **La cota de WordPress (5/3600 s) es global para esta vía** → mitigación: nunca se reintenta, el
  `429` se traduce a un mensaje que lo explica, y la reserva se libera para poder responder más tarde.
- **Las Application Passwords no están disponibles en el desarrollo del sitio** → mitigación: es un
  requisito de aprovisionamiento declarado (abajo) y el bloqueo se consigna; los tests no dependen de
  un WordPress real (servidor de prueba `wiremock`), así que el código se puede desarrollar y probar
  entero sin el aprovisionamiento.
- **Un `metadata` de terceros acaba en el prompt y en la lógica de publicación** → mitigación: se
  aceptan **solo** dos campos con forma de entero positivo; el resto del objeto se ignora y nunca se
  interpreta como instrucción.
- **El modelo podría redactar y publicar algo incorrecto** → mitigación: `ExplicitApproval` y el
  hecho de que la herramienta **no** genera texto: solo publica lo que el usuario aprueba.

## Migration Plan

1. Migraciones aditivas e idempotentes: `20261022000001_comment_replies.sql` (tabla + índice) y
   `20261022000002_blog_skill.sql` (habilitación de la skill y fragmento). Ninguna reconstruye una
   tabla; `rollback` = borrar el directorio del change, no hay que deshacer datos.
2. Aprovisionamiento manual en WordPress (operador), con los comandos exactos para cerrarlo:

```bash
# 1) Usuario de servicio con rol mínimo (read + moderate_comments). Se hace por WP-CLI
#    contra el contenedor del sitio, o desde el panel.
just wp -- user create valet-bot valet-bot@example.invalid --role=subscriber --user_pass="$(openssl rand -base64 24)"
just wp -- user add-role valet-bot moderate_comments

# 2) Application Password para ese usuario. Requiere SSL o el entorno `local`;
#    sin `wp-cli` disponible, se genera desde Usuarios → Perfil → Application Passwords.
just wp -- user application-password create valet-bot "valet" --porcelain

# 3) Las tres variables en el entorno de Valet (no en el repositorio).
BLOG_BASE_URL=https://atareao.es
BLOG_SERVICE_USER=valet-bot
BLOG_APP_PASSWORD=xxxx xxxx xxxx xxxx xxxx xxxx
```

3. Prueba de punta a punta (bloqueada hasta que el paso 2 esté hecho): aprobar un comentario en el
   sitio, comprobar que llega el aviso con `metadata.comment_id`/`metadata.post_id`, pedir al
   asistente que responda y verificar que el comentario aparece **aprobado** y **una sola vez**.

## Open Questions

Ninguna. Los tres puntos que quedaban abiertos se resolvieron en la aprobación del 2026-10-22:

- **Permiso:** `Permission::ExplicitApproval` (precedente `email_send`); `NoConfirm` descartado.
- **Nombres:** herramienta `blog_comment_reply`, skill `blog` (la undécima del catálogo) y variables
  de entorno `BLOG_BASE_URL` / `BLOG_SERVICE_USER` / `BLOG_APP_PASSWORD`.
