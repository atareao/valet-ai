# blog-comments Specification

## Purpose

La publicación de respuestas a comentarios del blog: la herramienta `blog_comment_reply` responde a un comentario del sitio WordPress por la API REST del núcleo, con Application Password, tomando los identificadores del `metadata` de la notificación que lo anunció. Recuerda en una tabla propia qué comentarios ya tienen respuesta, traduce cada fallo de la vía —credencial, rol, cota de escritura, petición, red— a un resultado tipado que el modelo pueda explicar, y nunca publica dos veces la misma respuesta.

## ADDED Requirements

### Requirement: La tabla `comment_replies` SHALL recordar qué comentarios ya tienen respuesta

La tabla `comment_replies` SHALL tener las columnas `comment_id` (`TEXT` **PRIMARY KEY**, el id del
comentario del blog al que se responde, en su forma decimal), `post_id` (`TEXT` NOT NULL, el id de la
entrada), `notification_id` (`TEXT` NULL, el id de la notificación de la que salieron los
identificadores, cuando se conozca), `state` (`TEXT` NOT NULL, uno de `'reserved'`, `'published'` o
`'uncertain'`), `wp_comment_id` (`TEXT` NULL, el id que devuelve WordPress al crear la respuesta),
`last_status` (`INTEGER` NULL, el código HTTP del último intento), `created_at` (`TEXT` NOT NULL
DEFAULT `(datetime('now'))`) y `updated_at` (`TEXT` NOT NULL DEFAULT `(datetime('now'))`). La tabla
SHALL declarar un índice `idx_comment_replies_post ON comment_replies(post_id, created_at)`. La
migración que la crea SHALL ser **aditiva e idempotente** (`CREATE TABLE IF NOT EXISTS`) y SHALL NOT
reconstruir ninguna tabla existente. La fila SHALL escribirse con una única sentencia
`INSERT ... ON CONFLICT(comment_id) DO NOTHING`, de modo que la reserva de un comentario sea atómica.

No existe hoy ninguna tabla donde quepa este registro: `settings` es clave-valor de valor único,
`tools` es el catálogo del registry y `notifications` es la bandeja de entrada. Por eso se añade una
tabla propia en vez de reutilizar una existente.

#### Scenario: Una reserva se materializa con un solo INSERT atómico

- **Given** la tabla `comment_replies` migrada y sin filas
- **When** se reserva el comentario `901` de la entrada `42`
- **Then** existe una fila con `comment_id` `901`, `post_id` `42` y estado `reserved`
- **And** la sentencia ha afectado a una sola fila

#### Scenario: Reservar dos veces el mismo comentario no crea una segunda fila

- **Given** una reserva ya tomada para el comentario `901`
- **When** se vuelve a reservar el comentario `901`
- **Then** la sentencia afecta a **cero** filas
- **And** la tabla sigue teniendo una sola fila para `901`

#### Scenario: La migración es aditiva e idempotente

- **Given** una base de datos ya migrada, con filas en `comment_replies`
- **When** se vuelve a aplicar la migración
- **Then** no falla
- **And** la definición de la tabla y la fila existente NO cambian
- **And** no se reconstruye ninguna tabla preexistente

### Requirement: La herramienta `blog_comment_reply` SHALL publicar la respuesta por la API REST del núcleo de WordPress

La herramienta `blog_comment_reply` SHALL publicar una respuesta a un comentario del blog con
`POST {base}/wp-json/wp/v2/comments`, autenticada con **Application Password** mediante la cabecera
`Authorization: Basic base64(usuario:application_password)`. El cuerpo SHALL llevar `post` (id de la
entrada), `parent` (id del comentario al que se responde), `content` (la respuesta) y
`status: "approved"`, y SHALL NOT llevar `author_name` ni `author_email`: con ellos el comentario
nace **pendiente** en vez de aprobado en un solo paso. Los parámetros SHALL ser `content`
(obligatorio), `comment_id` y `post_id` (obligatorios salvo que se indique `notification_id`), y
`notification_id` (opcional, mutuamente excluyente con los dos anteriores). `content` SHALL
recortarse, SHALL NOT poder quedar vacío y SHALL NOT superar los 4000 caracteres, y ambos límites
SHALL comprobarse **sin salir a la red**. Se SHALL considerar éxito únicamente un `HTTP 201`.

#### Scenario: Una respuesta válida se publica aprobada

- **Given** una configuración válida y un comentario `901` de la entrada `42`
- **When** se ejecuta `blog_comment_reply` con ese `comment_id`, ese `post_id` y un `content` no vacío
- **Then** la petición es un `POST` a `/wp-json/wp/v2/comments`
- **And** el cuerpo lleva `post` `42`, `parent` `901`, el `content` recortado y `status` `approved`
- **And** el resultado es un éxito con el `id` de la respuesta creada y el estado `approved`

