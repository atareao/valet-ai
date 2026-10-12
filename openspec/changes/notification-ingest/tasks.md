# Tasks

Todas las casillas empiezan sin marcar. **No se escribe ni una línea de código ni un solo test antes de que el usuario apruebe este change.**

Este change está **sin aprobar**: ninguna casilla está marcada, ni siquiera la 0.1, porque el protocolo OpenSpec exige la aprobación explícita del usuario antes de tocar código.

## 0. Aprobación (punto de parada del protocolo)

- [x] 0.1 Presentar este change (proposal + specs de `notifications`, `ingest-keys`, `push`, `oidc-auth` y `frontend` + design + tasks) y **esperar aprobación explícita** antes de tocar código. Es el punto de parada obligatorio del protocolo: sin un «aprobado» del usuario no se avanza a la tarea 1.1. Ninguna de las casillas de las fases 1 a 9 se marca hasta que exista esa aprobación.

## 1. Migración `20261012000002_notification_ingest.sql`

Fichero nuevo: `migrations/20261012000002_notification_ingest.sql`. Aditiva e idempotente: `CREATE TABLE IF NOT EXISTS` para `notifications` e `ingest_keys` y `CREATE INDEX IF NOT EXISTS` para sus índices. No reconstruye ninguna tabla.

- [x] 1.1 RED — test de migración en `tests/db/notification_ingest_migration.rs`, con el mismo patrón que `tests/db/webpush_migration.rs` (pool `:memory:` con `valet::db::schema::run_migrations`, helper `columns()` sobre `pragma_table_info` y helper de lectura de SQL desde disco). Debe fallar porque la migración todavía no existe. Aserciones:
  - la tabla `notifications` existe y tiene exactamente las columnas `id` (`TEXT`, PK), `origin` (`TEXT` NOT NULL), `author` (`TEXT` NOT NULL), `message` (`TEXT` NOT NULL), `level` (`TEXT` NOT NULL, default `'info'`), `metadata` (`TEXT` nullable), `read_at` (`TEXT` nullable) y `created_at` (`TEXT` NOT NULL, default con `datetime`);
  - la tabla `ingest_keys` existe y tiene exactamente las columnas `id` (`TEXT`, PK), `name` (`TEXT` NOT NULL, UNIQUE), `key_hash` (`TEXT` NOT NULL, UNIQUE), `prefix` (`TEXT` NOT NULL), `created_at` (`TEXT` NOT NULL, default con `datetime`), `last_used_at` (`TEXT` nullable) y `revoked_at` (`TEXT` nullable);
  - existe el índice parcial `idx_notifications_unread` (`sqlite_master.type = 'index'`), declarado `ON notifications(created_at) WHERE read_at IS NULL`;
  - existe el índice `idx_notifications_origin` `ON notifications(origin, created_at)`;
  - la violación de `UNIQUE` en `ingest_keys.name` y en `ingest_keys.key_hash` es rechazada (segundo `INSERT` con el mismo valor devuelve error de `UNIQUE`);
  - tras migrar, `SELECT COUNT(*) FROM notifications` y `SELECT COUNT(*) FROM ingest_keys` son ambos `0`: las dos tablas nacen vacías.
  - Comando en rojo: `cargo test --test notification_ingest_migration`. **Pegar la salida en rojo en el informe.**
- [x] 1.2 GREEN — escribir `migrations/20261012000002_notification_ingest.sql` con las dos tablas y los dos índices (`CREATE TABLE IF NOT EXISTS` / `CREATE INDEX IF NOT EXISTS`), con las columnas, defaults, `NOT NULL` y `UNIQUE` de la tarea 1.1. Confirmar el test en verde con `cargo test --test notification_ingest_migration` y `cargo check` sin errores.
- [x] 1.3 GREEN — test de idempotencia a través del migrador (patrón `test_webpush_migration_is_idempotent_through_migrator`): ejecutar `run_migrations` dos veces sobre el mismo pool y aseverar que `notifications` e `ingest_keys` existen exactamente una vez y que los índices no se duplican. Comando: `cargo test --test notification_ingest_migration`.

## 2. Repositorios de dominio

### 2.1 `src/db/repos/notifications.rs`

