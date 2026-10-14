# Tasks

Todas las casillas empiezan sin marcar. **No se escribe ni una línea de código ni un solo test antes de que el usuario apruebe este change.**

Comandos de verificación de este repo (backend y frontend): `cargo test`, `cargo clippy -- -D warnings`, `cargo fmt --check`, `npx vitest run`, `npx tsc --noEmit`, `npm run lint`, `openspec validate --all`.

## 0. Aprobación (punto de parada del protocolo)

- [x] 0.1 Presentar este change (proposal + deltas de `notifications` y `frontend/notifications` + design + tasks) y **esperar aprobación explícita** antes de tocar código. **No se escribe código ni tests antes de la aprobación.**
- [x] 0.2 Resolver la **decisión abierta D9** del `design.md`: dónde vive `parseSqliteTimestamp` — mover a `frontend/src/utils/sqliteTimestamp.ts` (recomendado), duplicar, o importar desde `stats/`. **Bloquea el primer paso de GREEN de frontend.**
  Resuelta: mover el helper a `frontend/src/utils/sqliteTimestamp.ts` (recomendado), con test propio y reapuntando el import de `stats/BackgroundCard.tsx` y `stats/BackgroundCard.test.tsx`.

## 1. RED — backend (repositorio)

- [ ] 1.1 RED — test de `NotificationsRepo::mark_all_read`: marca todas las no leídas y devuelve el número de filas (`rows_affected`). **Pegar la salida en rojo en el informe.**
- [ ] 1.2 RED — test: `mark_all_read` es **idempotente**; la segunda llamada devuelve `0` y no altera los `read_at` ya fijados.
- [ ] 1.3 RED — test: `list_unread` con cursor keyset **no salta los avisos empatados del mismo segundo**. Insertar dos filas con el mismo `created_at` y comprobar que la segunda página no pierde ninguna (el cursor de solo `created_at` las perdería).
- [ ] 1.4 RED — test: `list_unread` respeta `created_at DESC, rowid DESC` y el `limit` de la página.
- [ ] 1.5 RED — test: `total` es el recuento de no leídas, **independiente** de la página.
- [ ] 1.6 RED — test: `next_cursor` es `Some("<created_at>|<rowid>")` de la última fila solo si hay más, y `None` en la última página.

## 2. RED — backend (ruta)

- [ ] 2.1 RED — test: `GET /api/notifications/unread?limit=10` devuelve un `PaginatedResponse` con `next_cursor` no nulo cuando hay más de 10 no leídas. **Pegar la salida en rojo en el informe.**
- [ ] 2.2 RED — test: seguir el `cursor` devuelto recupera la página siguiente sin perder filas (incluidos los empatados del mismo segundo).
- [ ] 2.3 RED — test: un `cursor` malformado (no encaja en `"<created_at>|<rowid>"`) responde `400`.
- [ ] 2.4 RED — test: `next_cursor` es `null` en la última página.
- [ ] 2.5 RED — test: `POST /api/notifications/mark-read` **sin `ids`** marca todas las no leídas y responde `{"marked": n}`.
- [ ] 2.6 RED — test: `POST /api/notifications/mark-read` con `ids: []` responde `400`.
- [ ] 2.7 RED — test: la marca masiva es **idempotente**; repetirla responde `{"marked": 0}`.

## 3. RED — backend (herramienta)

- [ ] 3.1 RED — test: `notifications.mark_read` **sin `ids`** marca todas las no leídas. **Pegar la salida en rojo en el informe.**
- [ ] 3.2 RED — test: `notifications.mark_read` con `ids: []` explícito sigue devolviendo `InvalidArguments`.
- [ ] 3.3 RED — test: paridad ruta/herramienta — misma semántica para «todas» y para `ids: []`.
- [ ] 3.4 RED — test: `list_unread` de la herramienta se adapta a la firma nueva (sin cursor) sin cambio de comportamiento visible.

## 4. GREEN — backend

- [ ] 4.1 GREEN — `src/db/repos/notifications.rs`: implementar `mark_all_read(pool) -> Result<u64, sqlx::Error>` y cambiar `list_unread` a la firma con `cursor: Option<&str>` que devuelve `(Vec<Notification>, Option<String>, i64)` (trae `limit+1` filas).
- [ ] 4.2 GREEN — `src/routes/notifications.rs`: `cursor` opaco en `GET /unread` (malformado → `400`); `ids` ausente en `POST /mark-read` → marca todas; `ids: []` → `400`.
- [ ] 4.3 GREEN — `src/tools/notifications.rs`: `mark_read` sin `ids` llama a `mark_all_read`; `list_unread` adaptado; actualizar `description()` y las descripciones de `operation` e `ids` para que el modelo sepa que omitir `ids` marca todas.
- [ ] 4.4 GREEN — `cargo test` en verde.