#### Scenario: No se envían `author_name` ni `author_email`

- **Given** una llamada válida a `blog_comment_reply`
- **When** se inspecciona el cuerpo de la petición enviada a WordPress
- **Then** el cuerpo NO contiene `author_name`
- **And** el cuerpo NO contiene `author_email`

#### Scenario: Un `content` vacío o demasiado largo se rechaza sin salir a la red

- **Given** una llamada con `content` en blanco, o con más de 4000 caracteres
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición

#### Scenario: Un 201 con cuerpo ilegible no provoca un segundo intento

- **Given** un servidor de prueba que responde `201` con un cuerpo que no es JSON
- **When** se ejecuta `blog_comment_reply`
- **Then** el resultado es un éxito con el `wp_comment_id` ausente
- **And** el comentario queda registrado como `published`
- **And** no se repite la petición

### Requirement: La herramienta SHALL resolver los identificadores desde el `metadata` de la notificación y SHALL NOT inventarlos

Cuando se indique `notification_id`, la herramienta SHALL leer esa notificación de la bandeja y SHALL
resolver `comment_id` y `post_id` a partir de su `metadata`, que llega como **cadena JSON** (o
`NULL`). El valor de cada identificador SHALL aceptarse como número **entero positivo** o como cadena
de dígitos y SHALL normalizarse a su forma decimal; cualquier otra forma SHALL rechazarse. La
herramienta SHALL NOT publicar cuando falte cualquiera de los dos identificadores y SHALL NOT
sustituirlos por ningún valor por defecto, inventado o calculado: en su lugar SHALL devolver
`Err(ToolError::InvalidArguments)` con un mensaje que indique de dónde obtenerlos, **sin salir a la
red**. Indicar `notification_id` junto con `comment_id` o `post_id` SHALL ser `InvalidArguments`. Un
`notification_id` que no exista SHALL ser `Err(ToolError::NotFound)`, también sin salir a la red. Los
dos identificadores de una misma respuesta SHALL proceder siempre del mismo `metadata`, que es la
misma procedencia que WordPress usa al crearlos; la herramienta SHALL NOT comprobar contra WordPress
que el comentario pertenece a la entrada.

#### Scenario: Un `metadata` con identificadores numéricos se resuelve

- **Given** una notificación cuyo `metadata` es `{"comment_id": 901, "post_id": 42}`
- **When** se ejecuta `blog_comment_reply` con su `notification_id`
- **Then** se publica la respuesta al comentario `901` de la entrada `42`

#### Scenario: Un `metadata` con identificadores en cadena se normaliza

- **Given** una notificación cuyo `metadata` es `{"comment_id": "901", "post_id": "42"}`
- **When** se ejecuta `blog_comment_reply` con su `notification_id`
- **Then** se usa el comentario `901` y la entrada `42`, en su forma decimal

#### Scenario: Un `metadata` ausente o NULL no publica nada

- **Given** una notificación sin `metadata`
- **When** se ejecuta `blog_comment_reply` con su `notification_id`
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** el mensaje explica que la notificación no trae los identificadores
- **And** no se envía ninguna petición

#### Scenario: Un `metadata` que no es JSON válido no publica nada

- **Given** una notificación cuyo `metadata` no parsea como JSON
- **When** se ejecuta `blog_comment_reply` con su `notification_id`
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición

#### Scenario: Un `metadata` que no es objeto no publica nada

- **Given** una notificación cuyo `metadata` es un arreglo o un escalar JSON
- **When** se ejecuta `blog_comment_reply` con su `notification_id`
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición

#### Scenario: Un identificador que falta o tiene una forma inválida no publica nada

- **Given** un `metadata` al que le falta `post_id`, o con `comment_id` igual a `0`, a `-3`, a `1.5`, a `null`, a `true`, a `[]`, a `{}` o a una cadena vacía
- **When** se ejecuta `blog_comment_reply`
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** el mensaje nombra el identificador que falta o no es válido
- **And** no se envía ninguna petición

#### Scenario: Un `notification_id` inexistente no publica nada

- **Given** un `notification_id` que no figura en la bandeja
- **When** se ejecuta `blog_comment_reply`
- **Then** devuelve `Err(ToolError::NotFound)`
- **And** no se envía ninguna petición

#### Scenario: Mezclar `notification_id` con identificadores explícitos se rechaza

- **Given** una llamada con `notification_id`, `comment_id` y `post_id` a la vez
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición

