# Design

## Context

- El orquestador emite `SSEEvent::Widget { id, name, data }` con `id = Uuid::new_v4()` al ejecutar con
  éxito `render_widget` (`agent.rs`), pero el `id` se descarta y el mensaje final se persiste con
  `tool_calls: None`.
- `Message` serializa `tool_calls`/`tool_results`; el frontend ya recibe el historial por
  `GET /api/chat/init` (50 mensajes) y `GET /api/messages`.
- El frontend asocia widgets por id de mensaje en `widgetsByMessage` (solo en vivo hoy).
- No se persiste el estado de interacción (Fase 1 no lo tenía).

## Goals / Non-Goals

- **Goal:** que al recargar, los widgets renderizados en cada mensaje del asistente vuelvan a aparecer,
  interactivos y con estado fresco.
- **Non-Goal:** persistir estado de interacción; reconstruir widgets en flujos que no sean el historial;
  cambiar el formato `[widget:Name#id] action payload`.

## Decisions

- **Columna dedicada `widgets TEXT` (JSON), no `tool_calls`.** Guardar el widget como tool_call del
  mensaje final sería semánticamente falso (el mensaje final no llamó a la tool) y podría romper el
  contexto del LLM (tool_call huérfano). Una columna aparte no interfiere con el contexto.
- **`set_widgets(id, widgets)` en vez de tocar `create`.** `MessagesRepo::create` tiene 35 call sites;
  se añade un UPDATE específico tras crear el mensaje del asistente. Menos churn y más claro.
- **El `id` del widget se genera una vez y se reutiliza.** El mismo `id` del evento SSE se persiste, así
  que en recarga la acción `[widget:Name#id]` sigue siendo coherente.
- **Solo widgets con éxito.** Se persisten los mismos que emiten el evento (los que pasan la allowlist).
- **Reconstrucción sin duplicar.** En vivo, el widget se mueve de `streaming` al mensaje en `onDone`; al
  cargar historial se puebla desde `message.widgets`. Son caminos disjuntos (carga vs stream).

## Contracts

### Columna y payload

```sql
-- migrations/20261004000002_message_widgets.sql
ALTER TABLE messages ADD COLUMN widgets TEXT;
```

```jsonc
// messages.widgets
[{ "id": "uuid", "name": "LocationWidget", "data": { /* … */ } }]
```

### API

`MessagesRepo::set_widgets(pool, id, widgets: &Value) -> Result<(), sqlx::Error>` (UPDATE).
`Message.widgets: Option<Value>` (serializado por serde en `chat_init` y `list_messages`).

### Frontend

`Message.widgets?: WidgetInstance[] | null`; al cargar, `widgetsByMessage[msg.id] = msg.widgets`.

## Risks / Trade-offs

- [N+1 al cargar] → Es parte del SELECT de mensajes, sin consultas extra.
- [Widgets de turnos antiguos con datos obsoletos] → Es lo deseado: reflejan lo que el asistente mostró.
- [Crecimiento de la fila] → El JSON de widgets es pequeño y acotado.

## Migration Plan

Migración aditiva e idempotente (`ALTER TABLE … ADD COLUMN`). Sin rollback destructivo.
`docker-compose.prod.yml` no se toca.
