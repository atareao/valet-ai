# Change: Publicador de respuestas a comentarios del blog (`blog_comment_reply`)

## Why

En el sitio WordPress ya está cerrado el change `valet-comment-replies`: cuando se aprueba un
comentario, el aviso llega a Valet con los identificadores dentro de `metadata` (`comment_id` y
`post_id`) y el asistente puede leerlo con la herramienta `notifications` (`list_unread` devuelve la
fila cruda, con el `metadata` serializado). Falta **justo la otra mitad**: quien publica la respuesta
es Valet, y hoy **no existe ninguna herramienta en este repositorio que escriba en WordPress**. Se ha
comprobado por CLI:

```
grep -rn "wp/v2/comments\|wp-json" src/   → sin coincidencias
```

La consecuencia es un flujo a medias y caro de operar a mano: el aviso llega, el usuario lo lee en
Valet, y para responder tiene que abrir el navegador, entrar al panel de WordPress y escribir la
respuesta él mismo. El asistente puede redactarla, pero no tiene dónde ponerla. Esta pieza es la que
cierra el circuito, y por eso va ahora: sin ella, el contrato que WordPress ya garantiza no lo
consume nadie.

## What Changes

- **Nueva herramienta `blog_comment_reply`** (`src/tools/blog_comments.rs`), expuesta al modelo, que
  publica **una** respuesta a un comentario del blog contra la **API REST del núcleo de WordPress**
  (`POST /wp-json/wp/v2/comments`) autenticada con **Application Password** por
  `Authorization: Basic base64(usuario:application_password)`. Campos que viajan: `post` (id de la
  entrada), `parent` (id del comentario al que se responde), `content` (la respuesta) y
  `status: approved`. **SHALL NOT enviarse `author_name` ni `author_email`**: con ellos el comentario
  nace **pendiente** en vez de aprobado en un solo paso, que es exactamente lo que este change quiere
  evitar.
- **Nueva capa de servicio `Wordpress`** (`src/services/wordpress.rs`), con la base **inyectable**
  (para poder apuntar los tests a un servidor de prueba con `wiremock`, igual que `apimail` y
  `miniflux`), `User-Agent` propio, timeout y un enum cerrado `WordpressError` con mensajes
  accionables. **Sin reintentos automáticos**: la vía de escritura de WordPress impone una cota
  (5 respuestas por 3600 s) y reintentar en bucle la empeora.
- **Credenciales por variables de entorno** — `BLOG_BASE_URL`, `BLOG_SERVICE_USER` y
  `BLOG_APP_PASSWORD` — leídas en cada llamada y documentadas en `.env.example`, `.env.j2` y
  `.env.prod.sample`. La contraseña de aplicación **SHALL NOT** aparecer en un log, en la telemetría,
  en el resultado de la herramienta ni en un mensaje de error; el `Debug` de la configuración la
  redacta. Sin configuración **no se envía ninguna petición** y el error dice dónde definirla.
- **Origen de los identificadores: el `metadata` de la notificación.** La herramienta acepta o bien
  `notification_id` (y resuelve `comment_id`/`post_id` leyendo y parseando el `metadata` de esa
  notificación) o bien los dos identificadores explícitos que el modelo ya leyó del `metadata`. Si no
  se pueden resolver —`metadata` ausente, ilegible, que no sea objeto, sin los campos, o con un valor
  que no sea entero positivo— la herramienta devuelve `InvalidArguments` **sin salir a la red** y
  **nunca inventa** un identificador.
- **Idempotencia con registro propio.** **No existe hoy ninguna tabla en la que quepa** recordar qué
  comentarios ya se respondieron (se ha revisado el inventario de tablas migradas: `notifications`,
  `messages`, `tools`, `settings`… ninguna es un registro de publicaciones). Se añade una migración
  **aditiva e idempotente** con la tabla `comment_replies` —`comment_id` como clave primaria, con el
  `post_id`, el estado de la publicación y el id que devuelve WordPress— y un repositorio
  `CommentRepliesRepo`. La reserva se toma con un `INSERT ... ON CONFLICT DO NOTHING` (una sola
  llamada concurrente gana), se suelta **solo** ante respuestas que garantizan que no se creó nada
  (`400`, `401`, `403`, `429`) y se **conserva** ante un desenlace ambiguo (timeout, fallo de red,
  `5xx`), porque publicar dos veces es el fallo más caro de esta funcionalidad.
- **Skill nueva `blog`** (la undécima del catálogo cerrado) que cubre **exactamente** la herramienta
  nueva, con su pregunta, sus dos criterios, su umbral, su clave de fragmento `SKILL_BLOG_PROMPT` y su
  encabezado `# SKILL ACTIVA: BLOG`. Es la misma decisión que se tomó con `email` y `rss`: una
  capacidad de escritura **no** ensancha una skill ajena. `blog_comment_reply` **no** entra en
  `pendientes` (donde vive `notifications`, que solo lee), de modo que un turno sobre recordatorios o
  tareas no gana capacidad de publicar en el sitio.
- **Permiso `Permission::ExplicitApproval`.** Publicar una respuesta en el blog es **público e
  irreversible**, igual que enviar un correo, y el precedente del repositorio es el mismo:
  `email_send` exige aprobación explícita. La decisión del usuario de publicar **sin moderación** es
  sobre el `status: approved` de WordPress, no sobre saltarse la confirmación del agente. Queda como
  punto a confirmar en la aprobación de este change.
