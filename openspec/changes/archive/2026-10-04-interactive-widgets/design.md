# Design

## Context

- El stream de chat ya transporta eventos tipados: `SSEEvent` (`src/orchestrator/agent.rs:143`) se
  serializa con `#[serde(tag = "type")]` y el frontend hace `switch (event.type)` en `useSSE.ts`.
- Existe tool-calling completo con registro (`ToolRegistry`, `src/tools/registry.rs`), guardrails
  (`src/orchestrator/guardrails.rs`) y aprobación human-in-the-loop.
- El bucle ReAct vive en `Orchestrator::process_message_stream` (`src/orchestrator/agent.rs:496`):
  ejecuta tools, emite `SSEEvent::ToolCall`/`ToolResult` e inyecta el resultado como mensaje `tool`.
- El frontend es React 18 + **antd 5** (no Tailwind). El estado del chat vive en `useMainChat` (sin
  store global). `MessageBubble` renderiza Markdown con `react-markdown`.
- Fase 1: render **en vivo**; no se reconstruyen widgets al recargar.

## Goals / Non-Goals

- **Goal:** que el modelo pueda pedir la renderización de un componente interactivo y que la acción del
  usuario vuelva al backend.
- **Goal:** que cualquier fallo (widget desconocido, `data` inválido) degrade de forma visible sin
  romper el chat.
- **Non-Goal:** reconstruir widgets al recargar; endpoint dedicado de acciones; editor de widgets;
  widgets de sistema (SystemMonitor); streaming parcial de `data`.

## Decisions

- **Widget = tool call.** Se reutiliza tool-calling, guardrails y registro en vez de inventar un canal.
  `render_widget` devuelve un `ToolResult` de acuse y el orquestador la detecta por nombre para emitir
  el evento; así el bucle ReAct sigue su curso normal.
- **Evento SSE dedicado (`type: "widget"`).** Más claro que reutilizar `tool_call` y permite un `id`
  estable por widget para correlacionar acciones.
- **Allowlist de nombres en el backend.** El modelo no puede hacer que el frontend renderice un
  componente arbitrario; el backend valida `widget_name` y el frontend vuelve a comprobar contra su
  registry.
- **Acciones como turno de usuario.** `sendWidgetAction` construye un contenido estable y lo envía por
  `/api/chat/stream`; se reutilizan pipeline, persistencia e histórico sin endpoint nuevo.
- **La tool se gestiona como el resto.** Al registrarse en `build_tool_registry`, la sincronización
  existente (`ToolsRepo::sync_from_registry`, `src/lib.rs:174`) la inserta en la tabla `tools`; por
  tanto aparece automáticamente en `GET /api/tools` y en la pestaña «Herramientas», con `enabled` que
  gobierna si se ofrece al LLM. No hace falta código nuevo de frontend para listarla.
- **Render en el flujo del mensaje.** Los widgets se asocian al mensaje del asistente en curso dentro de
  `useMainChat` y se pintan tras el Markdown. El tipo de API `Message` **no** se contamina: los widgets
  viven en un mapa aparte.
- **Aislamiento por widget (Error Boundary).** Cada widget se renderiza dentro de un
  `WidgetErrorBoundary` propio: un `data` que haga lanzar en render degrada a un `Alert` de error y no
  tumba el chat. Además, los arrays estructurales (`fields`, `options`, `items`) se validan con
  `Array.isArray` antes de recorrerlos, de modo que un `data` malformado degrada a lista vacía.
- **Bloqueo durante el stream.** Los widgets no pueden emitir acciones mientras el asistente genera la
  respuesta: `WidgetProps.disabled` fluye de `ChatView` (`streaming`) a los botones, y `sendWidgetAction`
  ignora defensivamente la acción si hay un stream activo (evita abortar el stream en curso). Por el
  mismo motivo, un evento `widget` que llegue sin stream activo se descarta.

## Contracts

### Evento SSE `widget`

```json
{ "type": "widget", "id": "<uuid>", "name": "QuickForm", "data": {} }
```

### Tool `render_widget`

Parámetros (JSON Schema):

- `widget_name` (string, enum: `["QuickForm","Checklist"]`)
- `data` (object)

Permiso: `NoConfirm`. En éxito devuelve
`ToolResult { success: true, data: { "rendered": true, "widget_name": "…" }, message: Some("…") }`.

### Interfaces TypeScript (`frontend/src/components/widgets/`)

```ts
export interface WidgetProps<T = unknown> {
  data: T;
  onAction: (action: string, payload?: unknown) => void;
  disabled?: boolean;
}
export type WidgetComponent = React.ComponentType<WidgetProps>;

export interface WidgetInstance {
  id: string;
  name: string;
  data: unknown;
}

export interface QuickFormField {
  name: string;
  label: string;
  type: "text" | "select" | "checkbox" | "slider";
  options?: string[];
  min?: number;
  max?: number;
}
export interface QuickFormData {
  title: string;
  fields: QuickFormField[];
  submit_label?: string;
}

export interface ChecklistItem {
  id: string;
  label: string;
}
export interface ChecklistData {
  title: string;
  items: ChecklistItem[];
}
```

### Envoltorio de acción

`formatWidgetAction(name, id, action, payload)` produce una única línea:

```
[widget:<name>#<id>] <action> <json-payload>
```

p. ej. `[widget:QuickForm#abc] submit {"ciudad":"Madrid"}`.

## Risks / Trade-offs

- [El modelo entrega `data` malformado] → Render defensivo: campos/ítems que no encajen se ignoran o se
  muestran como texto; arrays inválidos degradan a lista vacía; un fallo de render queda contenido por
  `WidgetErrorBoundary`; nunca revienta el chat.
- [Acción durante el stream] → Los botones se deshabilitan con `disabled` y `sendWidgetAction` ignora la
  acción si hay un stream activo, para no abortarlo ni perder el mensaje en curso.
- [Widget con `id` vacío] → Descartado en `useSSE` con aviso, evitando `key` de React colisionando.
- [Ruido en el historial] → El turno de acción se persiste como mensaje de usuario; es asumible y
  coherente con el resto.
- [Bucle modelo↔widget] → El límite de reintentos ya existente (`MAX_TOOL_RETRIES = 5`) acota el riesgo.
- [Widgets no visibles al recargar] → Aceptado en Fase 1; se evaluará en una fase posterior.

## Migration Plan

Sin migración de BD. Despliegue normal (backend + frontend) con `docker-compose.yml`.
`docker-compose.prod.yml` no se toca.