Operaciones: `insert` (devuelve el `id`), `list_unread` (paginado, más recientes primero, con `total`) y `mark_read` (devuelve cuántas filas cambiaron, idempotente). Struct `Notification` con `id`, `origin`, `author`, `message`, `level`, `metadata` (JSON serializado o `None`), `read_at` y `created_at`.

- [x] 2.1.1 RED — tests unitarios en `#[cfg(test)]` dentro de `src/db/repos/notifications.rs` (patrón `src/db/repos/push.rs`), que deben fallar mientras el repo no exista. Aserciones:
  - `insert` con `origin`, `author` y `message` persiste la fila, devuelve un `id` no vacío y la fila recién leída tiene `read_at IS NULL`, `level = 'info'` y `created_at` no nulo (scenario «Una notificación nace sin leer»);
  - `insert` sin `metadata` deja `metadata` a NULL; `insert` con un objeto JSON lo deja serializado (scenario «`metadata` admite ausencia y objeto»);
  - `list_unread` solo devuelve las filas con `read_at IS NULL`, ordenadas por `created_at DESC`, con `limit` respetado y `total` igual al número de no leídas (scenario «Solo se listan las no leídas, más recientes primero»);
  - `mark_read` con dos ids no leídos devuelve `2` y deja ambos `read_at` distinto de NULL; repetirlo con los mismos ids devuelve `0` y no cambia los `read_at` (idempotencia); un id inexistente devuelve `0` sin error (scenarios de `mark-read`).
  - Comando en rojo: `cargo test --lib notifications_repo`. **Pegar la salida en rojo.**
- [x] 2.1.2 GREEN — implementar el repo con `sqlx` (`INSERT`, `SELECT ... WHERE read_at IS NULL ORDER BY created_at DESC LIMIT ?`, `SELECT COUNT(*) ... WHERE read_at IS NULL`, `UPDATE ... SET read_at = datetime('now') WHERE read_at IS NULL AND id IN (...)` devolviendo `rows_affected`). Confirmar `cargo test --lib notifications_repo` y `cargo check` en verde.

### 2.2 `src/db/repos/ingest_keys.rs`

Operaciones: alta guardando **solo** el SHA-256 y el prefijo, búsqueda por hash (actualizando `last_used_at`), listado sin token y revocación. Struct `IngestKey` con `id`, `name`, `key_hash`, `prefix`, `created_at`, `last_used_at` y `revoked_at`, con un `Debug` que nunca incluya un token (solo hay hash y prefijo, pero se asevera).

- [x] 2.2.1 RED — tests unitarios en `#[cfg(test)]` dentro de `src/db/repos/ingest_keys.rs`. Aserciones:
  - un alta persiste `key_hash` igual al SHA-256 en hexadecimal del token y `prefix` igual a sus 12 primeros caracteres; **ninguna** columna contiene el token completo (scenario «Una clave nueva persiste hash y prefijo, nunca el token»);
  - la búsqueda por el hash del token encuentra la clave y deja `last_used_at` distinto de NULL (scenario «Un token válido actualiza `last_used_at` y pasa», parte de repo); la búsqueda por un hash desconocido devuelve `None`; la búsqueda de una clave revocada devuelve `None` y no toca `last_used_at`;
  - el listado devuelve todas las claves (activas y revocadas) sin exponer el token (scenarios de `GET /api/ingest-keys` a nivel de repo);
  - la revocación fija `revoked_at` sin borrar la fila; repetirla no altera el `revoked_at` ya fijado; revocar un id inexistente no es error (scenarios de `DELETE /api/ingest-keys/{id}` a nivel de repo);
  - el `Debug` de `IngestKey` no contiene la cadena del token (scenario «La salida de depuración no filtra el token»).
  - Comando en rojo: `cargo test --lib ingest_keys_repo`. **Pegar la salida en rojo.**
- [x] 2.2.2 GREEN — implementar el repo: alta con `INSERT` de `id`, `name`, `key_hash` (SHA-256 hex), `prefix`; `find_by_hash` con `SELECT ... WHERE key_hash = ?` y `UPDATE ... SET last_used_at = datetime('now')` cuando la clave no está revocada; `list_all` (sin token); `revoke` con `UPDATE ... SET revoked_at = datetime('now') WHERE id = ? AND revoked_at IS NULL`. Confirmar `cargo test --lib ingest_keys_repo` y `cargo check` en verde.
- [x] 2.2.3 GREEN — registrar ambos módulos en `src/db/repos/mod.rs` (`pub mod notifications;` y `pub mod ingest_keys;`) y confirmar `cargo check` en verde.

