# Proposal: Notificaciones WebPush (VAPID) y entrega de recordatorios

## Why

Los recordatorios de Valet tienen hoy `status='pending'` y un `datetime`, pero **nadie los
entrega**. La capa proactiva se retiró en `2026-09-30-remove-proactive-workers` precisamente porque
no hacía nada observable para el usuario: el worker miraba, no enviaba, y el recordatorio se quedaba
en la base de datos. El resultado es una funcionalidad a medias: el asistente *guarda* recordatorios,
pero no los *avisa*.

WebPush cierra ese hueco con dos piezas independientes: el **transporte** —la suscripción del
navegador, las claves VAPID y el envío cifrado conforme a RFC 8030/8188/8291— y la **entrega** —un
worker que vigila los recordatorios que vencen y empuja un aviso—. Es, por tanto, una capability
nueva de cero en el backend y en el frontend, más dos añadidos aditivos sobre `db/schema` y
`workers`.

## What Changes

- **Transporte WebPush en el backend** (`push`): generación y persistencia de las claves VAPID,
  registro y baja de las suscripciones del navegador, y envío cifrado. Tres rutas nuevas:
  `GET /api/push/vapid-key`, `POST /api/push/subscriptions` y `DELETE /api/push/subscriptions`.
- **Entrega automática de recordatorios vencidos** (`workers`): un `ReminderPushWorker` que cada 60
  segundos selecciona los recordatorios vencidos, los empuja a todas las suscripciones y los marca.
- **Esquema de datos** (`db/schema`): tabla `push_subscriptions`, columna `reminders.notified_at` y
  siembra de los ajustes de WebPush, en una migración aditiva e idempotente.
- **Interfaz** (`push-ui`): un service worker `frontend/public/sw.js`, el registro tolerante en
  `main.tsx`, un gestor de suscripción y un tab «Notificaciones» en `SettingsDialog`.

### Decisiones de diseño y desviaciones de la especificación técnica

- **Dependencia `web-push-native = "0.5"` en lugar de `web-push = "0.10"`.** La imagen de producción
  es `alpine:3.21` con **musl** y solo lleva `ca-certificates`. El crate `web-push` arrastra, por su
  dependencia no opcional `ece`, el `openssl` del sistema —verificado con `ldd`: el binario enlaza
  `libssl.so.3` y `libcrypto.so.3`— y su feature por defecto es un cliente `isahc` (curl). Meterlo
  obligaría a añadir `openssl-dev` al builder y `libssl3`/`libcrypto3` a la imagen final, y a
  arrastrar un segundo stack TLS. `web-push-native` es Rust puro, devuelve un
  `http::Request<Vec<u8>>` listo para enviar con el `reqwest` que el proyecto ya usa, y no introduce
  `hyper`. **Alternativa si se rechaza:** quedarse con `web-push 0.11` añadiendo `openssl-dev` al
  builder y `libssl3 libcrypto3` a la imagen final.
- **Rutas `/api/push/...`, sin `/v1`.** El proyecto no tiene ninguna ruta versionada: todas son
  `/api/...`. Se mantiene la convención existente.
- **El DTO del payload sustituye `level` por `tag`.** `level` no tiene ningún consumidor en el
  service worker; `tag` sí, y evita que dos avisos del mismo recordatorio se apilen.
- **Se elimina el índice redundante `idx_push_subs_endpoint`** que traía la especificación: `endpoint`
  ya es `UNIQUE`, y eso crea el índice. Un índice explícito sobre la misma columna sería duplicado.
- **`badge` reutiliza `/icon-192.png`.** La especificación pedía `/assets/badge.png`, que no existe;
  añadir un asset nuevo queda fuera de alcance.
- **Fuera de alcance el endpoint de prueba `POST /api/push/test`.** El alcance elegido es la entrega
  *real* de recordatorios; la verificación de punta a punta se hace con un recordatorio vencido de
  verdad, no con un botón de test.
- **`notified_at` y la ventana de gracia (`push_reminder_grace_hours`) son añadidos de diseño** que
  la especificación del usuario no contemplaba. `notified_at` es lo que evita reenviar el mismo
  recordatorio; la ventana de gracia evita inundar de avisos antiguos al primer dispositivo que se
  suscriba.
- **`user_id` se mantiene pero siempre vale `'default'`.** Valet es monousuario; la identidad OIDC no
  se cablea en esta capa. Es un no-goal, no un olvido.
- **`axum 0.8` y `sqlx 0.9`** (la especificación decía `0.7`), que es lo que el proyecto usa, con
  `{param}` en las rutas y `sqlx::AssertSqlSafe` si hubiera SQL dinámico —aquí no lo hay—.

## Capabilities

### New Capabilities

- `push`: transporte WebPush en el backend —claves VAPID, suscripciones y envío conforme a
  RFC 8030/8188/8291— sobre el cliente HTTP del proyecto.
- `push-ui`: service worker de notificaciones, registro tolerante, gestión de suscripción y tab de
  Ajustes en el frontend.

### Modified Capabilities

- `db/schema`: nueva tabla `push_subscriptions`, nueva columna `reminders.notified_at` y siembra de
  los ajustes de WebPush.
- `workers`: nuevo `ReminderPushWorker` en el `WorkerPool`, con entrega como máximo una vez, ventana
  de gracia y `PushSender` inyectable.

## Impact

- **Dependencias:** `Cargo.toml` y `Cargo.lock` (`web-push-native`).
- **Datos:** `migrations/20261012000001_webpush_notifications.sql` (nueva).
- **Backend:** `src/db/repos/push.rs` (nuevo), `src/db/repos/settings.rs` (`SENSITIVE_KEYS`),
  `src/db/repos/reminders.rs` (marcar `notified_at`), `src/services/push.rs` (nuevo),
  `src/routes/push.rs` (nuevo), `src/lib.rs` (rutas), `src/workers/reminder_push.rs` (nuevo),
  `src/workers/mod.rs` y `src/workers/pool.rs`.
- **Frontend:** `frontend/public/sw.js` (nuevo), `frontend/src/main.tsx`,
  `frontend/src/push/pushManager.ts` (nuevo), `frontend/src/api/push.ts` (nuevo),
  `frontend/src/components/SettingsDialog.tsx`.
- **Build:** `static/` (build versionado que hay que regenerar).

## Non-goals

- **No se cablea la identidad OIDC a las notificaciones.** `user_id` existe en la tabla pero siempre
  vale `'default'`; el envío no segmenta por usuario.
- **No hay endpoint de prueba** `POST /api/push/test`: la verificación es la entrega real.
- **No se añade un asset nuevo** para el `badge`; se reutiliza `/icon-192.png`.
- **No se introducen notificaciones de eventos, tareas ni memoria**, solo de recordatorios.
- **No se toca `docker-compose.prod.yml`.**
