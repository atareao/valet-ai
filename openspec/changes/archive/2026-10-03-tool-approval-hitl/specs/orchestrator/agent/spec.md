# Spec Delta: orchestrator/agent

## Purpose

Bucle ReAct del agente de Valet: uso del system prompt desde la base de datos, tolerancia a errores de
herramientas, límite de reintentos, entrega incremental vía chat_stream, prioridad de los tool_calls
del evento Done y registro de estadísticas de cada llamada al LLM.

## ADDED Requirements

### Requirement: La aprobación explícita SHALL pausar y reanudar el turno sin romper el stream

Ante un `GuardrailResult::RequiresApproval`, el orquestador SHALL emitir `SSEEvent::ApprovalRequired`
y SHALL esperar la decisión como máximo `APPROVAL_TIMEOUT`. SHALL NOT devolver error ni cerrar el
stream mientras espera.

- **Aprobada:** SHALL ejecutar la herramienta y tratarla como cualquier otro tool call.
- **Denegada, expirada o cancelada:** SHALL inyectar un mensaje `role:"tool"` indicando que la llamada
  no se ejecutó y que no debe reintentarse, y SHALL continuar el bucle ReAct para que el LLM responda.
- **En todos los casos:** SHALL emitir `SSEEvent::ApprovalResult { request_id, approved }`, con
  `approved: true` solo si hubo aprobación.
- El mensaje assistant final SHALL persistirse como en un turno normal, y el turno SHALL terminar con
  `SSEEvent::Done`.

**Contract:**

```rust
const APPROVAL_TIMEOUT: Duration = Duration::from_secs(300);

// Antes: return Err(AgentError::GuardrailError(...)) al recibir RequiresApproval.
// Ahora: emitir ApprovalRequired, await_approval, decidir y continuar el bucle.
```

#### Scenario: Aprobar ejecuta la herramienta y termina el turno

**Given** un orquestador cuyo LLM pide una herramienta `ExplicitApproval`
**When** el usuario aprueba la solicitud
**Then** la herramienta se ejecuta
**And** se emite `ApprovalResult { approved: true }`
**And** el stream continúa y termina con `Done`
**And** el mensaje assistant se persiste

#### Scenario: Denegar no ejecuta y el LLM responde

**Given** un orquestador cuyo LLM pide una herramienta `ExplicitApproval`
**When** el usuario deniega la solicitud
**Then** la herramienta NO se ejecuta
**And** se emite `ApprovalResult { approved: false }`
**And** el LLM recibe un mensaje `role:"tool"` de rechazo
**And** el bucle continúa y el stream termina con `Done`

#### Scenario: El timeout se comporta como denegación

**Given** un orquestador esperando una aprobación
**When** se agota `APPROVAL_TIMEOUT` sin decisión
**Then** la herramienta NO se ejecuta
**And** se emite `ApprovalResult { approved: false }`
**And** el stream continúa y termina con `Done`

#### Scenario: Una aprobación no rompe el stream con error

**Given** un orquestador cuyo LLM pide una herramienta `ExplicitApproval`
**When** el guardrail devuelve `RequiresApproval`
**Then** NO se emite un `SSEEvent::Error` por este motivo
**And** el turno queda a la espera de la decisión