## 3. Transporte: envío masivo y ayudante de host compartido

### 3.1 Envío masivo en `src/services/push.rs`

- [x] 3.1.1 RED — tests unitarios en `#[cfg(test)]` dentro de `src/services/push.rs` para la operación de envío masivo, con un `PushSender` falso (el `FakeSender` que ya existe en `src/workers/reminder_push.rs` sirve de modelo; se instrumenta aquí un doble equivalente). Aserciones:
  - con tres suscripciones cuyos envíos devuelven `Delivered`, `Gone` y `Failed`, los contadores son `attempted == 3`, `accepted == 1`, `pruned == 1` y `failed == 1` (scenario «Una mezcla de resultados cuenta cada suscripción por separado»);
  - con una única suscripción cuyo envío devuelve `Gone`, `attempted == 1`, `pruned == 1`, `failed == 0` y la suscripción se elimina de la base de datos (scenario «Una suscripción caducada se elimina y no cuenta como fallo»);
  - con cero suscripciones la operación **no** es un error y devuelve todos los contadores a cero (scenario «Sin suscripciones no es un error»);
  - un `Err(PushError)` cuenta como `attempted` y `failed`, y no cortocircuita el resto de la lista.
  - Comando en rojo: `cargo test --lib push`. **Pegar la salida en rojo.**
- [x] 3.1.2 GREEN — implementar en `src/services/push.rs` la operación de envío masivo (payload + `PushRepo::list_all`, bucle por suscripción con los cuatro contadores, `Gone` → eliminar suscripción y sumar a `pruned` sin contar como `failed`; struct de resultado con `attempted`, `accepted`, `failed`, `pruned`). Confirmar `cargo test --lib push` y `cargo check` en verde.

### 3.2 Traslado del ayudante de host a `src/services/push.rs`

- [x] 3.2.1 RED — test del ayudante puro de host, añadido al módulo de tests de `src/services/push.rs` (el contrato es el mismo que hoy cubre `endpoint_host_returns_host_of_a_normal_url` y `endpoint_host_marks_unparseable_values` en `src/workers/reminder_push.rs`): para `https://fcm.googleapis.com/fcm/send/abc` devuelve `fcm.googleapis.com`; para una cadena vacía o un valor sin host reconocible devuelve el marcador `(endpoint no parseable)` y **nunca** la cadena original. Comando en rojo: `cargo test --lib push`. **Pegar la salida en rojo.**
- [x] 3.2.2 GREEN — trasladar el ayudante `endpoint_host` desde `src/workers/reminder_push.rs` a `src/services/push.rs` (mismo cuerpo, mismo marcador `(endpoint no parseable)`), hacer que `reminder_push.rs` lo use desde `crate::services::push` en vez de su copia privada, y mantener los tests de `reminder_push.rs` que ya lo cubren. El comportamiento del worker debe ser **idéntico**: es un traslado, no un cambio. Confirmar `cargo test --lib push` y, muy especialmente, `cargo test --lib reminder_push` en verde (los tests del worker siguen pasando sin tocar sus aserciones).

## 4. Autenticación: middleware de ingesta y exención de sesión por par

### 4.1 Middleware `require_ingest_key`

- [x] 4.1.1 RED — tests de integración en `tests/api/` (arnés `TestApp` de `tests/api/common/mod.rs`), que deben fallar mientras el middleware no exista. Con una clave de ingesta sembrada en la base de datos (vía `IngestRepo` o `INSERT` directo con el hash del token), y `POST /api/notifications` con un cuerpo válido:
  - sin cabecera `Authorization` → `401` y `notifications` no gana ninguna fila (scenario «Sin cabecera de autorización no se ingesta»);
  - con cabecera `Authorization` sin el esquema `Bearer` o con un valor arbitrario → `401` (scenario «Un token malformado no ingesta»);
  - con un token desconocido → `401`;
  - con el token de una clave revocada → `401` (scenario «Una clave revocada no ingesta»);
  - con un token válido → la petición alcanza el handler y `last_used_at` de la clave deja de ser NULL (scenario «Un token válido actualiza `last_used_at` y pasa»);
  - tras autenticar, el middleware deja disponible para el handler la clave autenticada (`id` y `name`, p. ej. en las extensiones de la petición), porque el handler necesita el `name` para rellenar `origin` y `author` cuando faltan; el test debe comprobar esa disponibilidad: una ingesta con una clave llamada `monitor` y un cuerpo sin `origin`/`author` persiste `origin = "monitor"` y `author = "monitor"`, lo que solo puede ocurrir si el nombre llegó al handler. Además, el token **no** aparece registrado (scenario «Los registros de ingesta no contienen el token», verificable en la medida en que el repo no guarda el token).
  - Comando en rojo: `cargo test --test notifications`. **Pegar la salida en rojo.**
