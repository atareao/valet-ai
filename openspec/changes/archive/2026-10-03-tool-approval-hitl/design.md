# Design

## Context

- `Tool::permission(&self)` sin args (`src/tools/trait.rs:38`); `ToolRegistry::permission(name)`
  (`src/tools/registry.rs:89-91`) es su único consumidor.
- `Guardrails::check(name, args)` (`src/orchestrator/guardrails.rs:53-72`) recibe los args y **los
  descarta**: decide con `registry.permission(name)`.
- `calendar.permission()` (`src/tools/calendar.rs:295`) y `tasks.permission()`
  (`src/tools/tasks.rs:189`) devuelven `NoConfirm` incondicional.
- En `RequiresApproval`, el agente (`src/orchestrator/agent.rs:1019-1038`) emite `ApprovalRequired` y
  hace **`return Err`**; el stream muere con un evento `error`. `ApprovalResult` nunca se emite. El
  assistant con `tool_calls` y los mensajes `role:"tool"` viven solo en el `Vec<ChatMessage>` local; el
  user message sí se persiste y queda huérfano.
- `Guardrails` es un `Arc` compartido en `AppState` (producción), así que `pending_approvals` es global
  y sobrevive al stream; hoy nunca se limpia.
- Frontend: `useSSE` maneja `approval_required` y expone `onApprovalRequired`, que **ningún componente
  pasa**; `api.approveAction` existe y **nunca se llama**.

## Goals / Non-Goals

**Goals**

- Permiso por operación: el borrado de `calendar` y `tasks` exige aprobación explícita.
- La aprobación pausa y reanuda de verdad, sin romper el stream ni perder el turno.
- UI para aprobar/denegar.
- Sin fugas de solicitudes pendientes.

**Non-Goals**

- Persistir aprobaciones entre reinicios del servidor.
- Un canal de notificación nuevo para `Notify`.
- H5 (`search_places`) y H6 (timeout de `weather`).

## Decisions

### 1. `permission` recibe los argumentos (cambio de firma, no método nuevo)

`fn permission(&self, args: &Value) -> Permission`. El registry: `permission(name, args)`. Las tools
que no distinguen por operación ignoran `args`. Se actualizan los 12 tools y los mocks de test.

- **Por qué:** un único método; el tipo obliga a todo call site a disponer de los args. Un
  `permission_for` con default dejaría `permission()` sin args vivo y reabriría el mismo error.
- **`operation` ausente o desconocida → `NoConfirm`** (permiso de lectura). `execute()` ya rechaza las
  operaciones desconocidas, así que ese permiso nunca habilita un borrado.

### 2. Pausa in-process con `oneshot` (no re-emisión)

`check()` crea la solicitud y un `tokio::sync::oneshot`; guarda el `Sender` en el mapa y devuelve
`RequiresApproval { request_id }`. El agente llama `await_approval(request_id, APPROVAL_TIMEOUT)` y
espera. `resolve_approval` envía por el `oneshot`; el receiver resuelve el `await`.

- **Por qué:** el estado del turno (`Vec<ChatMessage>`, `tc.id`, args) es local al task del stream.
  Reanudar en otra petición exigiría persistir `tool_calls`/`tool_results` y un `tool_call_id` en el
  modelo `Message`, hoy inexistente, además de un store de turnos. La pausa no toca la BD.
- **Alternativa descartada:** endpoint de resume + re-emisión — mucho mayor y con cambios de esquema.

### 3. Desenlace y timeout

`ApprovalOutcome { Approved, Denied, TimedOut, Cancelled }`; `APPROVAL_TIMEOUT = 300s`.

- **Aprobado** → se ejecuta la tool y el bucle sigue como siempre.
- **Denegado / expirado / cancelado** (cliente desconectado: el `oneshot::Sender` se dropea) → se
  inyecta un mensaje `role:"tool"` indicando que la llamada no se ejecutó y que **no debe
  reintentarse**, y el ReAct loop continúa para que el LLM responda.
- **En todos los casos** se emite `SSEEvent::ApprovalResult { request_id, approved }` (`false` salvo
  aprobación).

### 4. `Notify` no bloquea

`Allowed { notify: true }` ejecuta sin aprobación; el evento `ToolCall` ya informa al cliente. No se
añade evento nuevo. Se documenta como permiso no bloqueante (hoy el flag se descarta; se mantiene).

### 5. `resolve_approval` y limpieza

`resolve_approval(id, approved)` marca `Approved`/`Denied` y envía. La entrada se conserva hasta que
el turno la consume o expira; entonces se elimina. Doble resolución → `AlreadyResolved` (409); id
desconocido → `RequestNotFound` (404). El endpoint refleja el `approved` pedido: deja de forzar `true`.

### 6. Frontend

`useMainChat` mantiene `pendingApproval { requestId, toolName, reason }`; `useSSE` cablea
`onApprovalRequired`; `ChatView` renderiza un `Modal` de antd con Permitir/Denegar que llama
`api.approveAction`. El reader del stream sigue abierto; al resolver, el flujo continúa con `chunk*` y
`done`. El estado pendiente se limpia al recibir `approval_result`/`done`/`error`.

## Risks / Trade-offs

- **[El turno en espera consume una conexión SSE]** → timeout duro de 300s; al expirar se deniega.
- **[El cambio de firma toca 12 tools + mocks]** → cambio mecánico; `cargo check` y clippy lo acotan.
- **[`pending_approvals` global compartido]** → clave UUID + limpieza explícita evitan fugas.
- **[Tests que consagran `approved: true` en el deny]** (`tests/api/chat.rs:103-120`) → se actualizan.
- **[Proxies podrían cortar una pausa larga]** → timeout acotado y documentado.

## Migration Plan

Sin migración de esquema. Rollback: revertir el código; `calendar`/`tasks` vuelven a `NoConfirm` y la
aprobación deja de pausar.

## Open Questions

- Valor del timeout (300s) y si debe venir de `settings`: se fija constante; ajustable en otro change.
