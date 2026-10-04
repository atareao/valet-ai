# Proposal

## Why

La interfaz de Valet es puramente conversacional: el asistente solo responde texto Markdown y la única
vía de vuelta del usuario al backend es un mensaje de chat o la aprobación de una tool destructiva.
Cuando el modelo necesita una decisión con varios parámetros en una sola pregunta, o quiere entregar un
plan accionable que el usuario vaya marcando, se ve obligado a enumerarlo en prosa: funciona, pero se
lee y se responde mal.

El transporte ya está preparado: el stream de chat es SSE tipado (`SSEEvent` con tag `"type"`,
`src/orchestrator/agent.rs:143`) y el frontend ya despacha por tipo de evento (`useSSE.ts`). Además
existe tool-calling completo con registro, guardrails y aprobación. Basta añadir una tool que pida
renderizar un componente y un evento SSE que lo transporte para dar el salto a una UI generativa, sin
inventar transporte ni endpoint nuevos.

## What Changes

- **`tools/registry` (ADDED)**: se registra la tool `render_widget`, que el modelo invoca para pedir un
  widget interactivo; valida `widget_name` contra una lista permitida (`QuickForm`, `Checklist`) y no
  requiere aprobación. Como cualquier otra tool, se sincroniza con la tabla `tools`, aparece en
  `GET /api/tools` y se gestiona (habilitar/deshabilitar) desde la pestaña «Herramientas».
- **`orchestrator/agent` (ADDED)**: cuando el bucle ReAct ejecuta `render_widget` con éxito, el
  orquestador emite un `SSEEvent` de tipo `widget` (`id`, `name`, `data`) y sigue inyectando el
  resultado de la tool en el bucle, de modo que el turno no se rompe.
- **`frontend` (ADDED)**: registry de componentes, `WidgetRenderer` (degrada a un aviso con nombres
  desconocidos), manejo del evento `widget` en el stream, y dos widgets (`QuickForm` y `Checklist`)
  cuyas acciones vuelven al backend como un turno de usuario por el chat existente.

Alcance de la Fase 1: los widgets se renderizan **en vivo** durante el stream; no se reconstruyen al
recargar la conversación.

## Capabilities

### New Capabilities

- Ninguna.

### Modified Capabilities

- `tools/registry` (se añaden requisitos y se renombra el del catálogo de herramientas para no
  hardcodear el conteo).
- `orchestrator/agent` (se añade un requisito).
- `frontend` (se añaden requisitos).

## Impact

- **Backend**: `src/tools/` (nueva tool `render_widget`), `src/tools/mod.rs` y `src/lib.rs` (registro),
  los espejos del catálogo en tests (`src/db/repos/tools.rs`, `tests/api/tools.rs`) y
  `src/orchestrator/agent.rs` (variante nueva de `SSEEvent` e intercepción). Sin migraciones de BD.
- **Frontend**: `frontend/src/types/index.ts`, `hooks/useSSE.ts`, `hooks/useMainChat.ts`,
  `components/widgets/` (nuevos), `components/MessageBubble.tsx` y `components/ChatView.tsx`. Sin
  dependencias nuevas (antd 5 ya está).
- **No** se toca `docker-compose.prod.yml` ni la configuración de despliegue.
