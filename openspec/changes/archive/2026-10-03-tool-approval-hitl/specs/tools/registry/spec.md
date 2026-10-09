# Spec Delta: tools/registry

## Purpose

El catálogo de herramientas que el orquestador de Valet ofrece al LLM: qué herramientas existen,
cuáles se exponen en cada petición y en qué condiciones se permite su ejecución.

## ADDED Requirements

### Requirement: La interfaz Tool SHALL declarar el permiso en función de los argumentos

`Tool::permission(&self, args: &Value) -> Permission` SHALL recibir los argumentos de la llamada para
que las tools multioperación distingan la operación. El registry SHALL exponer
`permission(name, args)`. Las tools que no distinguen por operación SHALL ignorar `args` y devolver
siempre el mismo permiso.

**Contracts:**

```rust
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn parameters(&self) -> Value;
    fn permission(&self, args: &Value) -> Permission; // antes: sin argumentos
    async fn execute(&self, args: Value) -> Result<ToolResult, ToolError>;
}

impl ToolRegistry {
    pub fn permission(&self, name: &str, args: &Value) -> Option<Permission>;
}
```

#### Scenario: Una operación de lectura no requiere confirmación

**Given** un registry con una tool multioperación
**When** se consulta su permiso con unos args que seleccionan una operación de lectura
**Then** el permiso es `NoConfirm`

#### Scenario: Una operación de borrado requiere confirmación

**Given** un registry con una tool multioperación
**When** se consulta su permiso con unos args que seleccionan una operación de borrado
**Then** el permiso es `ExplicitApproval`

#### Scenario: Una tool sin operaciones mantiene su permiso

**Given** un registry con `web_search`, que no distingue operaciones
**When** se consulta su permiso con args cualesquiera
**Then** el permiso es el mismo que declaraba, con independencia de los args

### Requirement: Las operaciones destructivas SHALL requerir aprobación explícita

Toda operación de borrado de una tool multioperación SHALL declarar `ExplicitApproval`. En concreto,
la tool `tasks` SHALL devolver `ExplicitApproval` para `delete_task` y `NoConfirm` para el resto de
sus operaciones.

#### Scenario: delete_task requiere aprobación explícita

**Given** un registry con la tool `tasks`
**When** se consulta su permiso con `{"operation": "delete_task", "id": "..."}`
**Then** el permiso es `ExplicitApproval`

#### Scenario: list_tasks no requiere aprobación

**Given** un registry con la tool `tasks`
**When** se consulta su permiso con `{"operation": "list_tasks"}`
**Then** el permiso es `NoConfirm`
