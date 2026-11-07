# PLAN-008: El chat, sincronizado entre dispositivos

## Objetivo

Que el hilo de chat de Valet esté igual en todos los clientes de un mismo usuario sin recargar la página: lo que se escribe en el móvil aparece en el ordenador, y cuando se vuelve a un dispositivo que estuvo suspendido la conversación ya está puesta al día, sin huecos ni duplicados. Además, un único turno a la vez, con aviso claro al segundo cliente que lo intente.

## Punto de partida (medido, no supuesto)

- **Valet tiene un único hilo global.** No hay tabla `conversations` ni `conversation_id`; lo prohíbe un test explícito en `src/db/schema.rs`. Todos los dispositivos comparten la tabla `messages`.
- **Hoy solo se sincroniza al recargar.** `GET /api/chat/init` se llama una vez al montar el frontend y nunca más; no existe ni un solo `visibilitychange` ni `onfocus` en todo el frontend.
- **El streaming es por petición.** `POST /api/chat/stream` alimenta un canal `mpsc` por petición: sus eventos SSE los ve solo el cliente que pregunta. El turno se persiste en `messages`, pero no se difunde.
- **La única difusión existente es Web Push** (notificaciones) y el apagado de workers. No hay bus de eventos reutilizable.
- **Dos piezas encajan ya**: `GET /api/messages` pagina con cursor, y el evento SSE `done` informa de los identificadores persistidos.
- **Dos agujeros reales detectados al leer el código**:
  1. El cursor de `list_all` es `created_at < ?` sobre una columna con formato `datetime('now')`, es decir **resolución de segundo**: dos mensajes escritos en el mismo segundo se perderían en un replay.
  2. **No existe ninguna consulta «posterior al cursor»**: el paginador solo mira hacia atrás, y un replay necesita la dirección contraria.
- **No hay cerrojo de turno.** Dos clientes pueden lanzar dos turnos en paralelo sobre el mismo hilo.

## Alcance

**Dentro**

- Endpoint SSE de larga duración `GET /api/chat/events?since=<cursor>`: estado inicial del turno, replay de lo perdido y difusión en vivo de cada mensaje persistido.
- Bus de eventos del chat en el backend; el orquestador publica la fila persistida de cada mensaje que escribe.
- Identificador de mensaje propuesto por el cliente (`client_message_id`) para que la fusión sea un upsert por id.
- Cursor de orden total sin empates (`created_at` + `id`) y consulta en la dirección «posterior al cursor».
- Cerrojo de turno: un turno a la vez, con `409` y aviso legible al segundo cliente.
- Frontend: suscripción, fusión idempotente, orden por la pareja del servidor, recuperación al volver a la pestaña e indicador de turno ajeno.

**Fuera**

- Ver el texto del otro turno escribiéndose en directo: se sincronizan turnos cerrados más un indicador.
- Cola de turnos: el segundo envío se rechaza, no espera.
- Conversaciones múltiples: el hilo sigue siendo único y global, como manda un test existente.
- `docker-compose.prod.yml` y el despliegue de producción.
- La deuda de código del agente (el bucle ReAct que agota iteraciones sin desenlace). Es un trabajo distinto y no se mezcla con éste.

## Decisiones de diseño

1. **Bus con `tokio::sync::broadcast`** de capacidad acotada en el estado de la aplicación. Un receptor lento (`Lagged`) no se ignora: se trata como pérdida de sincronía y el stream se cierra al cliente para que reconecte con cursor. Nunca silencio.
2. **El evento canónico es la fila persistida**, no un delta de texto. Así la fusión en el cliente es un upsert idempotente y el orden relativo entre el evento `done` del propio turno y el evento del bus deja de importar. Se publica **después** de escribir en la base de datos.
3. **Cursor de orden total `(created_at, id)`**, opaco para el cliente. Descartado `rowid` (monótono hoy, pero `MessagesRepo::delete_all` existe y SQLite reutiliza el `rowid` máximo borrado, lo que invalidaría un cursor vivo) y descartada una columna `seq` nueva con `ALTER TABLE` y backfill.
4. **El cliente ordena por la misma pareja y adopta la marca temporal del servidor.** Hoy el mensaje del asistente se pinta con la hora local del navegador, así que dos dispositivos mostrarían horas distintas del mismo mensaje.
5. **`client_message_id`**: el cliente propone el id de su mensaje; el servidor lo usa si es válido y no colisiona, y si no genera uno propio **sin fallar el turno**. Elimina el emparejamiento por contenido, que fallaría al enviar dos veces el mismo texto.
6. **Cerrojo de turno con guarda RAII**: la guarda libera el cerrojo y publica el fin del turno en su `Drop`, de modo que un error —o un pánico dentro del turno— no deja el chat bloqueado para siempre.
7. **Latido cada 15 segundos** como comentario SSE, para que Traefik y nginx no cierren la conexión inactiva.
8. **Recuperación en tres capas** en el cliente: `since=<cursor>` en cada apertura, relectura al volver a la pestaña, y reintento con retroceso exponencial de 1 s a 30 s si el stream muere.
9. **El `409` es HTTP**, no un evento dentro de un `200`: el cliente ya traduce una respuesta no-OK a su manejador de error y muestra el mensaje del servidor.

