## ADDED Requirements

### Requirement: `mastodon_token` SHALL ser un ajuste sensible con endpoints dedicados

El sistema SHALL almacenar `mastodon_token` en `settings` y SHALL excluirlo de `GET /api/settings` y de toda escritura a través de `PUT /api/settings`. La única puerta de escritura SHALL ser `PUT /api/mastodon/token`, que acepta `{ "token": string }`, devuelve `400` si el token está vacío o solo con espacios y `204` cuando se guarda. `mastodon_base_url` SHALL ser un ajuste no sensible, SHALL no tener valor por defecto y SHALL viajar en `PUT /api/settings`.

#### Scenario: El token no aparece en la lectura de ajustes
- **GIVEN** un `mastodon_token` guardado con valor no vacío
- **WHEN** se llama a `GET /api/settings`
- **THEN** la respuesta no contiene la clave `mastodon_token` ni su valor

#### Scenario: El token se ignora en la escritura genérica
- **GIVEN** un cuerpo `PUT /api/settings` que incluye `mastodon_token`
- **WHEN** se procesa la petición
- **THEN** la clave se ignora, no se persiste y las demás claves se guardan con normalidad

#### Scenario: Un token vacío se rechaza
- **GIVEN** `PUT /api/mastodon/token` con `{ "token": "   " }`
- **WHEN** se valida el cuerpo
- **THEN** se devuelve `400` y el ajuste existente no se modifica

#### Scenario: Un token válido se guarda
- **GIVEN** `PUT /api/mastodon/token` con `{ "token": "abcdef" }`
- **WHEN** se complete la escritura
- **THEN** se devuelve `204` y `mastodon_token` queda disponible para el servicio

#### Scenario: La URL base no tiene valor por defecto
- **GIVEN** una instalación sin `mastodon_base_url`
- **WHEN** se intenta cualquier operación de Mastodon
- **THEN** la operación falla con un detalle que pide configurar la URL base y no envía ninguna petición a una instancia por defecto

### Requirement: `GET /api/mastodon/check` SHALL sondear la conexión real sin revelar el token

El sistema SHALL exponer `GET /api/mastodon/check` con respuesta HTTP `200` constante y cuerpo `{ "ok": boolean, "detail": string }`. La sonda SHALL llamar a `GET {mastodon_base_url}/api/v1/notifications?limit=1&types[]=mention` con el token en cabecera `Authorization: Bearer`. El detalle SHALL ser accionable en caso de error y SHALL no contener ni el token ni su fragmento. `GET /api/mastodon/status` SHALL devolver `{ "configured": boolean, "base_url": string }` y nunca el token.

#### Scenario: Configuración completa y token válido
- **GIVEN** `mastodon_base_url` y `mastodon_token` configurados y un servidor que responde `200` a la consulta de notificaciones
- **WHEN** se llama a `GET /api/mastodon/check`
- **THEN** se devuelve `200` con `ok: true` y el detalle «conexión correcta»

#### Scenario: Token inválido
- **GIVEN** un servidor que responde `401` a la consulta de notificaciones
- **WHEN** se llama a `GET /api/mastodon/check`
- **THEN** se devuelve `200` con `ok: false` y un detalle que indica que el token no es válido

#### Scenario: Sin token configurado
- **GIVEN** un token vacío
- **WHEN** se llama a `GET /api/mastodon/check`
- **THEN** se devuelve `200` con `ok: false`, no se hace ninguna petición externa y el detalle no contiene secretos

#### Scenario: El estado informa sin revelar el token
- **GIVEN** un token guardado
- **WHEN** se llama a `GET /api/mastodon/status`
- **THEN** la respuesta contiene `configured`, `base_url` y ningún campo con el token

### Requirement: Una migración SHALL sembrar los ajustes, el prompt y la tabla de dedupe de Mastodon

La migración `20261128000001_mastodon.sql` SHALL ser aditiva e idempotente. SHALL sembrar `mastodon_base_url` y `mastodon_token` vacíos y `SKILL_MASTODON_PROMPT` solo si no existe ya un valor (guarda byte a byte: no SHALL pisar un prompt editado por el usuario). SHALL crear `mastodon_seen` con `notification_id TEXT PRIMARY KEY` y `created_at`, sin destruir datos existentes.

#### Scenario: Primera ejecución
- **GIVEN** una base de datos sin ajustes de Mastodon
- **WHEN** se ejecuta la migración
- **THEN** los ajustes existen vacíos, el prompt está sembrado y `mastodon_seen` existe

#### Scenario: Reejecución con prompt editado
- **GIVEN** un `SKILL_MASTODON_PROMPT` modificado por el usuario
- **WHEN** se ejecuta de nuevo la migración
- **THEN** el prompt conserva byte a byte el valor del usuario y la migración no falla
