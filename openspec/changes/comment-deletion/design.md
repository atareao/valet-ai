# Design — Borrado de comentarios del blog (`blog_comment_delete`)

## Context

Ver `proposal.md` § *Why* para la motivación. Lo que condiciona el diseño:

- El cliente de la vía ya existe: `src/services/wordpress.rs` resuelve la configuración del entorno en
  cada llamada, construye `Authorization: Basic` en el momento de la petición, tiene `User-Agent` propio,
  timeout de 15 s, un `Debug` que redacta la contraseña y un enum cerrado `WordpressError` con mensajes
  accionables. **No tiene ninguna llamada de borrado**: la ruta de la constante `COMMENTS_PATH` es la de
  colección y solo se usa con `POST`.
- La herramienta vecina ya existe: `src/tools/blog_comments.rs` con `BlogCommentReplyTool`, la
  resolución de identificadores (`comment_ids_from_metadata`, `CommentIds`, `MetadataError`) y el
  constructor de test con configuración inyectada.
- El permiso `Permission::ExplicitApproval` y su confirmación en la interfaz ya están implementados y en
  uso (`email_send`, `tasks.delete_task`).
- **Hechos verificados del núcleo de WordPress** (no se re-investigan):

  | Desenlace de la ruta del ítem | `code` | Significado |
  |---|---|---|
  | `DELETE /wp-json/wp/v2/comments/<id>` con `force=false` → `200` | — | `wp_trash_comment()`: el comentario queda en la papelera |
  | `410` | `rest_already_trashed` | el comentario **ya** está en la papelera (distinto de «no existe») |
  | `404` | `rest_comment_invalid_id` | el comentario no existe |
  | `404` | `rest_no_route` | **la ruta no existe en el sitio** |
  | `403` | `rest_cannot_delete` | la credencial vale, pero no puede borrar |
  | `501` | `rest_trash_not_supported` | el sitio tiene `EMPTY_TRASH_DAYS = 0`: no hay papelera |

  El repositorio de WordPress **no** redefine `EMPTY_TRASH_DAYS`, así que su valor por defecto es 30
  días de papelera. El sitio **no notifica más de 20 comentarios por hora**. `rest_no_route` es lo que
  hoy recibe el publicador de respuestas (el plugin del sitio elimina la ruta de colección) y se repara
  en el change paralelo de **ese** repositorio; aquí solo se desambigua el `404` para que el mensaje sea
  accionable mientras tanto y después.

## Goals / Non-Goals

**Goals**

- Mandar a la papelera un comentario **notificado**, con la misma vía autenticada y la misma procedencia
  de identificadores que la respuesta.
- Que el usuario vea en el resultado **qué** se ha movido a la papelera (evidencia), sin exponer
  credenciales.
- Que un reintento del usuario o del modelo sea **inofensivo**: el `410` del sitio cierra la idempotencia
  sin estado propio.
- Que el modo de fallo de una instalación mal configurada —ruta ausente, papelera desactivada, rol
  insuficiente— se traduzca a un mensaje que diga qué revisar.

**Non-Goals (de diseño)**

- No se unifica en una herramienta multioperación ni se toca la firma de `blog_comment_reply`.
- No se modela el estado de la papelera en Valet (ni tabla, ni cola de borrados, ni restauración).
- No se refactoriza la resolución de identificadores ni el cliente: se **reutilizan**.

## Decisions

### D1 — Borrado por la ruta del **ítem**, y solo en modo papelera

`DELETE {base}/wp-json/wp/v2/comments/{comment_id}` con `force=false` como parámetro de consulta y sin
cuerpo. La ruta de colección (`/wp/v2/comments`) no admite borrado; además es justo la que el plugin del
sitio filtra, y apoyarse en ella ataría este cambio al change paralelo del repositorio de WordPress.

**Alternativa descartada:** enviar `force=true` siempre («borrar de verdad»). Se descarta: destruye el
comentario, hace imposible deshacer un error de identificación y convierte un `501` en una tentación de
borrado definitivo. La herramienta **nunca** envía `force=true`, ni como respaldo.

### D2 — Herramienta **propia** `blog_comment_delete`, no una operación de la existente

Tres razones: los permisos (una operación destructiva y una de publicación pueden divergir en el
futuro), el catálogo y sus tests tratan las herramientas por nombre (una herramienta nueva es un alta
explícita y una prueba de integridad que falla si se olvida), y no se altera la firma ni los escenarios
ya aprobados de `blog_comment_reply`.

**Alternativa descartada:** una herramienta multioperación `blog_comment` con `operation: reply|delete`.
Obligaría a tocar la spec, el esquema y los tests de la respuesta ya archivados, y a decidir permisos
por operación sin ninguna ganancia (ambas exigen `ExplicitApproval`).

### D3 — Lectura previa obligatoria, y es también la fuente de la evidencia

