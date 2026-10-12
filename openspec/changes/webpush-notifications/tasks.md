# Tasks

Todas las casillas empiezan sin marcar. **No se escribe ni una línea de código ni un solo test antes de que el usuario apruebe este change.**

## 0. Aprobación, migración y repositorios (test primero)

- [x] 0.1 Presentar este change (proposal + specs + design + tasks) y **esperar aprobación explícita** antes de tocar código. Es el punto de parada obligatorio del protocolo OpenSpec: sin un «aprobado» del usuario, no se avanza a la tarea 0.2.
- [x] 0.2 RED — test que falla de la migración (`tests/db/webpush_migration.rs`): tras migrar existen la tabla `push_subscriptions` con sus columnas y `UNIQUE(endpoint)`, la columna `reminders.notified_at` nullable, y las claves `push_vapid_public_key`/`push_vapid_private_key` vacías y `push_reminder_grace_hours` con `'24'`; un valor previo del usuario **no se sobrescribe**; volver a aplicar la migración es **idempotente**. Registrar el target `[[test]]` `name = "webpush-migration"` / `path = "tests/db/webpush_migration.rs"` en `Cargo.toml`. Verificar en rojo con `cargo test --test webpush-migration`.
- [x] 0.3 GREEN — implementar `migrations/20261012000001_webpush_notifications.sql` con el idioma idempotente de `20261010000007_rss_skill.sql` (`INSERT ... ON CONFLICT(key) DO UPDATE ... WHERE settings.value = '' OR settings.value IS NULL`), sin reconstruir ninguna tabla. Confirmar `cargo test --test webpush-migration` en verde.
- [x] 0.4 RED — test que falla de `PushRepo` (`src/db/repos/push.rs`): el *upsert* por `endpoint` conserva el `id` original y actualiza `p256dh`, `auth`, `user_agent` y `updated_at`; el borrado por `endpoint` es idempotente (exista o no la fila); el listado devuelve todas las suscripciones. Verificar en rojo con `cargo test push_repo`.
- [x] 0.5 GREEN — implementar `PushRepo` como mínimo para satisfacer los tests. Confirmar `cargo test push_repo` y `cargo check` en verde.
- [ ] 0.6 RED — test que falla de `RemindersRepo`: marcar `notified_at` de un recordatorio por su `id`. Verificar en rojo con `cargo test reminders`.
- [ ] 0.7 GREEN — implementar el marcado de `notified_at` en `src/db/repos/reminders.rs` y añadir `push_vapid_private_key` a `SENSITIVE_KEYS` en `src/db/repos/settings.rs`. Confirmar `cargo test reminders` en verde.

## 1. Servicio de push y claves VAPID

- [ ] 1.1 RED — tests que fallan de la generación y persistencia de VAPID: con las claves ausentes o vacías se generan y se persisten; con las claves presentes se reutiliza el par (misma clave pública); dos generaciones concurrentes convergen en un único par. Verificar en rojo con `cargo test vapid`.
- [ ] 1.2 GREEN — implementar la generación y persistencia transaccional (`INSERT ... ON CONFLICT DO NOTHING` + `SELECT`) en `src/services/push.rs`. Confirmar `cargo test vapid` en verde.
- [ ] 1.3 RED — test de contrato que falla del formato de la petición (sin red): método `POST` al `endpoint`; cuerpo con `salt` (16 B), `rs` (4 B big-endian), `idlen` (1 B = 65) y clave efímera (65 B, primer byte `0x04`); cabeceras `TTL`, `Content-Encoding: aes128gcm`, `Content-Length` y `Authorization` empezando por `vapid t=`. Verificar en rojo con `cargo test push_request`.
- [ ] 1.4 GREEN — construir la petición con `web-push-native` y enviarla con el `reqwest::Client` existente. Comprobar con `cargo tree | grep -Ei 'openssl|native-tls|isahc'` que **no** se ha introducido un segundo stack TLS. Confirmar `cargo test push_request` y `cargo check` en verde.

## 2. Rutas y validación

- [x] 2.1 RED — test que falla de `GET /api/push/vapid-key`: `200` con `{"public_key": "..."}` no vacía; fallo de carga/generación → `503` con error `push_unavailable` y sin la clave privada en el cuerpo. Verificar en rojo con `cargo test vapid_key`.
- [x] 2.2 RED — tests que fallan de `POST /api/push/subscriptions`: válida → `201` con `id` UUIDv4 y fila nueva; misma `endpoint` dos veces → mismo `id` y una sola fila; `endpoint` no-https → `400`; `p256dh` de longitud incorrecta → `400`; `auth` de longitud incorrecta → `400`. Verificar en rojo con `cargo test subscribe`.
- [x] 2.3 RED — test que falla de `DELETE /api/push/subscriptions`: elimina y responde `204`; endpoint desconocido → `204` igualmente; sin `endpoint` → `400`. Verificar en rojo con `cargo test unsubscribe`.
- [x] 2.4 GREEN — implementar `src/routes/push.rs` y montar las tres rutas en `src/lib.rs`. Confirmar `cargo test subscribe` y `cargo test unsubscribe` en verde, y `cargo check` sin errores.

