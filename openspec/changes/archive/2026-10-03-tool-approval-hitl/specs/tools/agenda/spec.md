# Spec Delta: tools/agenda

## Purpose

Tool de agenda de Valet: operaciones de eventos (crear, actualizar, eliminar, listar por categoría),
esquema unificado basado en `start` + `duration`, soporte de campos nuevos y permiso de aprobación
explícita para el borrado.

## MODIFIED Requirements

### Requirement: Calendar tool — change delete_event permission to ExplicitApproval

La tool `calendar` SHALL declarar el permiso **por operación**, evaluando el parámetro `operation` de
los argumentos: las operaciones de lectura SHALL ser `NoConfirm`, la creación y la edición SHALL ser
`Notify`, y el borrado SHALL ser `ExplicitApproval`.

Eliminar eventos es destructivo — debe requerir confirmación del usuario antes de ejecutarse.

```rust
fn permission(&self, args: &Value) -> Permission {
    match args.get("operation").and_then(|v| v.as_str()) {
        // get_events, check_availability, list_by_category → NoConfirm
        // create_event, update_event → Notify
        // delete_event → ExplicitApproval
    }
}
```

**Scenarios:**

#### Scenario: Delete event requires explicit approval

**When** el LLM llama a `delete_event`
**Then** el guardrail devuelve `ExplicitApproval`
**And** el orquestador pausa para pedir confirmación al usuario
**And** solo ejecuta el borrado si el usuario aprueba

#### Scenario: Read operations do not require approval

**Given** una llamada a `get_events`, `check_availability` o `list_by_category`
**When** el guardrail consulta el permiso con esos argumentos
**Then** el permiso es `NoConfirm`

#### Scenario: Create and update are allowed with notification

**Given** una llamada a `create_event` o `update_event`
**When** el guardrail consulta el permiso con esos argumentos
**Then** el permiso es `Notify`
**And** la operación se ejecuta sin aprobación explícita