- [x] 4.1.2 GREEN — implementar el middleware `require_ingest_key` (Bearer: leer `Authorization`, exigir el esquema, buscar por el SHA-256 del token con `IngestRepo::find_by_hash`, rechazar `401` si falta/malformado/desconocido/revocado, actualizar `last_used_at` y dejar pasar; sin registrar el token en ningún log). Confirmar el test en verde con `cargo test --test notifications` y `cargo check`.

### 4.2 Exención de sesión por par (método, ruta)

- [x] 4.2.1 RED — tests de integración que describen la exención por par, con `AUTH_ENABLED=true` (patrón `app_with(config)` de `tests/api/auth.rs`, que inyecta `state.auth_config`):
  - `POST /api/notifications` sin cookie de sesión pero con token de ingesta válido **no** es rechazada por el middleware de sesión y alcanza el handler (scenario «La ingesta sin sesión pero con token válido alcanza el handler»);
  - `POST /api/notifications` sin token de ingesta válido responde `401` y no ejecuta el handler (scenario «La ingesta sin token responde 401»);
  - `GET /api/notifications/unread` sin sesión pero con un token de ingesta válido presente responde `401` (scenario «Las lecturas de notificaciones exigen sesión aunque haya token de ingesta»);
  - `POST /api/notifications/mark-read` sin sesión responde `401`.
  - Comando en rojo: `cargo test --test auth`. **Pegar la salida en rojo.**
- [x] 4.2.2 GREEN — modificar `src/middleware/auth.rs`: sustituir la exención por prefijo de ruta por una función de exención por **par (método, ruta)** que deje pasar `POST /api/notifications` (protegido por el token de ingesta) y **no** exima `GET /api/notifications/unread` ni `POST /api/notifications/mark-read`; conservar las exenciones de `/api/health` y `/api/auth/*`. Usar un comparador de método explícito (no solo la ruta). Confirmar `cargo test --test auth` y `cargo check` en verde.

## 5. Rutas de ingesta y consumo

Fichero nuevo: `src/routes/notifications.rs`, siguiendo el estilo de `src/routes/push.rs` (docstrings, `AppError`, traducción del resultado a HTTP) y registrado en `src/routes/mod.rs` y en el `Router` de `app_with_state` (`src/lib.rs`).

- [x] 5.1 RED — tests de integración en `tests/api/notifications.rs` (arnés `TestApp`). Aserciones:
  - `POST /api/notifications` con token válido y cuerpo válido responde `201` con `{"id": "<uuid>"}` (scenario «Una clave válida ingesta con 201»);
  - `message` ausente o en blanco → `400` con `{"error": "..."}` (scenario «Un mensaje ausente o en blanco se rechaza»);
  - `origin` y `author` ausentes toman el **nombre de la clave** (con una clave llamada `monitor`, la fila persistida tiene `origin = "monitor"` y `author = "monitor"`) (scenario «`origin` y `author` ausentes toman el nombre de la clave»);
  - el `message` persistido no incluye espacios al principio ni al final (scenario «Se recorta el mensaje antes de persistir»);
  - `level = "debug"` → `400`; `level = "critical"` → `201` y persiste `critical` (scenario «Un `level` fuera de la lista blanca se rechaza»);
  - `metadata` igual a un arreglo JSON → `400`; igual a un objeto JSON → `201` (scenario «`metadata` debe ser objeto»);
  - `message` de más de 4096 bytes → `400`; cuerpo HTTP de más de 64 KiB → `413` (scenario «Se rechazan los cuerpos que exceden los límites»);
  - `GET /api/notifications/unread`: solo las no leídas, más recientes primero, con `data`, `next_cursor` y `total` (`PaginatedResponse`); `metadata` parseado a objeto o `null`; `limit` por defecto 50, máximo 200; `limit=0`, `limit=201` o `limit=abc` → `400`; sin sesión → `401` (scenarios del requirement de `unread`);
  - `POST /api/notifications/mark-read`: `{"marked": 2}` con dos no leídas; repetir → `{"marked": 0}` y los `read_at` no cambian; id inexistente → `{"marked": 0}`; `ids` ausente o `[]` → `400`; sin sesión → `401` (scenarios del requirement de `mark-read`).
  - Comando en rojo: `cargo test --test notifications`. **Pegar la salida en rojo.**