## Arquitectura

- **Estado**: bus de eventos del chat y cerrojo de turno viven en el estado de la aplicación (`src/lib.rs`), para que los tests puedan inspeccionarlos sin arrancar el servidor.
- **Orquestador** (`src/orchestrator/agent.rs`): publica en el bus la fila persistida del mensaje de usuario y la del asistente. No publica fragmentos.
- **Rutas** (`src/routes/stream.rs`): `GET /api/chat/events` (estado inicial, replay, vivo, latido) y `POST /api/chat/stream` con cerrojo y `client_message_id`.
- **Repositorio** (`src/db/repos/messages.rs`): cursor compuesto, orden total y `list_after`.
- **Frontend**: `useChatSync.ts` nuevo (suscripción, fusión, recuperación), `useMainChat.ts` integrado, `ChatView` con el indicador y el aviso.
- **Sin migración de base de datos.** `client_message_id` es opcional: un cliente antiguo sigue funcionando.

## Tareas (por fases, con TDD estricto)

- **Fase 0** — Aprobación del change de OpenSpec. Nada de código antes.
- **Fase 1 (RED)** — Tests que fallan del cursor: orden determinista a igualdad de marca temporal, dos mensajes del mismo segundo repartidos entre páginas sin perderse, `list_after` en orden ascendente, `list_after` sin posteriores.
- **Fase 2 (RED)** — Tests que fallan del bus: publicación del mensaje de usuario, publicación del mensaje del asistente, no publicar un asistente que no llegó a persistirse, y que el bus no transporte texto parcial.
- **Fase 3 (RED)** — Tests que fallan del endpoint de eventos: `snapshot`, replay acotado al cursor, replay vacío, cursor ilegible, cabeceras no transformables y latido.
- **Fase 4 (RED)** — Tests que fallan del cerrojo: `409` con `busy` sin persistir nada, publicación de inicio y fin de turno, liberación tras un turno que falla, aprobación que no requiere el cerrojo, y los cuatro casos de `client_message_id`.
- **Fase 5 (GREEN)** — Implementación mínima en repositorio, estado, orquestador, rutas y router; más el tratamiento de `Lagged`.
- **Fase 6 (RED)** — Tests que fallan del frontend: fusión sin duplicados, orden por la pareja, sustitución de la burbuja optimista, recuperación al volver a la pestaña, retroceso exponencial, indicador de turno ajeno, aviso de `409` y `401` sin bucle.
- **Fase 7 (GREEN)** — Implementación del frontend.
- **Fase 8** — Verificación y cierre: `cargo test`, `clippy` sin warnings, `fmt`, `vitest`, `tsc`, `openspec validate --strict`, revisión con los revisores, prueba con dos clientes reales y archivo del change.

## Riesgos

- **Una conexión SSE por pestaña**: se cierra al desmontar; con pocos dispositivos el coste es despreciable, pero conviene no dejarla abierta en pantallas que no muestran el chat.
- **El `409` cambia el contrato de un endpoint existente**: es el comportamiento buscado, pero hay que anotarlo como cambio incompatible.
- **El cerrojo vive en el proceso**: hoy Valet se despliega como una sola instancia, y con varias no bastaría.
- **Los cursores en vuelo de una sesión abierta** dejan de ser válidos al cambiar el formato; como son opacos, el efecto es una relectura desde el principio.
- **Dependencia del entorno de producción**: la incidencia previa del 24 de octubre dejó un riesgo abierto (la fuente que siembra el modelo en el contenedor no se localizó). No es de este plan, pero afecta a cualquier prueba en el despliegue.

## Criterios de aceptación

1. Un mensaje escrito en un dispositivo aparece en el otro sin recargar.
2. Un cliente que estuvo suspendido o perdió la red recupera exactamente lo perdido: sin huecos ni duplicados, incluso con mensajes de la misma marca temporal.
3. Dos dispositivos muestran la misma marca temporal para el mismo mensaje.
4. Un segundo envío simultáneo recibe un aviso legible y el texto no se da por enviado.
5. Un turno que falla no deja el chat bloqueado.
6. `cargo test`, `clippy --all-targets -- -D warnings`, `fmt --check`, `vitest run`, `tsc --noEmit` y `openspec validate --strict` en verde.
7. El change pasa las revisiones de `@rust-reviewer` y `@react-reviewer` y se archiva.

## Estado

- [ ] Fase 0 — change de OpenSpec presentado y aprobado.
- [ ] Fase 1 — RED del cursor.
- [ ] Fase 2 — RED del bus.
- [ ] Fase 3 — RED del endpoint de eventos.
- [ ] Fase 4 — RED del cerrojo.
- [ ] Fase 5 — GREEN del backend.
- [ ] Fase 6 — RED del frontend.
- [ ] Fase 7 — GREEN del frontend.
- [ ] Fase 8 — verificación y cierre.

---