`GET {base}/wp-json/wp/v2/comments/{comment_id}` antes del borrado. Cumple dos funciones: es la
comprobación de que el comentario existe (si la lectura falla **no se envía ningún `DELETE`**) y es la
fuente de la evidencia —autor, extracto, estado previo y el `post_id` **real** del comentario—, que
existe en **todos** los desenlaces.

**Alternativas descartadas:** tomar la evidencia del cuerpo del `DELETE` (no lo hay en un `410`, y un
cuerpo ilegible dejaría el resultado vacío justo cuando el borrado sí ocurrió); confiar solo en el `404`
del `DELETE` como comprobación de existencia (deja sin evidencia los desenlaces correctos).

**Coste asumido:** dos peticiones por borrado (una lectura y un borrado) en vez de una. Es una operación
que el usuario aprueba explícitamente y no hay una cota de escritura que respetar, así que el coste es
irrelevante frente a la utilidad de la evidencia.

### D4 — Permiso `ExplicitApproval`

Coherente con la publicación y con `email_send`. El hecho de que la papelera sea reversible **no**
rebaja el permiso: el comentario deja de estar visible en el sitio, que es un cambio público mientras no
se restaure. El freno se declara en el registro, y la interfaz ya sabe pedir la confirmación.

### D5 — Sin cota propia de borrado

El freno es la confirmación; el daño lo acotan tres hechos del sistema (origen de los identificadores,
20 notificaciones por hora, papelera de 30 días). Una cota propia exigiría estado —tabla o contador—
para algo que el usuario ya aprueba una a una, y su número sería arbitrario: podría impedir un borrado
legítimo (una ráfaga de spam notificada) y haría que el resultado dependiera del historial de Valet en
vez de la voluntad del usuario. **Decisión del usuario**, no una omisión.

**Alternativa descartada:** un contador en `settings` (`BLOG_MAX_DELETES_PER_HOUR`) comprobado antes de
cada borrado. Añade estado mutable, una migración, una clave de configuración y un modo de fallo nuevo
(quedarse sin cupo), y su único efecto real es retrasar un borrado que el usuario ya pidió.

### D6 — Sin migración ni tabla: la idempotencia la da el `410`

En la respuesta, la tabla `comment_replies` existe porque el duplicado **crea contenido público** y por
eso es el fallo más caro. En el borrado no hay tal fallo: repetir el borrado es inocuo (el `410` dice que
ya está en la papelera) y reversible (la papelera conserva el comentario 30 días). Añadir una tabla
duplicaría el concepto de «qué comentarios ya se tocaron» para proteger a nadie.

### D7 — Desambiguación del `404` en el cliente, con dos variantes nuevas y compartidas

`error_from_status` deja de tratar el `404` como un estado inesperado más:

- `404` + `rest_no_route` → variante propia (propuesta: `WordpressError::RouteNotFound`) con el mensaje
  «la ruta ... no existe en el sitio: revisa la política REST del plugin», que es lo que hoy necesita
  quien publique respuestas.
- `404` + cualquier otro `code` → variante propia (propuesta: `WordpressError::CommentNotFound`) con
  «el comentario no existe en el sitio».

El cambio vive en la capa de cliente, así que **también** beneficia a `publish_comment_reply` sin tocar
su llamada. El `501` de papelera desactivada recibe su propia variante (propuesta:
`WordpressError::TrashUnsupported`) y el `403` reutiliza `Forbidden` con un mensaje **neutral respecto a
la operación** («no tiene permisos para esta operación: revisa el rol del usuario de servicio»), porque
el mismo 403 sirve a publicar y a borrar y el mensaje actual habla solo de publicar.

**Alternativa descartada:** dejar el `404` como estado inesperado y poner el texto explicativo en la
herramienta. La herramienta no ve el `code` —solo el estado ya traducido—, así que la desambiguación
tiene que ocurrir donde se lee el cuerpo.

### D8 — Traducción a `ToolError`

`CommentNotFound` → `Err(ToolError::NotFound)` (el recurso no existe, igual que un `notification_id`
desconocido). Todo lo demás → `Err(ToolError::ExecutionError)` con el mensaje accionable de la variante.
Ningún desenlace provoca `panic!`; un cuerpo ilegible en la lectura se trata como lectura fallida y no
borra nada.

### D9 — El `410` es un **éxito**, marcado como «ya estaba»

`ToolResult { success: true, data: { …, already_trashed: true } }`. El cometido de la herramienta —el
comentario en la papelera— se cumple igual. Devolver `success: false` haría que el modelo interpretase
que no se hizo nada y volviese a intentarlo o pidiese permiso otra vez. El mensaje lo dice de forma
inequívoca y **no se envía ninguna petición adicional** para confirmarlo.

### D10 — Reutilizar la resolución de identificadores en vez de duplicarla

El borrado usa la misma función pura de parseo y la misma lectura de la notificación
(`NotificationsRepo::get_by_id`) que la respuesta, de modo que una forma de `metadata` que se rechaza al
responder se rechaza al borrar. La validación ocurre **antes** de resolver la configuración y de tocar
la red.

