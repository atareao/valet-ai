# orchestrator/guardrails Specification

## Purpose
El guardrail que decide si una llamada a herramienta se ejecuta y gestiona el ciclo de aprobación
humana (HITL): consulta del permiso con los argumentos reales, solicitud de aprobación, resolución por
el usuario, espera acotada del turno y limpieza de las solicitudes.

## Requirements

### Requirement: El guardrail SHALL resolver el permiso con los argumentos de la llamada

`Guardrails::check(tool_name, args)` SHALL consultar `registry.permission(tool_name, args)` — con los
argumentos reales — y SHALL devolver `Allowed { notify: false }` para `NoConfirm`,
`Allowed { notify: true }` para `Notify` y `RequiresApproval { request_id }` para
`ExplicitApproval`. Un tool desconocido SHALL devolver `GuardrailError::ToolNotFound`.

**Contract:**

```rust
pub fn check(&self, tool_name: &str, args: &Value)
    -> Result<GuardrailResult, GuardrailError>;
```

#### Scenario: NoConfirm se permite sin aviso

**Given** un guardrail con una tool `NoConfirm`
**When** se comprueba la llamada
**Then** el resultado es `Allowed { notify: false }`

#### Scenario: Notify se permite con aviso

**Given** un guardrail con una tool `Notify`
**When** se comprueba la llamada
**Then** el resultado es `Allowed { notify: true }`

#### Scenario: ExplicitApproval crea una solicitud pendiente

**Given** un guardrail con una tool `ExplicitApproval`
**When** se comprueba la llamada con sus argumentos
**Then** el resultado es `RequiresApproval { request_id }`
**And** existe una solicitud pendiente con ese id, el nombre del tool y los argumentos

#### Scenario: Un tool desconocido no se permite

**Given** un guardrail sin la tool `x`
**When** se comprueba `check("x", args)`
**Then** se devuelve `GuardrailError::ToolNotFound`

### Requirement: La solicitud de aprobación SHALL conservar la llamada y un canal de respuesta

La solicitud SHALL guardar el nombre del tool, sus argumentos y un estado
(`Pending`/`Approved`/`Denied`), y SHALL disponer de un canal para señalizar la decisión al turno en
espera. `resolve_approval(id, approved)` SHALL marcar el estado y señalizar; SHALL devolver
`RequestNotFound` si el id no existe y `AlreadyResolved` si ya estaba resuelta y la entrada sigue registrada.

**Contract:**

```rust
pub fn resolve_approval(&self, request_id: &str, approved: bool)
    -> Result<(), GuardrailError>;
```

#### Scenario: Resolver una solicitud aprueba

**Given** una solicitud pendiente con id `req-1`
**When** `resolve_approval("req-1", true)`
**Then** el estado pasa a `Approved` y la espera recibe `true`

#### Scenario: Resolver una solicitud deniega

**Given** una solicitud pendiente con id `req-1`
**When** `resolve_approval("req-1", false)`
**Then** el estado pasa a `Denied` y la espera recibe `false`

#### Scenario: Id desconocido devuelve error

**When** `resolve_approval("no-existe", true)`
**Then** se devuelve `GuardrailError::RequestNotFound`

#### Scenario: Doble resolución devuelve error

**Given** una solicitud ya resuelta
**When** se vuelve a resolver
**Then** se devuelve `GuardrailError::AlreadyResolved`

### Requirement: El turno SHALL poder esperar la decisión con un timeout acotado

El guardrail SHALL exponer una espera asíncrona que resuelva en
`Approved`, `Denied`, `TimedOut` o `Cancelled`. `TimedOut` SHALL producirse al agotarse el plazo sin
decisión; `Cancelled` SHALL producirse si el turno que esperaba desaparece. Tras consumirse la
decisión, la solicitud SHALL eliminarse del registro.

**Contract:**

```rust
pub enum ApprovalOutcome { Approved, Denied, TimedOut, Cancelled }

pub async fn await_approval(&self, request_id: &str, timeout: Duration)
    -> ApprovalOutcome;
```

#### Scenario: La aprobación reanuda la espera

**Given** una solicitud pendiente y un turno esperando
**When** el usuario aprueba
**Then** la espera resuelve `Approved`

#### Scenario: Sin decisión, la espera expira

**Given** una solicitud pendiente y un turno esperando
**When** transcurre el timeout sin resolución
**Then** la espera resuelve `TimedOut`

#### Scenario: La solicitud se limpia tras consumirse

**Given** una solicitud resuelta y consumida por el turno
**Then** la solicitud ya no figura entre las pendientes
