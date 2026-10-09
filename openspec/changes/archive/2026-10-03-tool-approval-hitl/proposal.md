# Change: Aprobación humana (HITL) real y permisos por operación

## Why

El sistema de permisos de herramientas es de grano grueso: `Tool::permission(&self)` **no recibe los
argumentos**, así que las tools multioperación (`calendar`, `tasks`) declaran un único permiso para
todas sus operaciones. Consecuencia: `delete_event` y `delete_task` —borrados irreversibles— se
ejecutan sin confirmación. La spec vigente `tools/agenda` ya exige `ExplicitApproval` para
`delete_event`, y el código la incumple. No es una mejora: es una discrepancia spec↔código.

Además, la maquinaria de aprobación está a medio construir y **no reanuda**: cuando el guardrail
responde `RequiresApproval`, el orquestador emite `approval_required` y acto seguido hace
`return Err(...)`, cortando el stream. El endpoint `POST /api/approval/{id}` cambia un estado en
memoria que nadie relee, `ApprovalResult` nunca se emite y el frontend no tiene UI. Activar
`ExplicitApproval` sin arreglar esto dejaría los borrados imposibles: ni se confirman ni se ejecutan.

## What Changes

- **Permiso por operación.** `Tool::permission(&self, args: &Value)` recibe los argumentos; el registry
  expone `permission(name, args)`. `calendar` distingue lectura (`NoConfirm`), creación/edición
  (`Notify`) y borrado (`ExplicitApproval`); `tasks` declara `ExplicitApproval` para `delete_task`.
- **Pausa y reanudación reales.** Ante `RequiresApproval`, el orquestador emite `ApprovalRequired`,
  **espera** la decisión (con timeout) y continúa: aprobada → ejecuta la tool; denegada, expirada o
  cancelada → inyecta un rechazo al LLM y sigue el bucle. No hay `return Err`.
- **`approval_result` emitido.** El stream informa del desenlace y el turno termina con `Done` como
  cualquier otro.
- **Aprobaciones sin fugas.** La solicitud se resuelve vía `POST /api/approval/{id}` (200/404/409) y se
  elimina tras consumirse o expirar.
- **UI de confirmación.** El chat muestra la petición y ofrece Permitir/Denegar, manteniendo el stream
  abierto para continuar la respuesta.

## Capabilities

### New Capabilities
- `orchestrator/guardrails`: el guardrail que decide si una llamada se ejecuta y el ciclo de aprobación
  humana (solicitud, resolución, espera con timeout y limpieza).

### Modified Capabilities
- `tools/registry`: la interfaz `Tool` y el registry resuelven el permiso con los argumentos; regla de
  operaciones destructivas.
- `tools/agenda`: `calendar` declara permiso por operación (borrado = `ExplicitApproval`).
- `tools/web_search`: su requirement de interfaz se actualiza a la firma con argumentos.
- `orchestrator/agent`: la aprobación explícita pausa y reanuda el turno sin romper el stream.
- `stream-route`: `POST /api/approval/{request_id}` resuelve y señala al turno en espera.
- `frontend`: el chat pide confirmación para herramientas destructivas.

## Impact

- Backend: `src/tools/trait.rs`, `src/tools/registry.rs`, los 12 tools (firma de `permission`),
  `src/orchestrator/guardrails.rs`, `src/orchestrator/agent.rs`, `src/routes/stream.rs`.
- Frontend: `frontend/src/hooks/useSSE.ts`, `frontend/src/hooks/useMainChat.ts`,
  `frontend/src/components/ChatView.tsx` y un diálogo de aprobación nuevo.
- Contrato SSE: `approval_required` y `approval_result` pasan a usarse de verdad; `tool_call` y
  `tool_result` no cambian.
- Tests: guardrails (permiso por operación), agent (pausa/reanudación end-to-end), `tests/api/chat.rs`
  (el deny deja de devolver `approved: true`) y frontend (diálogo).
- Sin migración de datos ni cambios de esquema. `docker-compose.prod.yml` no se toca.

### Fuera de alcance

- H5 (`search_places` con `searchText`) y H6 (timeout de `weather`): changes aparte.
- Persistencia entre reinicios de aprobaciones pendientes: la aprobación vive una sesión; si el turno
  expira o el servidor cae, la operación no se ejecuta.
- Un canal de notificación nuevo para `Notify`: se mantiene no bloqueante (lo informa `ToolCall`).
