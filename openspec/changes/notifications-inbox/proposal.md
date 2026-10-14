# Proposal: Marcar todas las notificaciones como leídas y una bandeja de avisos

## Why

Dos defectos en una misma feature, ambos verificados en producción.

**1. El agente no puede marcar todas las notificaciones como leídas.**

Los turnos del **2026-10-13 19:12** y **2026-10-14 05:04** piden «marca todas las notificaciones
como leídas». El log del contenedor muestra el patrón:

```
list_unread                      → OK (devuelve 3 avisos)
mark_read tool_args={"operation":"mark_read"}   ← SIN `ids`
  → Invalid arguments: se requiere al menos un `id` en `ids`
  → (reintento 1..6-7)
  → ⚠️ Tool retry limit reached tool_name=notifications::mark_read
```

Los **3 avisos siguen con `read_at = NULL`**. El modelo incluso llegó a afirmar que había pasado
los ids; falso: los reintentos repiten `{"operation":"mark_read"}` sin `ids`.

**La herramienta NO está rota y el agente SÍ ve los avisos.** `list_unread` funciona y las tabula.
Lo que no existe es la operación «marcar todo lo no leído»: hoy `mark_read` exige `ids` y rechaza
la lista vacía (`ToolError::InvalidArguments("se requiere al menos un `id` en `ids`")`), y la ruta
`POST /api/notifications/mark-read` responde `400` con `ids` ausente o vacío. El agente no tiene
forma de expresar «todas», así que entra en un bucle de reintentos que no puede converger.

**2. No hay ninguna interfaz de bandeja de avisos.**

Cero consumidores de `/api/notifications/*` en el front: el único punto que toca notificaciones es
el tab de suscripción push de los ajustes. No hay forma de ver los avisos pendientes ni de marcar
uno o todos como leídos desde la interfaz.

## What Changes

- **AÑADIDO — `mark_read` con `ids` ausente marca TODAS las no leídas.** En la herramienta
  `notifications.mark_read` y en `POST /api/notifications/mark-read`, omitir `ids` es la expresión
  deliberada de «todas»; un `ids: []` explícito sigue siendo error (`InvalidArguments` en la
  herramienta, `400` en la ruta). La operación es idempotente: repetirla devuelve `marked: 0`.
- **AÑADIDO — `NotificationsRepo::mark_all_read`**: `UPDATE … WHERE read_at IS NULL`, devuelve
  `rows_affected()`.
- **MODIFICADO — `list_unread` pagina por cursor keyset.** La consulta ordena
  `created_at DESC, rowid DESC` y usa el cursor `"<created_at>|<rowid>"`; `created_at` es
  `datetime('now')` con granularidad de segundo, así que el cursor codifica **ambos** campos. La
  respuesta añade `next_cursor` (null en la última página) y conserva `total` (recuento de no
  leídas, independiente de la página).
- **MODIFICADO — `GET /api/notifications/unread`** acepta el parámetro opaco `cursor`; un cursor
  malformado responde `400`. `limit` conserva su default (50) y su máximo (200).
- **AÑADIDO — Capability de frontend `frontend/notifications`:** tipo `Notification`, métodos de
  API `listUnreadNotifications` / `markNotificationsRead` / `markAllNotificationsRead`, hook
  `useNotifications` con pila de cursores, componente `NotificationsView` (10 por página, marcar
  uno, marcar todos, fecha en UTC) y botón `BellOutlined` en el header de `AppLayout` que lo abre
  en un modal.

## Capabilities

### New Capabilities

- `frontend/notifications`: la bandeja de avisos del frontend — tipo, API, hook, vista y su
  integración en el header. 4 requisitos nuevos.

### Modified Capabilities

- `notifications`: **2 requisitos modificados** — el de `GET /api/notifications/unread` (pasa a
  paginar por cursor keyset) y el de `POST /api/notifications/mark-read` (gana la marca masiva con
  `ids` ausente). El requisito de la herramienta `notifications` también se toca para recoger la
  paridad (`mark_read` sin `ids` marca todas).
- `frontend`: **afectada** porque la decisión D9 (ya resuelta) mueve el helper
  `parseSqliteTimestamp` a `frontend/src/utils/sqliteTimestamp.ts`, lo que reapunta el import de
  `components/stats/BackgroundCard.tsx` y `components/stats/BackgroundCard.test.tsx`.

## Impact

- **Backend:** `src/db/repos/notifications.rs` (`mark_all_read` nuevo; `list_unread` con cursor),
  `src/routes/notifications.rs` (`cursor` en `GET /unread`; `ids` ausente en `POST /mark-read`),
  `src/tools/notifications.rs` (`mark_read` sin `ids`; `list_unread` adaptado a la firma nueva;
  texto de `description()` y de los parámetros).
- **Frontend:** `types/index.ts` (tipo `Notification`), `api/client.ts` (3 métodos),
  `hooks/useNotifications.ts` (hook nuevo), `components/NotificationsView.tsx` (componente nuevo),
  `components/AppLayout.tsx` (botón + modal), y el helper de fechas movido por D9
  (`components/stats/sqliteTimestamp.ts` → `utils/sqliteTimestamp.ts`, con test propio).
- **Base de datos:** ninguna migración. El índice parcial `idx_notifications_unread` ya sirve al
  orden nuevo.
- **Spec:** deltas en `notifications` (2-3 MODIFIED), `frontend/notifications` (4 ADDED) y
  `frontend` (reapuntado del import del helper movido por D9).
- **D9 resuelta (en `design.md`):** `parseSqliteTimestamp` vive en
  `frontend/src/utils/sqliteTimestamp.ts`.
- **Sin cambios** en `docker-compose.prod.yml`.

## Non-goals

- **No se toca el contrato de ingesta** (`POST /api/notifications`) ni su validación.
- **No se añade una pestaña nueva de ajustes** para la bandeja: vive en un modal del header.
- **No se cambia el comportamiento de `notificationclick` del service worker**: sigue solo
  enfocando la pestaña.
- **No se arreglan de golpe** las demás vistas que leen el crudo de SQLite.
- **No se migran datos.**
- **No se toca `docker-compose.prod.yml`.**
- **No se implementa el change `ingest-keys-timestamp-utc`**, aunque D9 le resuelva de facto la
  ubicación del helper compartido.