#### Scenario: Sin identificadores y sin `notification_id` se informa en vez de inventar

- **Given** una llamada con solo `content`
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** el mensaje indica que los identificadores se obtienen del `metadata` de la notificación con la herramienta `notifications`
- **And** no se envía ninguna petición

### Requirement: Las credenciales SHALL resolverse desde el entorno y SHALL NOT filtrarse

La herramienta SHALL resolver la URL base, el usuario de servicio y la Application Password de las
variables de entorno `BLOG_BASE_URL`, `BLOG_SERVICE_USER` y `BLOG_APP_PASSWORD` en **cada llamada**,
de modo que un cambio en el entorno no exija reiniciar. `BLOG_BASE_URL` SHALL tener como valor por
defecto `https://atareao.es`; `BLOG_SERVICE_USER` y `BLOG_APP_PASSWORD` SHALL NOT tener ningún valor
por defecto. Sin usuario o sin contraseña la herramienta SHALL NOT enviar ninguna petición y SHALL
devolver un error accionable que nombre las variables de entorno que faltan, **sin** provocar un
`panic!`. La contraseña de aplicación SHALL NOT aparecer en ningún registro, ni en la telemetría, ni
en el resultado de la herramienta, ni en un mensaje de error, ni en la representación `Debug` de la
configuración, que SHALL redactarla; la cabecera `Authorization` SHALL construirse en el momento de la
petición y SHALL NOT guardarse ni registrarse.

#### Scenario: Sin credenciales no hay ninguna petición

- **Given** un servidor de prueba y una configuración sin `BLOG_SERVICE_USER` y sin `BLOG_APP_PASSWORD`
- **When** se ejecuta `blog_comment_reply` con argumentos válidos
- **Then** devuelve un error accionable que nombra las variables que faltan
- **And** el servidor de prueba no recibe ninguna petición

#### Scenario: La URL base tiene un valor por defecto

- **Given** una configuración sin `BLOG_BASE_URL`
- **When** se resuelve la configuración
- **Then** la URL base es `https://atareao.es`

#### Scenario: La contraseña no aparece en la representación de depuración

- **Given** una configuración con una contraseña de aplicación conocida
- **When** se formatea su `Debug`
- **Then** la contraseña NO aparece
- **And** la URL base y el usuario sí aparecen

#### Scenario: La contraseña no aparece en un mensaje de error

- **Given** un servidor de prueba que responde `401` con un cuerpo arbitrario
- **When** se ejecuta `blog_comment_reply`
- **Then** el mensaje del error NO contiene la contraseña de aplicación
- **And** el mensaje NO contiene la cabecera `Authorization`

### Requirement: La herramienta SHALL NOT publicar dos veces la respuesta al mismo comentario

Antes de enviar nada, la herramienta SHALL reservar el `comment_id` en `comment_replies`. Si la
reserva no se materializa —porque ya hay una fila en estado `reserved`, `published` o `uncertain`—
la herramienta SHALL NOT enviar ninguna petición y SHALL devolver un resultado **sin éxito** cuyo
mensaje explique que ese comentario ya tiene una respuesta registrada o quedó en un estado
indeterminado, junto con el estado y la fecha del registro. La reserva SHALL **liberarse** únicamente
ante desenlaces que garanticen que **no** se creó ningún comentario: `400`, `401`, `403` y `429`. La
reserva SHALL **conservarse** ante un desenlace ambiguo —timeout, fallo de red o `5xx`— y quedar en
estado `uncertain`, y SHALL conservarse también en estado `reserved` si el proceso muere entre la
reserva y el desenlace. En éxito, la fila SHALL quedar en estado `published` con el `wp_comment_id`
devuelto por WordPress. Dos invocaciones concurrentes con el mismo `comment_id` SHALL producir **una
sola** publicación.

#### Scenario: Una segunda llamada no sale a la red

- **Given** un comentario ya registrado como `published`
- **When** se vuelve a ejecutar `blog_comment_reply` para ese mismo comentario
- **Then** el resultado no es un éxito y su mensaje lo explica
- **And** el servidor de prueba NO recibe ninguna petición

#### Scenario: Un 429 libera la reserva

- **Given** un servidor de prueba que responde `429` a la primera llamada y `201` a la segunda
- **When** se ejecuta `blog_comment_reply` y después se repite
- **Then** la primera llamada informa de la cota agotada
- **And** la segunda llamada publica la respuesta

#### Scenario: Un timeout conserva la reserva como indeterminada

