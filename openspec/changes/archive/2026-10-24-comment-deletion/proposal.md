# Change: Borrado de comentarios del blog a la papelera (`blog_comment_delete`)

## Why

El asistente ya publica respuestas en el sitio con `blog_comment_reply` (change archivado
`comment-reply-publisher`), pero **la capacidad de escritura del blog es de ida**: existe publicar y no
existe retirar. Cuando llega un comentario que no debe estar —spam que pasó el filtro, un comentario
duplicado, un aviso de un comentario ya borrado a mano en el panel—, el usuario tiene que abrir el
navegador, entrar al panel de WordPress, buscar el comentario y moverlo a la papelera él mismo; el
asistente puede explicarle qué hacer, pero no tiene con qué hacerlo.

Lo verificado por CLI en este repositorio es que **no hay ninguna escritura destructiva**:

```
grep -n "delete\|Delete\|DELETE\|force" src/services/wordpress.rs   → sin coincidencias
grep -rn "wp/v2/comments" src/                                      → 1 constante (COMMENTS_PATH, del POST)
```

La vía que falta es la del **ítem**: el núcleo de WordPress borra un comentario con
`DELETE /wp-json/wp/v2/comments/<id>`, y con `force=false` lo manda a la **papelera** —no lo destruye—,
que es exactamente el grado de reversibilidad que quiere esta funcionalidad. Es la pieza que cierra el
circuito de moderación abierto por `comment-reply-publisher`: el mismo aviso (`metadata.comment_id`,
`metadata.post_id`), la misma vía autenticada, el criterio contrario.

## What Changes

- **Nueva operación `blog_comment_delete`**, expuesta al modelo como **herramienta propia**
  (`src/tools/blog_comments.rs`, módulo ya existente), que manda a la **papelera** el comentario
  notificado contra la ruta del ítem: `DELETE {base}/wp-json/wp/v2/comments/{comment_id}` con
  `force=false` como parámetro de consulta, **sin cuerpo** y con la misma autenticación
  `Authorization: Basic base64(usuario:application_password)`. **`force=true` SHALL NOT enviarse nunca**:
  el borrado definitivo queda fuera de la herramienta.
- **Origen de los identificadores del borrado: el `metadata` de la notificación**, **exclusivamente**. El
  borrado usa la **misma** función pura de parseo y validación que ya usa la respuesta
  (`comment_ids_from_metadata`), pero con una **sola** vía: los identificadores salen del `metadata` de la
  notificación señalada por `notification_id`, que es **obligatorio** y la **única** vía (**D8**). Un
  `comment_id` o un `post_id` explícitos —en cualquier forma— se **rechazan** con `InvalidArguments` **sin
  salir a la red**, de modo que quien llama no puede elegir qué comentario se manda a la papelera. La vía
  explícita la conserva **solo** `blog_comment_reply`: la asimetría es **deliberada** (decisión del
  usuario; D8 alcanza solo al borrado). Sin identificadores resolubles, la herramienta devuelve
  `InvalidArguments` **sin salir a la red** y **nunca inventa** un id.
- **Lectura previa obligatoria.** Antes del borrado, la herramienta lee el comentario con
  `GET {base}/wp-json/wp/v2/comments/{comment_id}`. La lectura cumple dos funciones: es la
  comprobación de que el comentario existe (sin ella no se envía ningún `DELETE`) y es la fuente de la
  **evidencia** que el resultado devuelve —identificador, entrada, autor, extracto del comentario y
  **estado previo**—, también cuando el borrado acaba en `410` y el cuerpo de la respuesta no trae nada.
- **Permiso `Permission::ExplicitApproval`**, el mismo que la publicación y el mismo precedente que
  `email_send`: la operación cambia el estado de algo que ve el público exterior y **el freno de una
  operación destructiva es la confirmación explícita del usuario**.
- **Sin cota propia de borrado.** La herramienta SHALL NOT llevar contador, ventana temporal ni límite
  de borrados por hora: una cota propia exigiría estado (tabla o contador) para un freno que ya ejerce
  el usuario al aprobar cada borrado, y un límite arbitrario podría bloquear un borrado legítimo (una
  ráfaga de spam notificada). El daño está acotado por tres hechos del sistema: el sitio **no notifica
  más de 20 comentarios por hora**, la papelera de WordPress mantiene el comentario **30 días**
  (`EMPTY_TRASH_DAYS`, que el repositorio de WordPress no redefine) y los identificadores **solo salen
  de comentarios ya notificados**, así que no se puede alcanzar un comentario arbitrario del sitio.
