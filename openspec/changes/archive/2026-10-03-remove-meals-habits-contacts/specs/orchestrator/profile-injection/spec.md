# Spec Delta: orchestrator/profile-injection

## MODIFIED Requirements

### Requirement: Inject profile_id server-side instead of via LLM

El orquestador SHALL inyectar `profile_id` desde la sesión autenticada en los argumentos de cada tool call antes de ejecutarla.

Currently every tool requires `profile_id` as a parameter that the LLM must supply. This forces the LLM to ask the user for their profile_id, which creates confusing conversations and breaks the user experience. The fix is to inject `profile_id` from the authenticated session in the orchestrator, before tool execution.

**Contracts:**

```rust
// En agent.rs — antes de ejecutar cada tool call, inyectar profile_id

// En process_message() (no-streaming):
let mut args = tc.arguments.clone();
if let Some(obj) = args.as_object_mut() {
    obj.insert("profile_id".into(), serde_json::json!(profile_id));
}
let tool_result = self.registry.execute(&tc.name, args).await?;

// En process_message_stream() (streaming):
let mut args = tc.arguments.clone();
if let Some(obj) = args.as_object_mut() {
    obj.insert("profile_id".into(), serde_json::json!(profile_id));
}
let tool_result = match self.registry.execute(&tc.name, args).await { ... };
```

```rust
// En todas las tools — el schema ya NO incluye profile_id

// Ej: calendar.rs
fn parameters(&self) -> Value {
    serde_json::json!({
        "type": "object",
        "properties": {
            "operation": { ... },
            // ❌ profile_id eliminado
            "title": { ... },
            ...
        },
        "required": ["operation"]
    })
}
```

**Tool descriptions mejoradas (con sinónimos en español):**

| Tool | Descripción actual | Nueva descripción |
|------|--------------------|--------------------|
| calendar | "Gestión de agenda: eventos, disponibilidad y calendario" | "Agenda y calendario — eventos, citas, reuniones, cumpleaños, disponibilidad y huecos libres" |
| tasks | "Gestión de tareas: listar, crear, actualizar y completar tareas" | "Tareas por hacer — pendientes, próximas acciones, quehaceres, proyectos y cosas pendientes" |
| reminders | (actual) | "Recordatorios — alarmas, avisos, alarmas temporales, posponer y descartar" |
| knowledge | (actual) | "Notas y bloc de ideas — apuntes, información para recordar, hechos y referencias" |

**Scenarios:**

#### Scenario: LLM calls calendar without profile_id
Given the orchestrator receives a tool call for "calendar" with args `{"operation": "get_events", "start": "...", "end": "..."}`
When the orchestrator injects profile_id into the args
Then the tool receives `{"operation": "get_events", "start": "...", "end": "...", "profile_id": "profile-1"}`
And the call succeeds

#### Scenario: LLM asks user for profile_id — should NOT happen
Given a tool's parameters() schema
When inspected
Then it must NOT contain a "profile_id" property

#### Scenario: All tools have updated descriptions
Given any registered tool
When `description()` is called
Then it includes Spanish keywords and synonyms

#### Scenario: User says "gestiona mi agenda"
Given the LLM receives "gestiona mi agenda"
When it calls the calendar tool
Then the description maps "agenda" → calendar tool
And no profile_id is requested from the user