- **Given** un servidor de prueba que no responde dentro del timeout
- **When** se ejecuta `blog_comment_reply`
- **Then** el error informa de que no se pudo confirmar la publicación
- **And** la fila queda en estado `uncertain`
- **And** una segunda llamada para el mismo comentario NO sale a la red

#### Scenario: Dos llamadas concurrentes publican una sola vez

- **Given** un servidor de prueba que responde `201`
- **When** se ejecutan dos `blog_comment_reply` concurrentes para el mismo `comment_id`
- **Then** el servidor de prueba recibe exactamente una petición
- **And** solo una de las dos llamadas informa de un éxito

### Requirement: La herramienta SHALL traducir los fallos de la vía a un resultado tipado y SHALL NOT reintentar

La herramienta SHALL traducir cada desenlace de la vía a un error tipado y accionable, y SHALL NOT
reintentar en ningún caso: SHALL enviarse **una sola** petición por invocación. El `401` SHALL
informar de que la credencial no es válida o ha caducado; el `403`, de que la credencial es válida
pero no puede publicar y de que hay que revisar el rol del usuario de servicio; el `429`, de que la
cota de escritura está agotada, de que la respuesta **no** se publicó y de que **no** se debe
reintentar en bucle; el `400`, de que la petición es inválida; y un fallo de red, un timeout o un
`5xx`, de que el sitio no está disponible. Cualquier otro estado SHALL producir un error que lleve el
código HTTP y el código de error de WordPress **sin** su mensaje libre, para no incorporar texto de
terceros al contexto del modelo. Ningún desenlace SHALL provocar un `panic!` ni tumbar el proceso,
tampoco un cuerpo de respuesta ilegible o que no sea JSON.

#### Scenario: Un 401 informa de la credencial

- **Given** un servidor de prueba que responde `401`
- **When** se ejecuta `blog_comment_reply`
- **Then** el error informa de que la credencial no es válida o ha caducado
- **And** se ha enviado una sola petición

#### Scenario: Un 403 informa del rol del usuario de servicio

- **Given** un servidor de prueba que responde `403`
- **When** se ejecuta `blog_comment_reply`
- **Then** el error indica que hay que revisar el rol del usuario de servicio
- **And** el error es distinto del de una credencial inválida

#### Scenario: Un 429 no reintenta

- **Given** un servidor de prueba que responde `429` con el código `atareao_valet_reply_rate_limited`
- **When** se ejecuta `blog_comment_reply`
- **Then** el error informa de la cota de escritura agotada y de que la respuesta no se publicó
- **And** el servidor de prueba recibe exactamente una petición

#### Scenario: Un 400 informa de la petición inválida

- **Given** un servidor de prueba que responde `400`
- **When** se ejecuta `blog_comment_reply`
- **Then** el error informa de que la petición no es válida
- **And** el error NO es un fallo de disponibilidad

#### Scenario: Un 5xx o un fallo de red no tumba el proceso

- **Given** un servidor de prueba que responde `503`, o que no está disponible
- **When** se ejecuta `blog_comment_reply`
- **Then** el error informa de que el sitio no está disponible
- **And** la herramienta no entra en pánico
- **And** se ha enviado una sola petición

#### Scenario: Un estado inesperado no arrastra el mensaje de WordPress

- **Given** un servidor de prueba que responde `418` con un cuerpo que lleva un `code` y un `message`
- **When** se ejecuta `blog_comment_reply`
- **Then** el error lleva el código HTTP y el `code` de WordPress
- **And** el error NO lleva el `message` libre del cuerpo

### Requirement: La publicación SHALL registrarse sin volcar el contenido

Cada intento SHALL registrarse con `tracing`. El registro de un éxito SHALL incluir el `comment_id`,
el `post_id`, el `notification_id` cuando lo haya, el `wp_comment_id`, el estado y la latencia, y
SHALL NOT incluir el contenido de la respuesta. El registro de una negativa por reserva previa SHALL
incluir el `comment_id` y el estado del registro. El registro de un fallo SHALL incluir el
`comment_id` y el motivo tipado, y SHALL NOT incluir la credencial, ni la cabecera `Authorization`,
ni el contenido.

#### Scenario: El registro de un éxito no lleva el contenido

- **Given** una publicación correcta con un contenido conocido
- **When** se inspecciona el registro
- **Then** aparecen el `comment_id`, el `post_id` y el `wp_comment_id`
- **And** el contenido NO aparece en el registro

#### Scenario: El registro de un fallo no lleva la credencial

- **Given** una publicación que falla por credencial inválida
- **When** se inspecciona el registro
- **Then** el `comment_id` aparece
- **And** ni la contraseña de aplicación ni la cabecera `Authorization` aparecen
