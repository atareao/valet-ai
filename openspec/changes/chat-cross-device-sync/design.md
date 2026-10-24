# Design: Sincronización del chat entre dispositivos

## Context

Valet tiene **un único hilo de chat global**: no existe la tabla `conversations` y hay un test que lo prohíbe (`src/db/schema.rs`), así que todos los dispositivos comparten la tabla `messages`. El frontend carga el historial una sola vez al montar (`GET /api/chat/init`, últimos 50 mensajes) y no reacciona al foco ni a la vuelta a la pestaña: no hay ni un solo `visibilitychange` en todo el frontend. El streaming (`POST /api/chat/stream`) es un canal `mpsc` por petición: solo lo ve quien pregunta. La única infraestructura de difusión existente es la de Web Push y el apagado de workers.

Dos piezas existentes encajan con el trabajo: `GET /api/messages` ya pagina con cursor, y el evento `done` ya informa de los identificadores persistidos.

## Goals / Non-Goals

**Goals**

- Que un mensaje escrito en un cliente aparezca en los demás sin recargar.
- Que un cliente que se reconecta (móvil en segundo plano, red caída, pestaña dormida) recupere exactamente lo que se perdió, sin huecos ni duplicados.
- Que haya un único turno a la vez, con un aviso claro al segundo cliente.
- Que todo lo anterior sea verificable sin navegador: el bus, el cerrojo, el cursor y el endpoint se prueban con tests de Rust.

**Non-Goals**

- Ver la respuesta escribiéndose en el otro cliente: se sincronizan turnos cerrados más un indicador de turno ajeno.
- Cola de turnos: el segundo envío se rechaza, no espera.
- Conversaciones múltiples: el hilo sigue siendo único y global.
- `docker-compose.prod.yml` y producción.

## Decisions

### D1 — Bus de eventos con `tokio::sync::broadcast` en el estado de la aplicación

Un `broadcast` de capacidad 256 con los eventos del chat. Un receptor lento pierde eventos (`RecvError::Lagged`); eso **no** se ignora: se trata como pérdida de sincronía y el stream se cierra al cliente con un evento de resincronización, para que vuelva a conectar con cursor y relea. Nunca silencio.

### D2 — El evento canónico es la fila persistida, no un delta

`message_created` lleva el mensaje completo con la misma forma que devuelve `GET /api/messages`. Así el cliente hace un **upsert idempotente por id** y el orden relativo entre el evento `done` del propio turno y el evento del bus deja de importar. La publicación ocurre **después** de la escritura en base de datos, de modo que quien reaccione al evento ya puede leer la fila.

### D3 — Cursor de orden total: la pareja `(created_at, id)`

Hoy el cursor de `list_all` es `created_at < ?` sobre la pareja `(created_at, id)` y esa pareja —no la marca temporal suelta— es la que garantiza un orden total: dos mensajes con la misma marca se perderían en un replay si el cursor solo mirara `created_at`.

Sobre el formato de `created_at`: el código real **no** escribe `datetime('now')`. `MessagesRepo::create_with_id` estampa siempre `Utc::now().to_rfc3339()` —formato ISO 8601 con `T` y fracción sub-segundo, p. ej. `2026-10-24T14:18:27.194677551+00:00`—. El `datetime('now')` es solo el `DEFAULT` del esquema (`migrations/20260925000001_initial.sql`) y **únicamente lo usan fixtures de test**: ninguna ruta de producción inserta sin pasar el `created_at`.

Consecuencia que conviene dejar escrita: como la columna es `TEXT`, el orden total `(created_at, id)` es **lexicográfico**. Si en el futuro alguna fila se insertara dejando actuar al `DEFAULT` (formato con espacio, `2026-10-24 14:18:27`), ordenaría **antes** que todas las filas de producción, sea cual sea su fecha, porque `0x20` (espacio) es menor que `'T'` (`0x54`). Es un **riesgo conocido, no un fallo actual**: hoy no hay ninguna ruta que dependa del `DEFAULT`. Este change no lo aborda —no lleva migración ni cambio de esquema—; queda anotado para quien toque el esquema más adelante.

