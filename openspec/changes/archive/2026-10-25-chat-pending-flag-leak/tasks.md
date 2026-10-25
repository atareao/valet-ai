# Tasks

## 0. Aprobación

- [x] 0.1 Presentar este change (proposal + design + specs + tasks) y **esperar el OK explícito** del usuario antes de escribir código o tests.

## 1. RED — regresión de la carrera

- [x] 1.1 Test que falla: con la secuencia «burbuja optimista de usuario → fila canónica de usuario por el bus → fila canónica de asistente por el bus → `done` del asistente», tras enviar un mensaje nuevo el asistente anterior permanece por encima del mensaje nuevo.
- [x] 1.2 Test que falla: tras el `done` de un asistente cuya fila canónica ya llegó por el bus, ningún identificador queda marcado como pendiente.
- [x] 1.3 Test que falla: `sortMessages` coloca al final un mensaje marcado como optimista y lo devuelve a su posición `(created_at, id)` cuando la marca se retira.

## 2. GREEN — la marca viaja en el mensaje

- [x] 2.1 `Message` admite un campo interno opcional `pending?: boolean`, documentado como no persistido.
- [x] 2.2 `sortMessages` lee la marca del propio mensaje en lugar de un `Set` de identificadores.
- [x] 2.3 `useChatSync.upsert` fija y retira la marca dentro del mismo upsert; se elimina `pendingIdsRef`.
- [x] 2.4 La fusión por id (bus, `done` y burbuja optimista) converge sin dejar estado marcado como pendiente.

## 3. REFACTOR

- [x] 3.1 `npx tsc --noEmit`, `npm run lint`, `npx vitest run`.
- [x] 3.2 Revisar que ningún otro punto del frontend dependa de `pendingIdsRef`.

## 4. Verificación y cierre

- [x] 4.1 `openspec validate chat-pending-flag-leak`.
- [x] 4.2 `just check-spec`.
- [x] 4.3 `openspec archive chat-pending-flag-leak`.
