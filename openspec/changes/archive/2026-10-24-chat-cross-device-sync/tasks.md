# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y **esperar el OK explícito** del usuario antes de escribir código o tests.

## 1. RED — cursor de mensajes

- [x] 1.1 Test que falla: `list_all` ordena por `(created_at, id)` y el orden es determinista a igualdad de marca temporal.
- [x] 1.2 Test que falla: dos mensajes del mismo segundo se reparten entre dos páginas sin perderse ni repetirse.
- [x] 1.3 Test que falla: `list_after` devuelve los posteriores al cursor en orden ascendente y el cursor del último devuelto.
- [x] 1.4 Test que falla: `list_after` sin posteriores devuelve lista vacía y `None`.

## 2. RED — bus y publicación

- [x] 2.1 Test que falla: tras persistir el mensaje de usuario, el orquestador publica un evento con esa fila.
- [x] 2.2 Test que falla: tras persistir el mensaje del asistente, el orquestador publica un evento con esa fila.
- [x] 2.3 Test que falla: un turno que falla no publica un mensaje del asistente que no llegó a persistirse.
- [x] 2.4 Test que falla: el bus NO transporta fragmentos del turno (ningún evento contiene texto parcial).

## 3. RED — endpoint de eventos

- [x] 3.1 Test que falla: el primer evento es `snapshot` con `busy` reflejando el estado real.
- [x] 3.2 Test que falla: `since` hace replay solo de los posteriores al cursor, en orden ascendente.
- [x] 3.3 Test que falla: sin mensajes posteriores el replay está vacío y el `snapshot` llega igual.
- [x] 3.4 Test que falla: un cursor ilegible devuelve `400` y no abre stream.
- [x] 3.5 Test que falla: la respuesta lleva `Content-Type: text/event-stream`, `Cache-Control: no-cache, no-transform` y `X-Accel-Buffering: no`.
- [x] 3.6 Test que falla: una conexión ociosa emite latidos y no se cierra.

## 4. RED — cerrojo de turno

- [x] 4.1 Test que falla: con un turno en curso, un segundo `POST /api/chat/stream` responde `409` con `busy: true` y no persiste mensaje de usuario.
- [x] 4.2 Test que falla: el turno admitido publica inicio y fin de turno en el bus.
- [x] 4.3 Test que falla: un turno que termina con error libera el cerrojo y el siguiente envío arranca.
- [x] 4.4 Test que falla: resolver una aprobación `POST /api/approval/{id}` no requiere el cerrojo.
- [x] 4.5 Test que falla: `client_message_id` válido se usa como id del mensaje de usuario.
- [x] 4.6 Test que falla: `client_message_id` ausente, vacío, demasiado largo o ya usado hace que el servidor genere el id sin fallar el turno.

## 5. GREEN — backend

- [x] 5.1 `src/db/repos/messages.rs`: cursor compuesto `(created_at, id)`, orden total y `list_after`.
- [x] 5.2 `src/lib.rs`: bus de eventos del chat y cerrojo de turno en el estado de la aplicación.
- [x] 5.3 `src/orchestrator/agent.rs`: publicar en el bus cada mensaje persistido (usuario y asistente).
- [x] 5.4 `src/routes/stream.rs`: `GET /api/chat/events` con `snapshot`, replay, flujo en vivo y latido; `409` cuando el turno está ocupado; `client_message_id`.
- [x] 5.5 `src/main.rs`: montar la ruta nueva bajo la misma sesión.
- [x] 5.6 Tratar `RecvError::Lagged` como pérdida de sincronía: cerrar el stream al cliente para que reconecte con cursor.

## 6. RED — frontend

- [x] 6.1 Test que falla: `useChatSync` fusiona por upsert y no duplica un mensaje recibido por las dos vías.
- [x] 6.2 Test que falla: la lista se ordena por `(created_at, id)` con la marca temporal del servidor.
- [x] 6.3 Test que falla: la burbuja optimista se sustituye por su fila canónica y no se duplica.
- [x] 6.4 Test que falla: al volver a ser visible la pestaña se relee lo perdido y se reabre la suscripción.
- [x] 6.5 Test que falla: el reintento usa retroceso exponencial con el último cursor y no supera los 30 s.
- [x] 6.6 Test que falla: el indicador de turno ajeno aparece con el inicio de turno y desaparece con el fin.
- [x] 6.7 Test que falla: un `409` muestra el aviso del servidor y no deja el mensaje como enviado.
- [x] 6.8 Test que falla: un `401` dispara el evento global de sesión caducada y no reintenta en bucle.

## 7. GREEN — frontend

- [x] 7.1 `src/hooks/useChatSync.ts`: suscripción, fusión por id, orden, recuperación al volver y reintento.
- [x] 7.2 `src/hooks/useMainChat.ts`: integrar la suscripción, `client_message_id` y adopción de la marca temporal del servidor.
- [x] 7.3 `src/components/ChatView`: indicador de turno ajeno y aviso de chat ocupado.
- [x] 7.4 `src/api/client.ts` y `src/types/index.ts`: contrato del endpoint de eventos y campos nuevos.

## 8. Verificación y cierre

- [x] 8.1 `cargo test` en verde (baseline: medir con `cargo test` antes de empezar y anotar el número).
- [x] 8.2 `cargo clippy --all-targets -- -D warnings` (0 warnings).
- [x] 8.3 `cargo fmt --check` en verde.
- [x] 8.4 `npx vitest run` y `npx tsc --noEmit` en verde.
- [x] 8.5 `openspec validate chat-cross-device-sync --strict` en verde.
- [x] 8.6 Revisión con `@rust-reviewer` y `@react-reviewer`; hallazgos aplicados.
- [ ] 8.7 Prueba de dos clientes reales sobre el despliegue de trabajo: mensaje en uno, aparición en el otro, y recuperación tras suspender la pestaña.
- [ ] 8.8 `openspec archive chat-cross-device-sync` y anotar el cierre en `AGENTS.md` §V.
