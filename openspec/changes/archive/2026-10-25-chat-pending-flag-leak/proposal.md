# Change: La marca de optimista no debe filtrarse al mensaje canónico

## Why

Al enviar un mensaje, la respuesta del asistente **anterior** salta por debajo del mensaje nuevo: el orden de la lista se corrompe a partir del segundo envío.

La reproducción confirma que la causa está en el cliente, no en el servidor. El servidor persiste los mensajes con marcas temporales `(created_at, id)` correctas y alternancia `user`/`assistant` impecable; el log de producción no muestra ningún error de stream, de persistencia ni de bus de eventos. El fallo está en `useChatSync.upsert` (`frontend/src/hooks/useChatSync.ts`):

1. La condición de «mensaje optimista» vive en un ref aparte, `pendingIdsRef` (un `Set` de identificadores), y `sortMessages` manda **al final** todo id que esté en ese `Set`.
2. `upsert` **marca** el id en `pendingIdsRef` **antes** de decidir si la escritura se descarta.
3. Cuando el mensaje ya existe como fila canónica (`prev.some(m => m.id === message.id)`), `upsert` hace `return prev` y **se sale sin desmarcar** el id.

Resultado: si la fila **canónica** del asistente llega por el bus de eventos (`GET /api/chat/events`) **antes** que el evento `done` del stream —y suele llegar antes, porque el servidor hace un `await find_by_id` entre publicar el mensaje y emitir `done`—, el `done` entra por esa rama de descarte y deja el id del asistente **marcado como pendiente para siempre**. A partir de ahí, `sortMessages` lo empuja al final en **cada** reordenado posterior, de modo que el mensaje siguiente lo adelanta.

Es un fallo **preexistente** del change `chat-cross-device-sync` (v0.17.0); no lo introdujeron ni `chat-stream-resilience` ni `chat-reasoning-default-off`.

## What Changes

- **MODIFIED** la ordenación de la lista: la condición de «optimista» pasa a viajar **en el propio mensaje** (un campo interno que no se envía al servidor), en lugar de vivir en un estado aparte indexado por identificador. Una fila canónica nunca puede conservar la marca.
- **MODIFIED** `upsert` de `useChatSync`: se elimina `pendingIdsRef` y su actualización anticipada; la marca se fija y se retira junto con la fila, en el mismo upsert.
- **Nuevo** test de regresión que cubre la carrera «el bus canónico del asistente adelanta al `done` del stream» y su consecuencia sobre el envío siguiente.

## Capabilities

- **New Capabilities**: (ninguna)
- **Modified Capabilities**:
  - `frontend`: la marca de «mensaje optimista» vive en el mensaje y nunca se filtra a la fila canónica; el orden por `(created_at, id)` prevalece siempre.

## Impact

- **Frontend**: `src/hooks/useChatSync.ts` (se elimina `pendingIdsRef`), `src/utils/chatMessages.ts` (`sortMessages` lee la marca del mensaje), `src/types/index.ts` (campo interno opcional en `Message`).
- **Sin cambios de backend ni de base de datos**: el servidor ya ordena y persiste correctamente.
- **Sin migración**: el campo interno no se persiste ni viaja al servidor.
- **Fuera de alcance**: cambiar el bus de eventos o el protocolo del cursor, `docker-compose.prod.yml` ni producción.
