# Spec Delta: message

## ADDED Requirements

### Requirement: Message model SHALL include optional widgets field

**Given** un mensaje almacenado en la base de datos
**When** se recupera via API
**Then** el campo `widgets` SHALL ser `Option<Value>` (JSON)
**And** SHALL ser `null` cuando el mensaje no mostró widgets
**And** SHALL contener la lista `[{ id, name, data }]` de los widgets renderizados por el asistente cuando los haya

#### Scenario: set_widgets persiste la lista de widgets
**Given** un mensaje assistant recién creado
**When** se llama `MessagesRepo::set_widgets(id, [{ id: "w1", name: "LocationWidget", data: { latitude: 1.0, longitude: 2.0 } }])`
**Then** `find_by_id(id)` devuelve `widgets` con esa lista

#### Scenario: widgets es null cuando no hay
**Given** un mensaje sin widgets
**When** se recupera
**Then** `widgets` SHALL ser `null`

#### Scenario: Migration añade columna widgets idempotentemente
**Given** una base de datos con tabla `messages` existente
**When** `run_migrations()` se ejecuta
**Then** la tabla SHALL tener columna `widgets TEXT` nullable
**And** ejecutar migrations dos veces SHALL NO fallar

#### Scenario: LIST devuelve widgets
**Given** un mensaje con `widgets = [{ id: "w1", name: "Checklist", data: {} }]`
**When** `list_all()` es llamado
**Then** el mensaje devuelto SHALL incluir esa lista en `widgets`