- [x] 5.2 GREEN — implementar `src/routes/notifications.rs`: `POST /api/notifications` con validación y normalización (trim, longitudes `message ≤ 4096`, `origin ≤ 64`, `author ≤ 128`, lista blanca de `level`, `metadata` objeto ≤ 8192 bytes serializado, defaults `origin`/`author` desde el nombre de la clave), límite de cuerpo 64 KiB (`413` si lo excede) y `201` con el id tras el `INSERT`; despacho del WebPush en segundo plano (payload con título `[<origin>] <author>`, cuerpo el mensaje, `tag` `notification-<id>`, icono `/icon-192.png` y `url` `/`) tras el insert, best-effort. `GET /api/notifications/unread` y `POST /api/notifications/mark-read` con sesión, delegando en `NotificationsRepo`. Confirmar `cargo test --test notifications` y `cargo check` en verde.

## 6. Rutas de gestión de claves

Fichero nuevo: `src/routes/ingest_keys.rs`, con sesión OIDC, registrado en `src/routes/mod.rs` y en el `Router` de `app_with_state`.

- [ ] 6.1 RED — tests de integración en `tests/api/ingest_keys.rs` (arnés `TestApp`). Aserciones:
  - `POST /api/ingest-keys` con `{"name": "<nuevo>"}` responde `201` e incluye `id`, `name`, `prefix` y `token` (scenario «Crear una clave devuelve su token una vez»); el `token` empieza por `val_` y el resto es base64url sin padding (scenario «El token tiene el prefijo y la forma esperados»); el `prefix` son los 12 primeros caracteres del token (scenario «El prefijo son los 12 primeros caracteres»);
  - `name` ausente o en blanco → `400`; `name` de más de 64 bytes → `400`; `name` duplicado → `409` (scenarios de `POST /api/ingest-keys`);
  - `GET /api/ingest-keys` sin claves → `200` con lista vacía; con una activa y una revocada → `200` con una entrada por clave, cada una con `id`, `name`, `prefix`, `created_at`, `last_used_at` y `revoked_at`, y **ninguna** entrada con token (scenarios de `GET` y «El listado no filtra el token» / «El token no se relee después»);
  - `DELETE /api/ingest-keys/{id}` → `204`, la fila sigue con `revoked_at` distinto de NULL; repetir → `204` y `revoked_at` no cambia; id inexistente → `204` (scenarios de `DELETE`).
  - Comando en rojo: `cargo test --test ingest_keys`. **Pegar la salida en rojo.**
- [ ] 6.2 GREEN — implementar `src/routes/ingest_keys.rs` y el generador del token (`val_` + 32 bytes aleatorios en base64url sin padding), delegando en `IngestRepo`; comprobar explícitamente que el token **no** aparece en el listado ni en la respuesta del `GET`. Confirmar `cargo test --test ingest_keys` y `cargo check` en verde.

## 7. Herramienta del agente `notifications`

