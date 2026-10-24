## MODIFIED Requirements

### Requirement: La herramienta SHALL resolver los identificadores desde el `metadata` de la notificación y SHALL NOT inventarlos

Cuando se indique `notification_id` —tanto en `blog_comment_reply` como en `blog_comment_delete`—, la
herramienta SHALL leer esa notificación de la bandeja y SHALL resolver `comment_id` y `post_id` a partir
de su `metadata`, que llega como **cadena JSON** (o `NULL`). El valor de cada identificador SHALL
aceptarse como número **entero positivo** o como cadena de dígitos y SHALL normalizarse a su forma
decimal; cualquier otra forma SHALL rechazarse. La herramienta SHALL NOT operar cuando falte cualquiera
de los dos identificadores y SHALL NOT sustituirlos por ningún valor por defecto, inventado o calculado:
en su lugar SHALL devolver `Err(ToolError::InvalidArguments)` con un mensaje que indique de dónde
obtenerlos, **sin salir a la red**. Indicar `notification_id` junto con `comment_id` o `post_id` SHALL
ser `InvalidArguments`. Un `notification_id` que no exista SHALL ser `Err(ToolError::NotFound)`, también
sin salir a la red. Los dos identificadores de una misma operación SHALL proceder siempre del mismo
`metadata`, que es la misma procedencia que WordPress usa al crearlos; la herramienta SHALL NOT comprobar
contra WordPress que el comentario pertenece a la entrada. Las dos herramientas del blog SHALL compartir
**la misma** resolución: el parseo del `metadata` y su validación SHALL ser una sola pieza, de modo que
una forma que se rechaza al responder se rechace también al borrar.

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

#### Scenario: El borrado resuelve los identificadores igual que la respuesta

- **Given** una notificación cuyo `metadata` es `{"comment_id": 901, "post_id": 42}`
- **When** se ejecuta `blog_comment_delete` con su `notification_id`
- **Then** la lectura previa y el borrado se dirigen al comentario `901` de la entrada `42`
- **And** una llamada de borrado sin identificadores resolubles devuelve `Err(ToolError::InvalidArguments)` sin salir a la red

### Requirement: La herramienta SHALL traducir los fallos de la vía a un resultado tipado y SHALL NOT reintentar

La herramienta SHALL traducir cada desenlace de la vía a un error tipado y accionable, y SHALL NOT
reintentar en ningún caso: SHALL enviarse **una sola** petición de escritura por invocación. El `401`
SHALL informar de que la credencial no es válida o ha caducado; el `403`, de que la credencial es válida
pero no puede completar la operación y de que hay que revisar el rol del usuario de servicio; el `429`,
de que la cota de escritura del sitio está agotada, de que la operación **no** se completó y de que **no**
se debe reintentar en bucle; el `400`, de que la petición es inválida; y un fallo de red, un timeout o un
`5xx`, de que el sitio no está disponible. El `404` SHALL **desambiguarse por el `code`** que devuelve
WordPress: un `404` con `rest_no_route` SHALL producir un error **propio** que diga que la ruta solicitada
**no existe en el sitio** y que hay que revisar la política REST del plugin —nunca un «no existe» o un
«no encontrado» genérico—, mientras que un `404` con cualquier otro `code` (`rest_comment_invalid_id`)
SHALL producir un error **distinto** que diga que el comentario no existe. Ambos SHALL ser errores
distintos entre sí y del de un estado inesperado. Cualquier otro estado SHALL producir un error que lleve
el código HTTP y el código de error de WordPress **sin** su mensaje libre, para no incorporar texto de
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

#### Scenario: Un 404 de ruta ausente se distingue del comentario inexistente

- **Given** un servidor de prueba que responde `404` con el código `rest_no_route`
- **When** se ejecuta cualquiera de las dos herramientas del blog
- **Then** el error informa de que la ruta no existe en el sitio y de que hay que revisar la política REST del plugin
- **And** el error es distinto del que se produce con un `404` cuyo código es `rest_comment_invalid_id`
- **And** el error no menciona la contraseña de aplicación ni la cabecera `Authorization`