## Risks / Trade-offs

- **[Un comentario notificado de una entrada y otro comentario distinto con el mismo id]** → no aplica: el
  identificador es el del comentario y la lectura previa devuelve su `post_id` **real**, que es el que se
  muestra como evidencia; cualquier divergencia con el `metadata` queda visible en el resultado.
- **[El borrado alcanza un comentario que el usuario no quería]** → mitigación: el origen de los
  identificadores es el `metadata` (nunca se buscan), la lectura previa devuelve autor, extracto y estado
  para que el resultado describa **qué** se movió, la operación exige `ExplicitApproval` y la papelera
  (30 días) permite restaurarlo desde el panel.
- **[Desambiguar el `404` cambia el camino ya probado de la respuesta]** → el `404` deja de caer en
  `Unexpected`; mitigación: la suite del publicador (15 tests del cliente, 26 de la herramienta) se
  re-ejecuta en cada paso, y las mutaciones de verificación del `tasks.md` incluyen quitar la
  desambiguación y comprobar que el escenario nuevo se pone rojo.
- **[Un sitio con `EMPTY_TRASH_DAYS = 0` responde `501`]** → la herramienta **no** borra definitivamente
  como respaldo; informa de que el sitio no admite papelera y deja el comentario intacto.
- **[Lectura previa fallida con un `5xx` y borrado que sí habría funcionado]** → se prefiere no borrar sin
  haber leído: el usuario puede reintentar y la aprobación explícita hará el trabajo. Queda consignado
  como coste asumido de D3.
- **[El `Debug`/los registros filtran la credencial]** → la credencial ya se resuelve y se usa como en la
  publicación (construida en la petición, `Debug` redactado) y los tests de la sección 3 del `tasks.md`
  inspeccionan el resultado y el registro capturado.

## Verificación pendiente (fuera de este change)

La prueba de **punta a punta** sigue bloqueada por dos dependencias externas, las mismas que arrastra el
publicador de respuestas:

1. **Aprovisionamiento manual en WordPress**: usuario de servicio con rol mínimo (`read` +
   `moderate_comments`) y su **Application Password** en `BLOG_APP_PASSWORD`. No se puede hacer desde
   este repositorio. El borrado **no añade aprovisionamiento nuevo**: usa el mismo usuario.
2. **El stack de desarrollo del sitio no está en marcha**, y las Application Passwords solo se admiten
   sobre SSL o con el entorno `local` (el sitio de desarrollo es HTTP con entorno `production`).

Comandos previstos para cerrar el bloqueo (los mismos del change anterior, desde el panel *Usuarios →
Perfil → Application Passwords* si no hay WP-CLI):

```bash
just wp -- user create valet-bot valet-bot@example.invalid --role=subscriber --user_pass="$(openssl rand -base64 24)"
just wp -- user add-role valet-bot moderate_comments
just wp -- user application-password create valet-bot "valet" --porcelain
BLOG_BASE_URL=https://atareao.es
BLOG_SERVICE_USER=valet-bot
BLOG_APP_PASSWORD=xxxx xxxx xxxx xxxx xxxx xxxx
```

Guion de la prueba cuando se desbloquee: aprobar un comentario real → comprobar que el aviso trae
`metadata.comment_id` y `metadata.post_id` → pedir al asistente que lo mande a la papelera → verificar en
el panel que el comentario está en la **papelera** (y no borrado) → repetir la petición y comprobar que
responde «ya estaba en la papelera» sin error y sin segunda petición → restaurarlo desde el panel. **No
forma parte de este change**: se consigna aquí y en `tasks.md` como casilla bloqueada.

## Decisiones de la aprobación (Open Questions resueltas)

1. **Nombres de las variantes nuevas** de `WordpressError`: se aprueban `RouteNotFound` y
   `CommentNotFound` (404) y `TrashUnsupported` (501), tal como se propusieron.
2. **`403` neutral**: se **reutiliza** `Forbidden` con un mensaje neutro respecto a la operación
   («no puede completar esta operación»); **no** se añade una variante `CannotDelete`. Los escenarios de
   la respuesta siguen cumpliéndose porque solo exigen que el mensaje hable del rol del usuario de
   servicio.
3. **Nombre de la función de parseo compartida**: se **renombra** a `comment_ids_from_metadata` y su
   struct a `CommentIds`, ahora que la usan las dos herramientas del blog. Se actualizaron todos los usos
   y tests; **no** quedan alias ni código muerto (`clippy -- -D warnings` limpio).
4. **`410` como `success: true` con `already_trashed: true`** en `data` (D9): confirmado. El desenlace se
   trata como éxito (el cometido —el comentario en la papelera— ya se cumple) y el modelo no debe verlo
   como una negativa.
5. **Cota del extracto**: 200 caracteres, ya aplicada en [`CommentSnapshot`] del cliente y comprobada por
   los tests.