- [x] 7.1 RED — test que falle con la tool sin catalogar: en `src/tools/notifications.rs`, tests que aseveran que la herramienta `notifications` figura en las definiciones con las operaciones `list_unread` y `mark_read` (scenario «La herramienta figura en las definiciones»), que su permiso es `NoConfirm` (scenario «El permiso es `NoConfirm`»), que `list_unread` devuelve las mismas no leídas que el endpoint y no devuelve las leídas (scenario «`list_unread` coincide con el endpoint»), y que `mark_read` es idempotente (primera invocación marca 2, la repetición marca 0) (scenario «`mark_read` es idempotente también para el agente»). Además, el test de integridad de skills (`every_registered_tool_is_covered_by_core_or_a_skill`, en `src/orchestrator/skills.rs`) debe **fallar** con la tool registrada pero sin catalogar, y `catalog_plus_core_covers_all_twenty_eight_tools_without_orphans` debe fallar con el recuento del catálogo desactualizado. Comando en rojo: `cargo test --lib notifications` y `cargo test --lib skills`. **Pegar la salida en rojo.**
- [x] 7.2 GREEN — implementar `src/tools/notifications.rs` (struct `NotificationsTool`, operaciones `list_unread` y `mark_read` delegando en `NotificationsRepo`, `permission` → `Permission::NoConfirm`, descripciones en español siguiendo `src/tools/reminders.rs`); añadir `pub mod notifications;` a `src/tools/mod.rs`; registrar la tool en `build_tool_registry` (`src/lib.rs`, con `pool.clone()`); añadir `"notifications"` a las `tools` de la skill `pendientes` en `src/orchestrator/skills.rs`. Actualizar el recuento de herramientas que fijan los tests: `catalog_plus_core_covers_all_twenty_eight_tools_without_orphans` pasa a `29` (y renombrar su identificador en consecuencia) y el comentario de `build_tool_registry` («all 28 built-in tools»). **Antes de fijar el recuento, ejecutar `grep -rn "28" src/` para localizar cada sitio** (ver la nota de contradicción sobre `src/bin/route-eval.rs` en `## Notas`). Confirmar `cargo test --lib notifications`, `cargo test --lib skills` y `cargo check` en verde.
- [x] 7.3 GREEN — que la tool no quede muda: (a) extender las `instructions` y las `criteria` de la skill `pendientes` en `src/orchestrator/skills.rs` para que cubran también los avisos y mensajes **entrantes** de otras aplicaciones (formularios, monitorización, scripts), sin sacar del `criteria_true` el saludo de apertura del día ni los recordatorios (los exige `openspec/specs/orchestrator/skill-router/spec.md`, requisito `Las criteria por defecto del briefing SHALL reconocer el saludo de apertura del día`) y dejando `criteria_false` coherente; (b) crear la migración `migrations/20261012000003_pendientes_notifications_prompt.sql` que añade una línea de guía sobre `notifications` (`list_unread` / `mark_read`) al fragmento `SKILL_PENDIENTES_PROMPT`, sin reescribir `20261008000001` y sin pisar una personalización del usuario (solo actualiza el valor todavía sembrado; rellena ausente/vacío/NULL), y su test `tests/db/pendientes_prompt_migration.rs` con su `[[test]]` en `Cargo.toml`. Confirmar el test en verde y el fragmento empezando por `# SKILL ACTIVA: PENDIENTES`.

## 8. Frontend: sección de claves de ingesta en la pestaña API Keys

- [x] 8.1 RED — tests de componente con `@testing-library/react` en `frontend/src/components/` (convención `*.test.tsx`, `user-event` para la interacción). Aserciones:
  - crear una clave con un nombre muestra el token recién creado y un aviso explícito de que no se volverá a mostrar (scenario «Crear una clave muestra el token una sola vez con el aviso»);
  - la lista de claves muestra el prefijo de una clave creada y **no** muestra su token (scenario «La lista no contiene el token»);
  - revocar una clave hace que la lista la marque como revocada (scenario «Revocar marca la clave como revocada»);
  - un fallo del backend al crear se comunica con un aviso (`App.useApp()`, capability `ui-feedback`) y **no** deja la clave a medias en la lista (scenario «Un fallo del backend al crear se comunica sin dejar la clave a medias»).
  - Comando en rojo: `npx vitest run frontend/src/components/`. **Pegar la salida en rojo.**
- [x] 8.2 GREEN — añadir el cliente de API en `frontend/src/api/` (crear, listar y revocar claves de ingesta contra `/api/ingest-keys`) y la sección de claves **entrantes** en la pestaña **API Keys** del `SettingsDialog.tsx`, separada de las claves salientes que ya existen (OpenWeatherMap, Google Places, Brave, apimail y Miniflux): formulario para crear con nombre, token mostrado una sola vez con aviso, lista sin token y revocación por clave. Sin pestaña nueva de primer nivel. Los avisos, con `App.useApp()`, no con la API estática de antd. Confirmar `npx vitest run`, `npx tsc --noEmit` y `npm run lint` en verde.

