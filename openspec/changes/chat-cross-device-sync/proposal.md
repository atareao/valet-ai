# Change: Sincronización del chat entre dispositivos

## Why

El hilo de chat de Valet solo se sincroniza **al recargar la página**. `GET /api/chat/init` se llama una única vez al montar el frontend y nunca más, y `POST /api/chat/stream` devuelve sus eventos SSE **solo al cliente que pregunta**: un mensaje escrito en el móvil no aparece en el ordenador hasta que alguien refresca. Además, en móvil el sistema operativo suspende las conexiones en segundo plano, así que cualquier solución «en vivo» necesita además recuperarse al volver a la app. Y como no hay cerrojo de turno, dos clientes pueden lanzar dos turnos en paralelo sobre el mismo hilo.

Se aprovecha que `GET /api/messages` ya está paginado con cursor: ese cursor es el mecanismo natural de recuperación, pero hoy tiene resolución de segundo y no existe ninguna consulta en la dirección «posterior al cursor».

## What Changes

- **Nuevo** `GET /api/chat/events?since=<cursor>`: stream SSE de larga duración que emite un `snapshot` del estado del turno, hace replay de lo perdido y difunde cada mensaje persistido a todos los clientes suscritos.
- **Nuevo** bus de eventos de chat en el backend, en el que el orquestador publica la fila persistida de cada mensaje que escribe.
- **Nuevo** `client_message_id` opcional en el cuerpo de `POST /api/chat/stream`, para que cliente y servidor compartan el id del mensaje de usuario y la fusión sea un upsert por id.
- **BREAKING** `POST /api/chat/stream` responde `409` cuando ya hay un turno en curso: se admite un único turno a la vez.
- **MODIFIED** el cursor de paginación de mensajes pasa a ser un orden total sin empates (`created_at` + `id`), para que un replay no pierda ni duplique dos mensajes escritos en el mismo segundo.
- **Nuevo** en el frontend: suscripción al bus, fusión idempotente por id, orden por la pareja del servidor, relectura al volver a la pestaña y aviso de turno ajeno.

## Capabilities

- **New Capabilities**: (ninguna)
- **Modified Capabilities**:
  - `stream-route`: nuevo endpoint de eventos, latido, id propuesto por el cliente y cerrojo de turno.
  - `orchestrator`: publicación en el bus de cada mensaje persistido.
  - `messages-repo`: cursor de orden total sin empates y consulta en la dirección «posterior al cursor».
  - `frontend`: suscripción al bus, fusión idempotente, orden, recuperación al volver y aviso de turno ajeno.

## Impact

- **Backend**: `src/lib.rs` (estado del bus y del cerrojo), `src/routes/stream.rs`, `src/orchestrator/agent.rs`, `src/db/repos/messages.rs`, `src/models/message.rs`, router en `src/main.rs`.
- **Frontend**: `src/hooks/useChatSync.ts` (nuevo), `src/hooks/useMainChat.ts`, `src/hooks/useSSE.ts`, `src/api/client.ts`, `src/types/index.ts`, `src/components/ChatView`.
- **Sin migración de base de datos.** `client_message_id` es opcional: un cliente antiguo sigue funcionando.
- **Auth**: la ruta nueva queda bajo la misma sesión que el resto de la API; no se añaden permisos.
- **Rendimiento**: una conexión SSE persistente por cliente y pestaña, con latido cada 15 s.
- **Fuera de alcance**: ver el texto del otro turno escribiéndose en directo, cola de turnos, conversaciones múltiples y `docker-compose.prod.yml`.