## 3. Worker de recordatorios

- [x] 3.1 RED — test que falla de la selección y la marca: un recordatorio vencido con suscripciones se envía y queda con `notified_at`; el mismo recordatorio no se reenvía en el ciclo siguiente; un recordatorio futuro no se toca. Verificar en rojo con `cargo test reminder_push`.
- [x] 3.2 RED — test que falla de la ausencia de suscripciones: sin dispositivos suscritos no hay envío ni marca; al registrar uno y seguir dentro de la ventana se entrega en el ciclo siguiente.
- [x] 3.3 RED — test que falla de la ventana de gracia: vencido dentro de la ventana se entrega; vencido fuera de la ventana se marca y se registra sin envío; el umbral se lee de `push_reminder_grace_hours` y con `'0'` todo lo vencido queda fuera de ventana.
- [x] 3.4 RED — test que falla del payload: `title` «Recordatorio», `body` el texto, `url` `/` y `tag` `reminder-<id>`. Verificar en rojo con `cargo test reminder_payload`.
- [x] 3.5 RED — test que falla de la inyección: un `PushSender` de prueba registra las llamadas; el worker no falla si el sender devuelve error. Verificar en rojo con `cargo test push_sender`.
- [x] 3.6 GREEN — implementar `src/workers/reminder_push.rs` dependiendo de la abstracción `PushSender`, registrarlo en `src/workers/mod.rs` y arrancarlo desde `src/workers/pool.rs` con el patrón de `EpisodicMemoryWorker`. Confirmar `cargo test reminder` en verde.

## 4. Frontend: service worker, gestor y tab de Ajustes

- [x] 4.1 RED — test que falla del contenido de `frontend/public/sw.js`: declara los handlers `push` y `notificationclick` y **no** registra un handler de `fetch`. Verificar en rojo con `npx vitest run`.
- [x] 4.2 GREEN — escribir `frontend/public/sw.js` (push sin datos no muestra nada; con payload usa `showNotification`; `notificationclick` cierra y enfoca/abre la pestaña). Confirmar el test en verde.
- [x] 4.3 RED — test que falla del registro tolerante en `frontend/src/main.tsx`: con soporte se llama a `register('/sw.js')`; sin `navigator.serviceWorker` no se llama y no hay error; un `register` que rechaza no desmonta la app.
- [x] 4.4 GREEN — implementar el registro tolerante en `main.tsx`. Confirmar en verde.
- [x] 4.5 RED — test que falla de la conversión base64url→`Uint8Array` en `frontend/src/push/pushManager.ts`: cadena url-safe sin padding → bytes correctos; longitud múltiplo de 4 → sin padding añadido; cadena vacía → array vacío.
- [x] 4.6 GREEN — implementar `frontend/src/push/pushManager.ts` y `frontend/src/api/push.ts`. Confirmar en verde.
- [x] 4.7 RED — test que falla del tab «Notificaciones» en `frontend/src/components/SettingsDialog.tsx`: activar pide permiso, suscribe y publica (`POST`); permiso denegado muestra mensaje y no publica; desactivar cancela (`unsubscribe`) y borra (`DELETE`); sin `PushManager` el switch aparece deshabilitado con explicación; la carga inicial refleja la suscripción existente.
- [x] 4.8 GREEN — implementar el tab «Notificaciones» en `SettingsDialog.tsx`. Confirmar `npx vitest run` y `npx tsc --noEmit` en verde.

## 5. Verificación

- [x] 5.1 Anotar el recuento de tests antes y después y dejar la suite de Rust completa en verde con `cargo test --no-fail-fast` (0 fallos).
- [x] 5.2 `cargo clippy --all-targets -- -D warnings` y `cargo fmt --check` limpios (0 warnings).
- [x] 5.3 En el frontend, `npx vitest run`, `npx tsc --noEmit`, `npm run lint` y `npm run build` en verde.
- [x] 5.4 Regenerar el `static/` versionado con el nuevo build (`rm -rf static && cp -r frontend/dist static`) y comprobar que el backend sirve `/sw.js` como estático.
- [ ] 5.5 Construir la imagen Docker sobre **musl** (`alpine`) y comprobar que enlaza sin OpenSSL (sin `libssl.so.3`/`libcrypto.so.3`).
- [ ] 5.6 Verificación de punta a punta en un **navegador real**: activar las notificaciones, crear un recordatorio vencido de verdad y comprobar que llega el aviso (y que no se repite en el ciclo siguiente).

## Notas

- La migración `20261012000001_webpush_notifications.sql` es **aditiva e idempotente** y **no reconstruye ninguna tabla** —se remarca porque es lo primero que se revisa tras el incidente de la migración del timeline.
- La clave privada VAPID **nunca** se registra en un log, en un `Debug` ni en una respuesta HTTP; va en `SENSITIVE_KEYS`.
- La verificación de punta a punta se hace con un recordatorio vencido real: **no hay endpoint de prueba** `POST /api/push/test`.