## 9. Verificación

- [ ] 9.1 Suite de Rust completa en verde: `cargo test --no-fail-fast` (0 fallos), con el recuento de tests anotado **antes** y **después** del change.
- [ ] 9.2 `cargo clippy --all-targets -- -D warnings` limpio (0 warnings).
- [ ] 9.3 `cargo fmt --check` limpio.
- [ ] 9.4 Frontend: `npx vitest run` en verde, `npx tsc --noEmit` sin errores y `npm run lint` limpio.
- [ ] 9.5 `openspec validate --all` sin fallos.
- [ ] 9.6 Comprobación explícita de que **ninguna línea de log registra el token de ingesta ni la URL completa de un endpoint**: revisar (grep) los `tracing::*` de `src/middleware/` (ingesta), `src/routes/notifications.rs`, `src/routes/ingest_keys.rs`, `src/services/push.rs` y `src/workers/reminder_push.rs`, y confirmar que solo aparecen el `id`/`name` de la clave y el `host` del endpoint (o el marcador `(endpoint no parseable)`).

## Notas

- **Sin página de bandeja en el frontend:** el push abre la raíz de la app (`url` es `/`); una vista de bandeja es otro change.
- **Sin notificaciones salientes y sin rate limiting:** son no-goals, con los riesgos residuales declarados en `design.md` (un token filtrado puede inundar la tabla; la tabla crece sin poda).
- **El despacho del push es best-effort:** un fallo de envío no cambia el `201` y el mensaje ya está persistido. Es el mismo hueco *at-least-once* que el worker de recordatorios.
- **`POST /api/notifications` no acepta sesión:** es para máquinas, y es deliberado. El frontend no puede usar ese endpoint y no debe intentarlo.
- **No se toca `docker-compose.prod.yml`:** el despliegue es una orden aparte.
- **El contenido exacto de las líneas de log no se asevera en tests:** el repo no tiene arnés de captura de `tracing` (riesgo residual 4 de `design.md`). La verificación 9.6 es una revisión por `grep`, no un test.
- **Contradicción con el enunciado detectada durante la redacción:** el enunciado pide «actualizar el recuento de herramientas que fijen los tests (28 → 29) — revisar `grep -rn "28" src/orchestrator/skills.rs src/bin/route-eval.rs`». La revisión real del repo muestra que **`src/bin/route-eval.rs` no contiene ningún `28`** (consume `build_tool_registry` y no fija un recuento literal); el único `28` está en `src/orchestrator/skills.rs:669` (`catalog_plus_core_covers_all_twenty_eight_tools_without_orphans`) y, además, en el docstring de `build_tool_registry` (`src/lib.rs`: «all 28 built-in tools»). La tarea 7.2 fija esos dos sitios; el enunciado de `route-eval.rs` se ignora por no corresponderse con el repo.
- **Detalle añadido por mí (no estaba en el enunciado):** el registro de los dos módulos nuevos en `src/db/repos/mod.rs` (tarea 2.2.3) y de la tool en `src/tools/mod.rs` (tarea 7.2); el test de idempotencia del migrador (tarea 1.3); la creación del fichero de test de migración como `tests/db/notification_ingest_migration.rs`; y la verificación de que `POST /api/notifications/mark-read` sin sesión responde `401` (tarea 4.2.1), implícita en la spec de `oidc-auth` pero no listada como escenario propio.
- **Cierre de la fase 7 (tarea 7.3), añadido tras la revisión del director:** la 7.1 y la 7.2 dejaron la herramienta **registrada y catalogada pero muda**. Dos huecos no previstos en el plan la dejaban sin ruta y sin guía: (a) las `criteria` del router de la skill `pendientes` no mencionaban los avisos entrantes, así que un mensaje como «¿ha llegado algún aviso del formulario de contacto?» podía no activar la skill y el agente nunca vería la tool; (b) el fragmento `SKILL_PENDIENTES_PROMPT`, sembrado por migración, hablaba solo de `tasks`/`reminders`/`calendar`, de modo que el agente desconocía cuándo usar `notifications`. Ambos son necesarios y de bajo riesgo: el router solo gana cobertura y el fragmento una línea más, sin cambiar el número de skills ni el de herramientas.