#### Scenario: El publicador de respuestas también desambigua la ruta ausente

- **Given** un servidor de prueba que responde `404` con el código `rest_no_route` a la publicación de una respuesta
- **When** se ejecuta `blog_comment_reply`
- **Then** el error informa de que la ruta de la API REST no existe en el sitio y de que hay que revisar la política REST del plugin
- **And** se ha enviado una sola petición
- **And** no se reintenta

### Requirement: La herramienta SHALL NOT publicar dos veces la respuesta al mismo comentario

Antes de enviar nada, la herramienta SHALL reservar el `comment_id` en `comment_replies`. Si la
reserva no se materializa —porque ya hay una fila en estado `reserved`, `published` o `uncertain`—
la herramienta SHALL NOT enviar ninguna petición y SHALL devolver un resultado **sin éxito** cuyo
mensaje explique que ese comentario ya tiene una respuesta registrada o quedó en un estado
indeterminado, junto con el estado y la fecha del registro. La reserva SHALL **liberarse** únicamente
ante desenlaces que garanticen que **no** se creó ningún comentario: `400`, `401`, `403` y `429`, y
también un `404` —con `rest_comment_invalid_id` o con `rest_no_route`—, porque un comentario o una ruta
que no existen en el sitio garantizan igualmente que no se creó nada. La reserva SHALL **conservarse**
ante un desenlace ambiguo —timeout, fallo de red o `5xx`— y quedar en estado `uncertain`, y SHALL
conservarse también en estado `reserved` si el proceso muere entre la reserva y el desenlace. En éxito,
la fila SHALL quedar en estado `published` con el `wp_comment_id` devuelto por WordPress. Dos
invocaciones concurrentes con el mismo `comment_id` SHALL producir **una sola** publicación.

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

#### Scenario: Un 404 al publicar libera la reserva

- **Given** un servidor de prueba que responde `404` —con `rest_comment_invalid_id` y, en otro caso, con `rest_no_route`— a la primera llamada y `201` a la segunda
- **When** se ejecuta `blog_comment_reply` y después se repite, para cada uno de los dos códigos
- **Then** cada `404` libera la reserva: la segunda llamada publica la respuesta
- **And** ninguna reserva queda en estado `uncertain` por un `404`

## ADDED Requirements

### Requirement: La herramienta `blog_comment_delete` SHALL mandar a la papelera el comentario notificado y SHALL NOT borrarlo definitivamente

La herramienta `blog_comment_delete` SHALL mandar a la **papelera** el comentario del blog indicado,
enviando `DELETE {base}/wp-json/wp/v2/comments/{comment_id}` con `force=false` como parámetro de
consulta y **sin cuerpo**, autenticada con la misma Application Password y la misma cabecera
`Authorization: Basic base64(usuario:application_password)` que la publicación, resuelta de las mismas
variables de entorno `BLOG_BASE_URL`, `BLOG_SERVICE_USER` y `BLOG_APP_PASSWORD` en **cada llamada**. La
herramienta SHALL NOT enviar `force=true` en ningún caso: el borrado definitivo queda fuera de su
contrato, y sin usuario o sin contraseña SHALL NOT enviar ninguna petición y SHALL devolver el error
accionable que nombra las variables que faltan. Su **único** parámetro SHALL ser `notification_id`,
que SHALL ser **obligatorio**: de su `metadata` se resuelven el comentario y la entrada con la
procedencia y la validación del requisito de resolución de identificadores. El borrado SHALL NOT
aceptar `comment_id` ni `post_id` aportados por quien llama —un `comment_id` o un `post_id` presente
SHALL rechazarse con `Err(ToolError::InvalidArguments)` **sin salir a la red**—, de modo que quien llama
no pueda elegir qué comentario se manda a la papelera: los identificadores salen **siempre** del aviso
indicado en `notification_id`. El permiso de la herramienta SHALL ser
`Permission::ExplicitApproval`: mandar un comentario a la papelera es una operación destructiva sobre
algo que ve el público y SHALL exigir la confirmación explícita del usuario, con independencia de que
sea reversible. Antes de borrar, la herramienta SHALL leer el comentario con
`GET {base}/wp-json/wp/v2/comments/{comment_id}` y SHALL NOT enviar ningún `DELETE` si esa lectura
falla; por invocación SHALL enviarse **como mucho una lectura y un borrado**, sin reintentos.