El cursor pasa a ser la pareja `(created_at, id)` y el orden de la tabla, `created_at` ascendente con `id` ascendente para desempatar. Sigue siendo **opaco** para el cliente.

Alternativas descartadas: `rowid` — es monótono hoy, pero `MessagesRepo::delete_all` existe y SQLite reutiliza el `rowid` máximo borrado, lo que invalidaría un cursor vivo; y una columna `seq` nueva con `ALTER TABLE` y backfill, que añade una migración para algo que la pareja ya resuelve.

### D4 — El cliente ordena por la pareja del servidor

Para que el orden tras un replay sea idéntico al orden en vivo, la lista se ordena por `(created_at, id)` y el cliente **adopta siempre la marca temporal del servidor**. Hoy el mensaje del asistente se pinta con la hora local del navegador, así que dos dispositivos mostrarían horas distintas del mismo mensaje. Los mensajes optimistas van al final hasta ser sustituidos por su fila canónica.

### D5 — `client_message_id`: el cliente propone el id

El cliente envía el id que quiere para su mensaje de usuario; el servidor lo usa si es una cadena no vacía de hasta 64 caracteres que no exista ya, y si no, genera uno propio **sin fallar el turno**. Con esto la fusión es un upsert por id puro y desaparece el emparejamiento por contenido, que fallaría al enviar dos veces el mismo texto.

### D6 — Cerrojo de turno con guarda RAII

El estado expone un cerrojo que `POST /api/chat/stream` intenta adquirir **sin bloquear**. Si está ocupado, responde `409` con `{ "error": …, "busy": true }`. La guarda libera el cerrojo y publica el fin del turno en su `Drop`, de modo que un error —o un pánico dentro del turno— no deja el chat bloqueado para siempre. Descartado encolar el segundo turno: se pidió aviso, no cola.

### D7 — Latido contra los proxies intermedios

Un comentario SSE cada 15 s mantiene viva la conexión ante Traefik y nginx, que cierran conexiones inactivas. Un comentario no es un evento y el cliente lo ignora sin tratamiento especial.

### D8 — Recuperación en tres capas

(1) `since=<cursor>` en cada apertura y reapertura del stream; (2) relectura al volver a la pestaña (`visibilitychange` → visible); (3) reintento con retroceso exponencial de 1 s a 30 s si el stream muere. Un `401` dispara el evento global de sesión caducada, igual que el resto de la aplicación.

### D9 — Solo turnos cerrados, con indicador

El indicador de turno ajeno se deriva de `turn_started`, `turn_finished` y del `snapshot` inicial. No se difunden fragmentos de texto.

### D10 — El `409` es HTTP, no un evento dentro de un `200`

El cliente ya traduce una respuesta `!ok` a su manejador de error y muestra el mensaje del servidor. Abrir un stream SSE únicamente para decir «no» sería más caro y más difícil de probar.

## Risks / Trade-offs

- Una conexión SSE persistente por pestaña: se cierra al desmontar el componente, y con pocos dispositivos el coste es despreciable.
- El `409` cambia el contrato de un endpoint existente; un cliente que lance dos turnos en paralelo dejará de verlos intercalados. Es el comportamiento buscado.
- El cerrojo vive en el proceso: con una sola instancia no hay problema, y hoy Valet se despliega como una sola.
- El cursor cambia de forma; como es opaco, ningún cliente lo interpreta, pero los cursores en vuelo de una sesión abierta dejan de ser válidos y se traducen en una relectura desde el principio.

## Migration Plan

- Sin migración de base de datos.
- Un cliente antiguo no abre `/api/chat/events` y sigue funcionando como hoy.
- Un cursor de formato viejo recibido en el stream de eventos se rechaza con `400`; el cliente vuelve a conectar sin `since`.

## Open Questions

- Ninguna bloqueante. La conveniencia de revisar el modelo de chat por defecto de las plantillas se deja para el change de la deuda del agente, no para éste.