## 5. RED — frontend

- [ ] 5.1 RED — test del helper compartido (test propio tras mover `parseSqliteTimestamp`) y prueba de que la suite de stats sigue verde tras reapuntar el import. **Pegar la salida en rojo en el informe.**
- [ ] 5.2 RED — test: tipo `Notification` y métodos de API `listUnreadNotifications` / `markNotificationsRead` / `markAllNotificationsRead`.
- [ ] 5.3 RED — test de `useNotifications`: 10 por página, pila de cursores (`goNext`/`goPrev`), `total`, `hasPrev`/`hasNext`, `markRead` y `markAll` refrescan.
- [ ] 5.4 RED — test de `NotificationsView`: lista 10 por página, recuento «N avisos pendientes», fila con origen/autor/nivel/mensaje/fecha.
- [ ] 5.5 RED — test: «Marcar como leído» por fila y «Marcar todos como leídos» llaman a la API.
- [ ] 5.6 RED — test: `created_at = "2026-10-14 05:04:00"` se muestra como **UTC** y un valor no parseable muestra `—` (nunca crudo ni «Invalid Date»).
- [ ] 5.7 RED — test: estado vacío cuando no hay avisos pendientes.
- [ ] 5.8 RED — test de integración en `AppLayout`: el botón `BellOutlined` abre el `Modal` con `NotificationsView`.

## 6. GREEN — frontend

- [ ] 6.1 GREEN — aplicar la decisión de D9: mover el helper a `frontend/src/utils/sqliteTimestamp.ts` y reapuntar `stats/BackgroundCard.tsx` y `stats/BackgroundCard.test.tsx`.
- [ ] 6.2 GREEN — `frontend/src/types/index.ts`: añadir el tipo `Notification`.
- [ ] 6.3 GREEN — `frontend/src/api/client.ts`: añadir los 3 métodos.
- [ ] 6.4 GREEN — `frontend/src/hooks/useNotifications.ts`: implementar el hook con pila de cursores.
- [ ] 6.5 GREEN — `frontend/src/components/NotificationsView.tsx`: implementar la vista.
- [ ] 6.6 GREEN — `frontend/src/components/AppLayout.tsx`: botón `BellOutlined` + `Modal` con `NotificationsView` bajo `React.lazy`/`Suspense`.
- [ ] 6.7 GREEN — `npx vitest run` en verde.

## 7. REFACTOR

- [ ] 7.1 REFACTOR — limpiar artefactos en cascada: imports obsoletos del helper en `stats/` y comentarios que queden desactualizados.
- [ ] 7.2 REFACTOR — `cargo clippy -- -D warnings` sin warnings.
- [ ] 7.3 REFACTOR — `cargo fmt --check` sin diferencias.
- [ ] 7.4 REFACTOR — `npx tsc --noEmit` sin errores.
- [ ] 7.5 REFACTOR — `npm run lint` sin warnings nuevos.

## 8. Verificación y cierre

- [ ] 8.1 `cargo test` (suite completa) en verde.
- [ ] 8.2 `cargo clippy -- -D warnings` y `cargo fmt --check` en verde.
- [ ] 8.3 `npx vitest run` en verde.
- [ ] 8.4 `npx tsc --noEmit` y `npm run lint` en verde.
- [ ] 8.5 `openspec validate --all` en verde.
- [ ] 8.6 **Medir en producción tras desplegar:** pedir al agente «marca todas las notificaciones como leídas» y comprobar que `mark_read` **sin `ids`** ya no entra en bucle de reintentos (`Invalid arguments` → `retry limit`). Dejar constancia del **antes** (los 3 avisos con `read_at = NULL`, turnos del 2026-10-13 19:12 y 2026-10-14 05:04) y el **después** (los avisos marcados y la operación sin reintentos).
- [ ] 8.7 Parar y esperar aprobación del usuario. Aprobar el archive (`openspec archive notifications-inbox`) en un PR aparte tras el merge.
