# tools/email Specification

## Purpose
Las cuatro herramientas del correo de Valet —`email_list_unread`, `email_get_body`,
`email_mark_read` y `email_send`—: leen los mensajes no leídos de `INBOX`, traen el cuerpo de un
mensaje sin marcarlo como leído, marcan un mensaje como leído y envían correo —incluida la
respuesta a un mensaje, resolviendo el destinatario y el asunto a partir de su `uid`— contra la
API de apimail, con aprobación explícita para el envío.

## Requirements

### Requirement: `email_list_unread` SHALL listar los mensajes no leídos del buzón INBOX

La herramienta `email_list_unread` SHALL consultar `GET /api/messages?mailbox=INBOX&unseen=true` del servicio apimail y SHALL devolver el `total` de no leídos junto a los mensajes de la página, cada uno con su `uid`, su remitente legible, su asunto y su fecha. El parámetro `limit` SHALL ser opcional, SHALL estar acotado al rango que admite la API (`1..=200`) y SHALL valer `50` cuando se omita. Un `limit` fuera de rango SHALL devolver `Err(ToolError::InvalidArguments)` sin salir a la red. La herramienta SHALL declarar `Permission::NoConfirm` y SHALL NO alterar ninguna bandera del buzón.

#### Scenario: Un listado devuelve los no leídos con su remitente y su asunto
- **Given** un buzón INBOX con mensajes no leídos
- **When** se llama a `email_list_unread`
- **Then** se devuelve el total de no leídos
- **And** cada mensaje trae su `uid`, el remitente, el asunto y la fecha
- **And** ninguna bandera del buzón cambia

#### Scenario: Un limit fuera de rango es un argumento inválido
- **Given** una llamada con `limit` igual a `0` o a `500`
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición al servicio

### Requirement: `email_get_body` SHALL devolver el cuerpo de un mensaje sin marcarlo como leído

La herramienta `email_get_body` SHALL consultar `GET /api/messages/{uid}/body?mailbox=INBOX` y SHALL devolver el texto plano y el HTML del mensaje (`null` cuando la API no los derive). El parámetro `uid` SHALL ser obligatorio y SHALL ser un entero positivo; un `uid` ausente o no positivo SHALL devolver `Err(ToolError::InvalidArguments)` sin salir a la red. La herramienta SHALL declarar `Permission::NoConfirm`. El HTML devuelto SHALL tratarse como contenido de terceros no confiable.

#### Scenario: El cuerpo trae el texto plano y el HTML
- **Given** un mensaje existente con cuerpo en texto plano y en HTML
- **When** se llama a `email_get_body` con su `uid`
- **Then** se devuelven ambos cuerpos
- **And** no se fija la bandera `\Seen`

#### Scenario: Un uid ausente es un argumento inválido
- **Given** una llamada sin `uid` o con un `uid` igual a `0`
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición al servicio

### Requirement: `email_mark_read` SHALL marcar un mensaje como leído de forma idempotente

La herramienta `email_mark_read` SHALL consultar `PATCH /api/messages/{uid}/flags?mailbox=INBOX` con el cuerpo `{"add":["\\Seen"]}` y SHALL devolver las banderas resultantes. El parámetro `uid` SHALL ser obligatorio y SHALL ser un entero positivo; un `uid` ausente o no positivo SHALL devolver `Err(ToolError::InvalidArguments)` sin salir a la red. La herramienta SHALL declarar `Permission::NoConfirm`. Marcar un mensaje ya leído SHALL NO ser un error.

#### Scenario: Marcar como leído fija la bandera
- **Given** un mensaje no leído
- **When** se llama a `email_mark_read` con su `uid`
- **Then** la petición lleva `{"add":["\\Seen"]}`
- **And** se devuelven las banderas resultantes

#### Scenario: Marcar dos veces no es un error
- **Given** un mensaje ya marcado como leído
- **When** se vuelve a llamar a `email_mark_read` con su `uid`
- **Then** el resultado es correcto

### Requirement: `email_send` SHALL enviar un correo y SHALL poder responder a un mensaje por su uid

La herramienta `email_send` SHALL enviar correo por `POST /api/messages` del servicio apimail y SHALL exigir `Permission::ExplicitApproval`, porque enviar es irreversible y sale del sistema. Sus parámetros SHALL ser `text` (obligatorio, el cuerpo en texto plano), `to` (opcional, lista de direcciones), `subject` (opcional) y `reply_to_uid` (opcional). Cuando se informe `reply_to_uid`, la herramienta SHALL resolver el destinatario y el asunto sola: SHALL consultar `GET /api/messages/{uid}?format=summary`, SHALL tomar como destinatario la dirección del remitente del envelope y SHALL componer el asunto como `Re: <asunto original>` sin duplicar el prefijo si ya lo lleva; en ese caso `to` y `subject` SHALL NOT ser obligatorios. Sin `reply_to_uid`, SHALL exigirse `to` y `subject`. Un `text` ausente o en blanco, o una llamada sin `reply_to_uid` ni `to`, SHALL devolver `Err(ToolError::InvalidArguments)` sin salir a la red. El remitente (`from`) SHALL NOT exponerse como parámetro: lo resuelve apimail con el usuario SMTP configurado. Los adjuntos SHALL NOT soportarse.

