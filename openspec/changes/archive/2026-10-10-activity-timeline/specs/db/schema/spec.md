## ADDED Requirements

### Requirement: La migración SHALL crear la tabla `timeline_events` con sus índices

Una migración nueva SHALL crear, de forma idempotente, la tabla `timeline_events` con las columnas
`id` (clave primaria), `timestamp` (ISO 8601 UTC), `category` (con un conjunto cerrado garantizado por
`CHECK`), `fact`, `source_message_id` (clave ajena a `messages(id)` con `ON DELETE SET NULL`) y
`created_at`. SHALL crear además un índice por `timestamp` descendente, un índice por `category` y un
índice parcial por `source_message_id`.

**Given** una base de datos vacía
**When** se ejecuta la migración de `timeline_events`
**Then** existe la tabla `timeline_events` con las columnas indicadas
**And** existen los índices por fecha, por categoría y por mensaje de origen

#### Scenario: La tabla se crea con su CHECK de categoría
- **Given** la migración aplicada
- **When** se inspecciona el esquema de `timeline_events`
- **Then** la columna `category` admite `sport`, `lifestyle`, `work`, `shopping`, `health`, `social` y `system`
- **And** un valor fuera de ese conjunto lo rechaza la base de datos

#### Scenario: La clave ajena se anula al borrarse el mensaje
- **Given** un hecho cuyo mensaje de origen se borra
- **When** se consulta el hecho
- **Then** la fila sigue existiendo
- **And** su `source_message_id` queda a `NULL`

#### Scenario: La migración es idempotente
- **Given** la tabla `timeline_events` ya creada
- **When** se ejecuta la migración de nuevo
- **Then** no hay error
- **And** la tabla sigue existiendo con el mismo esquema

### Requirement: La migración SHALL ampliar los valores admitidos de `llm_requests.kind` sin perder filas

Una migración nueva SHALL ampliar el `CHECK` de `llm_requests.kind` para admitir `'timeline'`, además
de `'chat'`, `'router'`, `'archivist'`, `'consolidator'` y `'collapse'`. Como SQLite no permite
alterar un `CHECK` en línea, la migración SHALL reconstruir la tabla y SHALL preservar **todas** las
filas existentes, sea cual sea su `kind`.

#### Scenario: La tabla acepta el nuevo origen
- **Given** la migración aplicada
- **When** se inserta una fila de `llm_requests` con `kind = 'timeline'`
- **Then** la inserción se acepta

#### Scenario: Las filas previas sobreviven
- **Given** una base de datos con filas de los cinco valores anteriores de `kind`
- **When** se ejecuta la migración
- **Then** todas las filas siguen ahí
- **And** sus valores de `kind` no cambian

#### Scenario: Un valor fuera del conjunto sigue rechazándose
- **Given** la migración aplicada
- **When** se intenta insertar una fila con un `kind` desconocido
- **Then** la base de datos lo rechaza

#### Scenario: La migración es idempotente
- **Given** la migración ya aplicada
- **When** se ejecuta de nuevo
- **Then** no hay error
- **And** no se duplica ninguna fila