- **Migración de la skill** (aditiva e idempotente, sin reconstruir ninguna tabla) que siembra
  `ROUTER_SKILL_BLOG_ENABLED='true'` y el fragmento `SKILL_BLOG_PROMPT`, cuyo primera línea es el
  encabezado de la skill, que es como el catálogo detecta el fragmento y evita duplicarlo. La guía
  **no** se mete en `SKILL_PENDIENTES_PROMPT`: la tool pertenece a otra skill.
- **Trazabilidad sin contenido**: en el éxito se registra `comment_id`, `post_id`, `notification_id`
  (si la hubo), el `wp_comment_id`, el estado y la latencia; en la negativa por duplicado y en el
  fallo, el motivo tipado. **Nunca** el contenido íntegro de la respuesta ni la credencial.

## Capabilities

**New Capabilities**

- `blog-comments` — publicación de respuestas a comentarios del blog: la herramienta
  `blog_comment_reply`, su contrato con la API REST del núcleo de WordPress, la resolución de los
  identificadores desde el `metadata` de la notificación, las credenciales de entorno, el mapa de
  errores de la vía y el registro de idempotencia `comment_replies`.

**Modified Capabilities**

- `tools/registry` — el registro de producción pasa de treinta a **treinta y una** herramientas e
  incluye `blog_comment_reply`, con su permiso `ExplicitApproval` y su definición en español
  documentando los campos obligatorios.
- `orchestrator/skill-router` — el catálogo cerrado pasa de diez a **once** skills (`blog` cubre
  `blog_comment_reply`), y la unión del core más las skills sigue sin dejar herramientas huérfanas.

## Impact

### Specs

- **Nueva:** `blog-comments`.
- **Modificadas:** `tools/registry`, `orchestrator/skill-router`.

### Código

- `src/services/wordpress.rs` (nuevo) y `src/services/mod.rs`; `src/tools/blog_comments.rs` (nuevo),
  `src/tools/mod.rs` y `build_tool_registry` en `src/lib.rs`; `src/db/repos/comment_replies.rs`
  (nuevo) y `src/db/repos/mod.rs`; `src/orchestrator/skills.rs` (variante `Skill::Blog`, catálogo y
  tests de integridad); la lista `ALL_TOOLS` de los tests de `src/orchestrator/skill_router.rs`; el
  recuento de skills de `tests/api/tools.rs`. Las credenciales se leen **directamente en el
  servicio**, como ya hacen apimail y Miniflux, **sin tocar `src/config.rs`**.
- **Frontend: sin cambios.** La pestaña de skills del `SettingsDialog` se alimenta del catálogo que
  devuelve la API (`GET /api/skills`), así que la skill nueva aparece sola; la URL del blog y la
  Application Password son variables de entorno y **no** son ajustes editables.

### Datos

- `migrations/20261022000001_comment_replies.sql` — tabla `comment_replies` con su índice.
  **Aditiva e idempotente. No reconstruye ninguna tabla.**
- `migrations/20261022000002_blog_skill.sql` — siembra `ROUTER_SKILL_BLOG_ENABLED` y
  `SKILL_BLOG_PROMPT`, sin pisar un valor existente. **Aditiva e idempotente.**
- `[[test]]` nuevo `comment-reply-publisher-migration` para la migración.

### Aprovisionamiento manual en WordPress (dependencia externa)

La vía de escritura requiere, **en el sitio**, un **usuario de servicio** con rol mínimo (`read` +
`moderate_comments`) y una **Application Password** para él. Nada de eso se puede hacer desde este
repositorio: es aprovisionamiento manual del operador, y hasta que exista, la herramienta devuelve un
error accionable en vez de publicar. Además, las Application Passwords **no están disponibles en el
entorno de desarrollo del sitio** (WordPress las ofrece por defecto pero las restringe sin SSL, salvo
con el entorno `local`), de modo que la prueba **de punta a punta** queda bloqueada hasta que el
operador cree el usuario, le asigne el rol, genere la Application Password y devuelva las tres
variables. Los comandos exactos para cerrar ese bloqueo se consignan en `design.md`.

### Fuera de alcance (no-objetivos)

- **No** se cambia el contrato de ingesta: `IngestRequest` sigue con sus **cinco** campos y **sin**
  `deny_unknown_fields`. Los identificadores siguen llegando dentro de `metadata`.
- **No** se toca el repositorio de WordPress (`/data/php/atareao.es`).
- **No** se añade moderación: la publicación es directa (`status: approved`) por decisión del usuario.
- **No** se implementa la generación del texto: el contenido lo redacta el modelo; esta herramienta
  **solo publica**.
- **No** se valida que el `parent` pertenezca a la misma entrada: WordPress tampoco lo valida en esta
  vía, y una comprobación propia costaría una petición de lectura más por cada respuesta. La
  coherencia la garantiza el origen de los identificadores (los dos salen del mismo `metadata` de la
  misma notificación). Limitación conocida y consignada.
- **No** se añade UI para configurar el blog ni para elegir la respuesta a publicar: el flujo es el
  del chat.
- **No** se reintenta automáticamente ante `429` ni ante un fallo de red.