- **Sin migración ni tabla.** A diferencia de la respuesta —donde publicar dos veces crea contenido
  público duplicado y por eso es el fallo caro—, el borrado es **idempotente y reversible**: el núcleo
  responde `410 rest_already_trashed` cuando el comentario ya está en la papelera, y ese desenlace se
  trata como **ya estaba hecho** (éxito, sin segunda petición). La idempotencia la da el sitio, no un
  registro propio.
- **Desambiguación del `404` en la capa de cliente, para las dos escrituras.** El `404` de WordPress no
  es un solo desenlace: `rest_no_route` significa que **la ruta no existe en el sitio** —es lo que hoy
  recibe el publicador de respuestas, porque el plugin del sitio elimina la ruta de colección de
  comentarios, y se repara en el change paralelo del repositorio de WordPress—; `rest_comment_invalid_id`
  significa que **el comentario no existe**. El cliente SHALL producir errores **distintos** para uno y
  otro, con un mensaje para `rest_no_route` que diga que la ruta no existe en el sitio y que hay que
  revisar la política REST del plugin, en vez de un genérico «no encontrado». La desambiguación
  **también** aplica a `publish_comment_reply`.
- **Política de desenlaces cerrada para el borrado**: `200` → papelera hecha; `410 rest_already_trashed`
  → ya estaba hecho (éxito idempotente, sin segunda petición); `404 rest_comment_invalid_id` → el
  comentario no existe (`ToolError::NotFound`); `404 rest_no_route` → la ruta no existe en el sitio;
  `403` → la credencial vale pero no puede borrar (rol del usuario de servicio); `501
  rest_trash_not_supported` → el sitio no tiene papelera (`EMPTY_TRASH_DAYS = 0`) y **Valet no borra
  definitivamente en su lugar**; `401`, `400`, `429`, `5xx`, timeout y fallo de red → los errores
  accionables ya existentes. **Ni un reintento**: como mucho una lectura y un borrado por invocación.
- **Evidencia sin credenciales.** El resultado lleva el identificador del comentario y de la entrada, el
  autor, un extracto recortado y el estado previo; **nunca** la Application Password, la cabecera
  `Authorization` ni el contenido íntegro. El registro (`tracing`) lleva identificadores, desenlace y
  latencia, y tampoco la credencial.
- **La skill `blog` pasa a exponer dos herramientas** (`blog_comment_reply` y `blog_comment_delete`): el
  catálogo **sigue teniendo once skills** y sus `criteria` cubren ahora el caso legítimo de **borrar el
  comentario que llegó como aviso**. El borrado **no** ensancha `pendientes` (donde `notifications` solo
  lee): un turno sobre recordatorios o tareas no gana capacidad de borrar en el sitio.

## Capabilities

### New Capabilities

Ninguna. La operación de borrado es una capacidad más de la ya existente `blog-comments`.

### Modified Capabilities

- `blog-comments` — la operación de borrado (`blog_comment_delete`), su contrato con la ruta del ítem,
  la lectura previa obligatoria, el permiso `ExplicitApproval`, la desambiguación del `404`
  (`rest_no_route` frente a `rest_comment_invalid_id`) extendida al publicador y la ausencia de cota con
  su justificación. La procedencia de los identificadores y el mapa de errores compartido se **modifican**
  para nombrar las dos herramientas.
- `tools/registry` — el registro de producción pasa de **treinta y una** a **treinta y dos**
  herramientas e incluye `blog_comment_delete`, con su permiso `ExplicitApproval` y su definición en
  español.
- `orchestrator/skill-router` — el catálogo **sigue declarando once skills**, `blog` cubre ahora las dos
  escrituras del blog, la unión del core más las skills cubre las **treinta y dos** herramientas sin
  huérfanas y las `criteria` de `blog` reconocen el borrado del comentario notificado.

## Impact

### Specs

- **Ninguna nueva.** Se modifican `blog-comments`, `tools/registry` y `orchestrator/skill-router`.

