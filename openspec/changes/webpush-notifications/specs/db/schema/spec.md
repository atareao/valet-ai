## ADDED Requirements

### Requirement: La migración SHALL crear la tabla `push_subscriptions` para las suscripciones WebPush

Una migración nueva SHALL crear, de forma idempotente, la tabla `push_subscriptions` con las columnas
exactas `id TEXT PRIMARY KEY`, `user_id TEXT NOT NULL DEFAULT 'default'`, `endpoint TEXT NOT NULL
UNIQUE`, `p256dh TEXT NOT NULL`, `auth TEXT NOT NULL`, `user_agent TEXT`, `created_at TEXT NOT NULL
DEFAULT (datetime('now'))` y `updated_at TEXT NOT NULL DEFAULT (datetime('now'))`. La restricción
`UNIQUE` sobre `endpoint` ya crea el índice, así que SHALL NO añadirse un índice redundante sobre
`endpoint`.

#### Scenario: La migración crea la tabla

- **Given** una base de datos vacía
- **When** se ejecuta `run_migrations()`
- **Then** existe la tabla `push_subscriptions` con las columnas indicadas
- **And** la migración es idempotente al ejecutarse de nuevo

#### Scenario: Un endpoint duplicado viola UNIQUE

- **Given** una fila existente en `push_subscriptions` con un `endpoint`
- **When** se intenta insertar otra fila con el mismo `endpoint`
- **Then** la base de datos rechaza la inserción por la restricción `UNIQUE`

#### Scenario: El user_id por defecto es 'default'

- **Given** una fila insertada sin especificar `user_id`
- **When** se consulta la fila
- **Then** su `user_id` es `'default'`

### Requirement: `reminders` SHALL registrar cuándo se notificó un recordatorio

La tabla `reminders` SHALL ganar una columna nueva `notified_at TEXT` nullable, donde `NULL` significa
«sin notificar todavía».

#### Scenario: La columna existe y es nullable

- **Given** una base de datos migrada
- **When** se inspeccionan las columnas de `reminders`
- **Then** existe `notified_at` de tipo `TEXT`
- **And** admite `NULL`

#### Scenario: Un recordatorio recién creado no está notificado

- **Given** un recordatorio recién creado sin `notified_at`
- **When** se consulta su `notified_at`
- **Then** su valor es `NULL`

### Requirement: La migración SHALL sembrar los ajustes de WebPush

La migración SHALL sembrar los ajustes `push_vapid_public_key` y `push_vapid_private_key` vacíos —se
generan al primer uso— y `push_reminder_grace_hours` con el valor `'24'`. La siembra SHALL ser
idempotente y SHALL NO sobrescribir un valor ya personalizado, con el mismo idioma que
`20261010000007_rss_skill.sql`.

#### Scenario: Tras migrar existen las tres claves con sus valores

- **Given** una base de datos vacía
- **When** se ejecuta `run_migrations()`
- **Then** `settings` contiene `push_vapid_public_key` y `push_vapid_private_key` vacías
- **And** contiene `push_reminder_grace_hours` con `'24'`

#### Scenario: Una segunda ejecución no sobrescribe un valor personalizado

- **Given** un `push_reminder_grace_hours` cambiado por el usuario
- **When** se vuelve a ejecutar la migración
- **Then** el valor del usuario se conserva
- **And** no se produce ningún error