#### Scenario: El borrado manda el comentario a la papelera

- **Given** una configuración válida y una notificación cuyo `metadata` es `{"comment_id": 901, "post_id": 42}`, con el comentario `901` que no está en la papelera
- **When** se ejecuta `blog_comment_delete` con el `notification_id` de esa notificación
- **Then** la primera petición es un `GET` a `/wp-json/wp/v2/comments/901`
- **And** la segunda es un `DELETE` a `/wp-json/wp/v2/comments/901` con `force` `false` y sin cuerpo
- **And** ambas llevan `Authorization: Basic` con la credencial del usuario de servicio
- **And** el resultado es un éxito que dice que el comentario ha quedado en la papelera

#### Scenario: El borrado no acepta identificadores explícitos

- **Given** una llamada a `blog_comment_delete` con `comment_id` y `post_id` —aunque apunten a un comentario que existe y sería borrable— y sin `notification_id`
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)` nombrando que los identificadores salen del `metadata` del aviso indicado con `notification_id`
- **And** el servidor de prueba NO recibe ninguna petición, ni la lectura previa ni el borrado
- **And** el comentario no queda tocado

#### Scenario: El borrado exige `notification_id`

- **Given** una llamada a `blog_comment_delete` sin `notification_id`
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** el mensaje remite a `notification_id`
- **And** no se envía ninguna petición

#### Scenario: El borrado nunca es definitivo

- **Given** una llamada válida a `blog_comment_delete`
- **When** se inspeccionan las peticiones enviadas a WordPress
- **Then** el parámetro `force` vale `false`
- **And** ninguna petición lleva `force` `true`

#### Scenario: Sin la lectura previa no se borra nada

- **Given** un servidor de prueba cuya lectura del comentario falla (un `503` o una conexión rechazada)
- **When** se ejecuta `blog_comment_delete` con argumentos válidos
- **Then** el error informa de que no se pudo leer el comentario y de que no se ha borrado nada
- **And** el servidor de prueba NO recibe ninguna petición `DELETE`

#### Scenario: Sin credenciales no hay ninguna petición

- **Given** un servidor de prueba y una configuración sin `BLOG_SERVICE_USER` y sin `BLOG_APP_PASSWORD`
- **When** se ejecuta `blog_comment_delete` con argumentos válidos
- **Then** devuelve un error accionable que nombra las variables que faltan
- **And** el servidor de prueba no recibe ninguna petición

#### Scenario: El borrado exige aprobación explícita

- **Given** la herramienta `blog_comment_delete` registrada
- **When** se consulta su permiso para unos argumentos cualesquiera
- **Then** el permiso es `ExplicitApproval`

### Requirement: El resultado del borrado SHALL aportar evidencia sin credenciales

El resultado de un borrado SHALL identificar qué se ha movido a la papelera con la **evidencia** tomada
de la **lectura previa**: el identificador del comentario, el identificador de la entrada, el autor, un
**extracto** del comentario y su **estado previo**. El extracto SHALL proceder siempre de la lectura
previa —nunca inventado ni recortado del cuerpo de una respuesta de error— y SHALL estar acotado a 200
caracteres. Cuando el comentario ya estuviera en la papelera, el resultado SHALL indicarlo
explícitamente, con la misma evidencia. Ni el resultado, ni el mensaje, ni un mensaje de error, ni
ningún registro SHALL contener la Application Password ni la cabecera `Authorization`; el registro de un
intento SHALL llevar los dos identificadores, el estado previo, el desenlace y la latencia, y SHALL NOT
llevar el contenido íntegro del comentario ni la credencial.

#### Scenario: La evidencia nombra el comentario, su autor, su extracto y su estado

- **Given** una lectura previa de un comentario con autor y contenido conocidos, en estado `approved`
- **When** el borrado termina en `200`
- **Then** el resultado lleva el `comment_id`, el `post_id`, el autor, un extracto del contenido y el estado previo
- **And** el resultado informa de que el comentario está en la papelera

#### Scenario: El extracto está acotado

- **Given** un comentario cuyo contenido supera los 200 caracteres
- **When** se ejecuta `blog_comment_delete`
- **Then** el extracto del resultado tiene como mucho 200 caracteres

#### Scenario: La credencial no aparece ni en el resultado ni en el registro

- **Given** un borrado que falla con un `403`
- **When** se inspeccionan el mensaje del error y el registro del intento
- **Then** ni la contraseña de aplicación ni la cabecera `Authorization` aparecen
- **And** el registro lleva el `comment_id` y el desenlace
- **And** el contenido íntegro del comentario NO aparece en el registro

### Requirement: El borrado SHALL tratar cada desenlace de la vía con una política cerrada y sin reintentos

Cada desenlace de la vía de borrado SHALL traducirse a un resultado tipado y accionable, sin estado
propio en Valet:

- Un `200` SHALL ser el éxito: el comentario ha quedado en la **papelera**.
- Un `410` con el código `rest_already_trashed` SHALL tratarse como **ya estaba hecho**: el estado
  deseado (el comentario en la papelera) ya se cumple, así que el resultado SHALL ser un éxito que lo
  diga, SHALL informar de que el comentario ya estaba en la papelera y SHALL NOT provocar **ninguna**
  segunda petición —ni otra lectura ni otro borrado—.
- Un `404` con el código `rest_comment_invalid_id` —en la lectura o en el borrado— SHALL devolver
  `Err(ToolError::NotFound)` con un mensaje que diga que el comentario no existe en el sitio, y SHALL NOT
  haber borrado nada.
- Un `404` con el código `rest_no_route` SHALL producir el error propio de **ruta ausente** que describe
  el requisito compartido de traducción de fallos, y SHALL NOT confundirse con el comentario inexistente.
- Un `403` SHALL informar de que la credencial es válida pero no puede borrar y de que hay que revisar el
  rol del usuario de servicio, y SHALL ser distinto del error de credencial inválida.
- Un `501` con el código `rest_trash_not_supported` SHALL informar de que el sitio **no tiene papelera**
  (su `EMPTY_TRASH_DAYS` es `0`) y de que Valet **no** borra definitivamente en su lugar; el comentario
  SHALL quedar intacto.
- Un `401`, un `400`, un `429`, un `5xx`, un timeout o un fallo de red SHALL producir los errores
  accionables del mapa compartido, y SHALL NOT reintentarse.
- Cualquier otro estado SHALL producir un error que lleve el código HTTP y el `code` de WordPress **sin**
  su mensaje libre.

Salvo el `404` de comentario inexistente —que SHALL llegar al modelo como `Err(ToolError::NotFound)`—,
todos los fallos SHALL llegar como `Err(ToolError::ExecutionError)` con el mensaje accionable, sin
provocar un `panic!` ni tumbar el proceso. La herramienta SHALL NOT registrar ni consultar ningún
estado propio del borrado —no hay tabla, ni reserva, ni contador—: la idempotencia SHALL venir del `410`
del sitio y la reversibilidad del hecho de que el comentario vive en la papelera.

#### Scenario: Un 410 se trata como ya estaba hecho, sin segunda petición

- **Given** un servidor de prueba que responde la lectura con `200` y el borrado con `410` y el código `rest_already_trashed`
- **When** se ejecuta `blog_comment_delete`
- **Then** el resultado es un éxito que dice que el comentario ya estaba en la papelera
- **And** el servidor de prueba recibe exactamente una lectura y un borrado
- **And** no se envía ninguna petición adicional

#### Scenario: Un comentario inexistente se informa como no encontrado

- **Given** un servidor de prueba que responde la lectura con `404` y el código `rest_comment_invalid_id`
- **When** se ejecuta `blog_comment_delete`
- **Then** devuelve `Err(ToolError::NotFound)` con un mensaje que dice que el comentario no existe
- **And** el servidor de prueba NO recibe ninguna petición `DELETE`

#### Scenario: Un 403 de borrado informa del rol y no es un 401

- **Given** un servidor de prueba que responde la lectura con `200` y el borrado con `403` y el código `rest_cannot_delete`
- **When** se ejecuta `blog_comment_delete`
- **Then** el error informa de que hay que revisar el rol del usuario de servicio
- **And** el error es distinto del de una credencial inválida
- **And** se ha enviado una sola petición de borrado

#### Scenario: Un 501 informa de que el sitio no tiene papelera y no borra definitivamente

- **Given** un servidor de prueba que responde el borrado con `501` y el código `rest_trash_not_supported`
- **When** se ejecuta `blog_comment_delete`
- **Then** el error informa de que el sitio no admite la papelera y de que el comentario no se ha borrado
- **And** la herramienta NO envía ninguna petición con `force` `true` para borrarlo definitivamente

#### Scenario: Un 5xx no reintenta

- **Given** un servidor de prueba que responde la lectura con `200` y el borrado con `503`
- **When** se ejecuta `blog_comment_delete`
- **Then** el error informa de que el sitio no está disponible
- **And** el servidor de prueba recibe exactamente una lectura y un borrado
- **And** no se envía ninguna petición adicional

#### Scenario: Un segundo intento repite el mismo desenlace idempotente

- **Given** un comentario que ya está en la papelera y un servidor de prueba que responde `410` a cada borrado
- **When** se ejecuta `blog_comment_delete` dos veces
- **Then** las dos llamadas devuelven el mismo desenlace de éxito y ningún error
- **And** cada llamada envía una lectura y un borrado, nunca dos borrados
- **And** ninguna escritura de Valet registra estado del borrado

### Requirement: El borrado SHALL NOT llevar cota propia

El borrado SHALL NOT llevar ninguna cota propia: ni contador, ni ventana temporal, ni límite de borrados
por hora o por día, ni estado equivalente. El único freno SHALL ser la **confirmación explícita** del
usuario (`Permission::ExplicitApproval`), y el daño SHALL quedar acotado por hechos que ya impone el
sistema, no por un límite inventado en Valet:

- **El origen de los identificadores**: solo se puede borrar un comentario que el sitio haya
  **notificado**, porque los dos identificadores salen del `metadata` de una notificación y nunca se
  buscan ni se inventan. No se puede alcanzar un comentario arbitrario.
- **La tasa de notificación del sitio**: el sitio no anuncia más de 20 comentarios por hora, así que no
  puede haber más de 20 borrados legítimos en esa ventana sin que el usuario los apruebe uno a uno.
- **La reversibilidad**: el comentario queda en la **papelera** de WordPress, que lo conserva 30 días
  (`EMPTY_TRASH_DAYS`, que el repositorio de WordPress no redefine); nada se destruye.

Se ha considerado y **descartado** una cota propia: exigiría estado (una tabla o un contador) para un
freno que ya ejerce el usuario al aprobar cada borrado, y un límite por hora sería un número arbitrario
que podría impedir un borrado legítimo —por ejemplo, limpiar de una vez una ráfaga de spam notificada—.
Una cota así haría además que el resultado dependiera del historial de Valet y no del deseo del usuario,
que es lo contrario de lo que protege una operación destructiva.

#### Scenario: El borrado no declara ninguna cota

- **Given** la herramienta `blog_comment_delete` registrada
- **When** se inspecciona lo que la herramienta declara como freno
- **Then** el único freno es el permiso `ExplicitApproval`
- **And** no consulta ni escribe ningún contador, ventana temporal ni tabla de borrados

#### Scenario: Varios borrados seguidos no se frenan entre sí

- **Given** dos comentarios distintos notificados y aprobados por el usuario
- **When** se ejecutan los dos borrados uno tras otro
- **Then** los dos se mandan a la papelera
- **And** el segundo no se rechaza por ninguna cota propia de la herramienta