#### Scenario: Enviar un correo suelto
- **Given** una llamada con `to`, `subject` y `text`
- **When** se ejecuta y el usuario la aprueba
- **Then** la petición lleva esos tres campos
- **And** el resultado confirma el envío

#### Scenario: Responder resolviendo destinatario y asunto del original
- **Given** un mensaje original del remitente `ana@example.com` con asunto `Presupuesto`
- **When** se llama a `email_send` con ese `reply_to_uid` y un `text`
- **Then** la petición lleva `to` con `ana@example.com`
- **And** el asunto es `Re: Presupuesto`
- **And** no hizo falta aportar ni destinatario ni asunto

#### Scenario: Un asunto que ya empieza por Re no se duplica
- **Given** un mensaje original cuyo asunto es `Re: Presupuesto`
- **When** se responde con su `reply_to_uid`
- **Then** el asunto enviado es `Re: Presupuesto`, no `Re: Re: Presupuesto`

#### Scenario: Sin destinatario ni original es un argumento inválido
- **Given** una llamada sin `reply_to_uid` y sin `to`
- **When** se ejecuta
- **Then** devuelve `Err(ToolError::InvalidArguments)`
- **And** no se envía ninguna petición al servicio

#### Scenario: El envío exige aprobación explícita
- **Given** la definición de `email_send`
- **When** se consulta su permiso
- **Then** es `Permission::ExplicitApproval`

#### Scenario: Un fallo de entrega se traduce a mensaje accionable
- **Given** un servicio que responde `502 smtp_error`
- **When** se ejecuta `email_send`
- **Then** el error indica que el envío no pudo entregarse

### Requirement: La configuración de apimail SHALL resolverse de settings, del entorno y de un valor por defecto

La URL base y la API key de apimail SHALL resolverse con esta precedencia: el ajuste de `settings` (`apimail_base_url` / `apimail_api_key`) si tiene contenido, la variable de entorno (`APIMAIL_BASE_URL` / `APIMAIL_API_KEY`) en su defecto y, solo para la URL, el valor por defecto `https://apimail.territoriolinux.es`. Las claves SHALL leerse en cada llamada, de modo que editarlas surta efecto sin reiniciar. Sin API key SHALL NO intentarse ninguna petición de red: las cuatro herramientas SHALL devolver un error accionable que indique dónde configurarla.

#### Scenario: El ajuste manda sobre el entorno
- **Given** `apimail_api_key` en `settings` y `APIMAIL_API_KEY` en el entorno
- **When** se ejecuta una de las herramientas
- **Then** la clave usada es la de `settings`

#### Scenario: Sin ajuste se usa el entorno
- **Given** `apimail_base_url` vacío en `settings` y `APIMAIL_BASE_URL` en el entorno
- **When** se ejecuta una de las herramientas
- **Then** se usa la URL del entorno

#### Scenario: Sin nada configurado la URL cae al valor por defecto
- **Given** ni el ajuste ni la variable de entorno de la URL
- **When** se ejecuta una de las herramientas
- **Then** se usa `https://apimail.territoriolinux.es`

#### Scenario: Sin API key no se sale a la red
- **Given** ni el ajuste ni la variable de entorno de la API key
- **When** se llama a cualquiera de las cuatro herramientas
- **Then** devuelve un error accionable
- **And** no se envía ninguna petición al servicio

### Requirement: Las cuatro herramientas del correo SHALL describirse en español y traducir los errores de apimail

Las descripciones de `email_list_unread`, `email_get_body`, `email_mark_read` y `email_send` SHALL estar redactadas en español, SHALL explicar qué devuelve cada una y SHALL enumerar sus argumentos obligatorios. Las cuatro SHALL exponer un esquema de parámetros de tipo objeto. La credencial SHALL viajar en la cabecera `Authorization: Bearer <clave>` y SHALL NO aparecer nunca en la descripción ni en el resultado. Los códigos de error de apimail SHALL traducirse a mensajes accionables: `401` a credenciales inválidas, `404 message_not_found` a que el mensaje no existe, `502 smtp_error` a que el envío no pudo entregarse, `503 imap_unavailable` a que el servidor de correo no está disponible, y un fallo de red o un `5xx` a un error de ejecución.

#### Scenario: Las descripciones están en español y citan los obligatorios
- **Given** las definiciones de las cuatro herramientas
- **When** se inspeccionan sus descripciones
- **Then** están en español
- **And** cada una menciona sus argumentos obligatorios

#### Scenario: La credencial no se expone
- **Given** una llamada con la API key configurada
- **When** se inspecciona la definición y el resultado de la herramienta
- **Then** la clave no aparece en ninguno de los dos

#### Scenario: Un 401 se traduce a credenciales inválidas
- **Given** un servicio que responde `401 unauthorized`
- **When** se ejecuta una de las herramientas
- **Then** devuelve un error que indica que la API key no es válida

#### Scenario: Un 404 de mensaje se traduce a que no existe
- **Given** un servicio que responde `404 message_not_found`
- **When** se ejecuta `email_get_body`
- **Then** el error indica que el mensaje no existe

#### Scenario: Un 503 se traduce a servidor no disponible
- **Given** un servicio que responde `503 imap_unavailable`
- **When** se ejecuta `email_mark_read`
- **Then** el error indica que el servidor de correo no está disponible