### Código

- `src/tools/blog_comments.rs` — segunda herramienta del módulo, reutilizando la resolución de
  identificadores (`comment_ids_from_metadata`) y la validación previa. Sin tabla ni reserva.
- `src/services/wordpress.rs` — nuevas llamadas del cliente (`fetch_comment` y `trash_comment`) y las
  variantes nuevas del enum cerrado de errores; `error_from_status` pasa a desambiguar el `404` por el
  `code`, con lo que `publish_comment_reply` también distingue `rest_no_route`. La base, el
  `User-Agent`, el timeout y el `Debug` que redacta la contraseña se reutilizan tal cual.
- `src/tools/mod.rs` y `build_tool_registry` (`src/lib.rs`) — registro de la herramienta nueva
  (**31 → 32** herramientas).
- `src/orchestrator/skills.rs` — `blog` cubre dos herramientas; tests de integridad del catálogo
  (`CATALOG` sigue con once entradas); `tests/api/tools.rs` sigue con once skills; la lista `ALL_TOOLS`
  de los tests de `src/orchestrator/skill_router.rs` pasa a 32.
- **Frontend: sin cambios.** La pestaña de skills se alimenta del catálogo de la API y no hay ninguna
  pantalla que enumere herramientas; la confirmación explícita ya está implementada por el permiso
  `ExplicitApproval` existente.

### Datos

- **Ninguna migración, ninguna tabla.** No hay estado propio del borrado: la idempotencia la da el `410`
  del sitio y la reversibilidad la da la papelera.

### Aprovisionamiento manual en WordPress (dependencia externa)

El borrado exige, **en el sitio**, que el usuario de servicio tenga la capacidad de moderar comentarios
(`moderate_comments`) y que su Application Password siga viva en `BLOG_APP_PASSWORD`. Es el **mismo**
usuario de servicio que ya necesita la publicación, así que este change **no añade aprovisionamiento
nuevo**: si la publicación funciona, el borrado funciona. Además, el sitio debe tener papelera
(`EMPTY_TRASH_DAYS` distinto de `0`, que es el valor por defecto de WordPress); si no la tuviera, el
núcleo responde `501 rest_trash_not_supported` y la herramienta informa en vez de borrar.

### Fuera de alcance (no-objetivos)

- **No** se añade cota de borrado (ni contador, ni ventana temporal, ni límite por hora): el freno es la
  confirmación explícita y el daño está acotado por la tasa de notificación del sitio, la papelera y el
  origen de los identificadores. Decisión del usuario, no un olvido.

  La acotación por la **papelera** la sostiene **nuestra disciplina de consumidor**: la herramienta envía
  **siempre** `force=false` y **nunca** `force=true`. Pero el sitio **no impide** `force=true`
  —`wp-includes/rest-api/class-wp-rest-comments-controller.php:1071` lee el parámetro `force` y
  `:1087-1089` ejecuta `wp_delete_comment($id, true)`—, así que la papelera es una **garantía del
  consumidor, no del sitio**: quien usara la API REST con credenciales propias podría borrar
  definitivamente. Ese hecho queda declarado como **riesgo residual** en el change de WordPress, no como
  algo que Valet pueda imponer.
- **No** se implementa el borrado definitivo (`force=true`), ni una herramienta aparte para vaciar la
  papelera, ni la restauración del comentario: el panel de WordPress sigue siendo el sitio donde se
  administra la papelera.
- **No** se puede borrar un comentario que no haya sido notificado: los identificadores salen siempre del
  `metadata` de una notificación y no se buscan ni se inventan.
- **No** se añade lectura ni listado de comentarios (leer el hilo, buscar comentarios por entrada,
  inspeccionar un comentario suelto): la única lectura de este change es la previa al borrado, y su
  resultado vive en la evidencia del propio resultado.
- **No** se modifica el contrato de ingesta (`IngestRequest` sigue con sus **cinco** campos y sin
  `deny_unknown_fields`).
- **No** se toca el repositorio de WordPress (`/data/php/atareao.es`), donde vive el change paralelo que
  repara la política REST del plugin.
- **No** se reintenta automáticamente ante `429`, `5xx`, timeout ni fallo de red.
