# Proposal

## Why

En la Fase 1, los widgets interactivos eran **solo en vivo**: el `render_widget` se ejecuta y se emite
por SSE, pero el mensaje del asistente se persiste con `tool_calls: None` y **sin rastro del widget**. Al
recargar la página, los widgets desaparecen porque no hay nada en la BD que los reconstruya.

## What Changes

- **Persistencia**: el mensaje del asistente guarda los widgets renderizados en ese turno
  (`[{ id, name, data }]`) en una columna nueva `widgets` (JSON) de `messages`.
- **`message` (ADDED)**: el modelo `Message` gana el campo opcional `widgets`.
- **`messages-repo` (ADDED)**: `MessagesRepo::set_widgets` persiste el JSON; las consultas de API
  devuelven la columna.
- **`orchestrator/agent` (ADDED)**: el orquestador recoge los widgets emitidos durante el turno y los
  persiste con el mensaje final del asistente.
- **`frontend` (ADDED)**: al cargar el historial, el frontend reconstruye los widgets desde
  `message.widgets` y los vuelve a pintar.

**Fuera de alcance:** persistir el **estado de interacción** de un widget (casillas marcadas, valores
introducidos). Al recargar, el widget se reconstruye **interactivo con estado fresco**.

## Capabilities

### Modified Capabilities

- `message`, `messages-repo`, `orchestrator/agent`, `frontend` (cada una con un requisito añadido).

## Impact

- **Backend**: migración `..._message_widgets.sql`; `src/models/message.rs`;
  `src/db/repos/messages.rs` (SELECTs + `set_widgets`); `src/orchestrator/agent.rs`.
- **Frontend**: `src/types/index.ts` (`Message.widgets`); `src/hooks/useMainChat.ts` (reconstrucción).
- **No** se toca `docker-compose.prod.yml` ni el formato de las acciones de widget.
