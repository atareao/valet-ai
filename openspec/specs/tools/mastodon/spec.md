# tools/mastodon Specification

## Purpose
TBD - created by archiving change mastodon-mentions. Update Purpose after archive.

## Requirements

### Requirement: La herramienta `mastodon` SHALL exponer `list_mentions` y `reply` con permisos distintos

El registro SHALL contener una herramienta `mastodon` con dos operaciones: `list_mentions` y `reply`. `list_mentions` SHALL tener permiso `NoConfirm`. `reply` SHALL tener permiso `ExplicitApproval`. Una operación ausente o desconocida SHALL devolver un error de argumentos SHALL no iniciar ninguna petición de red.

#### Scenario: Permiso de lectura
- **GIVEN** una llamada con `operation: "list_mentions"`
- **WHEN** se consulta el permiso de la herramienta
- **THEN** el permiso es `NoConfirm`

#### Scenario: Permiso de respuesta pública
- **GIVEN** una llamada con `operation: "reply"`
- **WHEN** se consulta el permiso de la herramienta
- **THEN** el permiso es `ExplicitApproval`

#### Scenario: Operación desconocida
- **GIVEN** una llamada con `operation: "boost"`
- **WHEN** se ejecuta la herramienta
- **THEN** se devuelve un error `InvalidArguments` y no se contacta con la instancia

### Requirement: `list_mentions` SHALL convertir el HTML a texto y crear avisos deduplicados

`list_mentions` SHALL aceptar `limit` entre 1 y 40 (por defecto 20) y `max_id` opcional, y SHALL llamar a `GET /api/v1/notifications?types[]=mention` de la instancia configurada. SHALL devolver para cada mención `notification_id`, `status_id`, `author`, `text`, `visibility` y `url`. El campo `text` SHALL derivar del HTML del status conservando el texto de los enlaces, convirtiendo `<br>` y cierres de bloque en saltos de línea y decodificando entidades HTML habituales, sin añadir dependencias. Por cada notificación SHALL intentar `INSERT OR IGNORE` en `mastodon_seen`; solo si se inserta SHALL crear un aviso con `NotificationsRepo::insert`, `origin="mastodon"`, `level="info"`, `author` igual al `acct` de la cuenta, `message` igual al texto plano y `metadata` con `notification_id`, `status_id` y `url`.

#### Scenario: Primera lectura de menciones
- **GIVEN** una instancia que devuelve dos notificaciones de tipo `mention` con contenido HTML
- **WHEN** se ejecuta `list_mentions` por primera vez
- **THEN** se devuelven dos menciones con texto sin etiquetas y se insertan dos avisos de origin `mastodon`

#### Scenario: Lectura repetida sin avisos duplicados
- **GIVEN** dos menciones ya procesadas por `list_mentions`
- **WHEN** se ejecuta de nuevo `list_mentions` con el mismo resultado
- **THEN** se devuelven las menciones pero no se crean avisos nuevos

#### Scenario: Enlaces y entidades
- **GIVEN** un status con `<p>Hola <a href="https://ejemplo.es">mundo</a> &amp; amigos<br/>fin</p>`
- **WHEN** se convierte el contenido a texto
- **THEN** el texto contiene «Hola mundo & amigos», un salto de línea antes de «fin» y ninguna etiqueta HTML

#### Scenario: Sin configuración
- **GIVEN** `mastodon_token` vacío
- **WHEN** se ejecuta `list_mentions`
- **THEN** se devuelve un error accionable que indica que hay que pegar el token en Ajustes y no se hace ninguna petición externa

### Requirement: `reply` SHALL publicar con aprobación explícita, idempotencia y visibilidad segura

`reply` SHALL aceptar `status_id`, `text` (no vacío), `visibility` (`public`, `unlisted`, `private` o `direct`; por defecto `unlisted`) y `spoiler_text` opcional. SHALL publicar `POST /api/v1/statuses` con `status`, `in_reply_to_id` y una cabecera `Idempotency-Key` con un UUID v4 nuevo en cada petición. Si `mastodon_seen` registra el `status_id` con visibilidad `direct`, la petición SHALL usar `direct` aunque se haya pedido otra visibilidad. Los errores SHALL mapearse a `401/403 → Unauthorized`, `404 → NotFound`, `422 → InvalidInput`, `429 → RateLimited` y `5xx` o fallo de red a `Upstream`.

#### Scenario: Respuesta aprobada
- **GIVEN** un `status_id` válido y texto no vacío con aprobación explícita
- **WHEN** se ejecuta `reply`
- **THEN** se envía `POST /api/v1/statuses` con `in_reply_to_id`, `status` y un `Idempotency-Key` único, y se devuelve la URL del status creado

#### Scenario: Visibilidad directa protegida
- **GIVEN** un `status_id` registrado como `direct` y una petición con `visibility: "public"`
- **WHEN** se ejecuta `reply`
- **THEN** la visibilidad enviada es `direct`

#### Scenario: Visibilidad inválida
- **GIVEN** `visibility: "global"`
- **WHEN** se valida la petición
- **THEN** se devuelve `InvalidArguments` sin contactar con la instancia

#### Scenario: Texto vacío
- **GIVEN** `text: "   "`
- **WHEN** se valida la petición
- **THEN** se devuelve `InvalidArguments` sin contactar con la instancia

#### Scenario: Error de límite de tasa
- **GIVEN** una instancia que responde `429`
- **WHEN** se ejecuta `reply`
- **THEN** se devuelve un error `RateLimited` con un detalle accionable
