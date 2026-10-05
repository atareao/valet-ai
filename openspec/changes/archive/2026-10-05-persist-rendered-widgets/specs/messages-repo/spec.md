# Spec Delta: messages-repo

## ADDED Requirements

### Requirement: MessagesRepo::set_widgets SHALL persistir los widgets de un mensaje

**Given** un pool de base de datos y un mensaje existente
**When** se llama `MessagesRepo::set_widgets(pool, id, widgets)` con un valor JSON
**Then** el valor SHALL almacenarse serializado en la columna `widgets` de la fila `id`
**And** las consultas de API (`find_by_id`, `list_all`) SHALL incluir la columna `widgets` y devolverla deserializada

#### Scenario: Persistir y leer widgets
**Given** un mensaje creado via `MessagesRepo::create`
**When** se llama `MessagesRepo::set_widgets(pool, &msg.id, &json!([{"id":"w1","name":"Checklist","data":{}}]))`
**Then** `find_by_id(pool, &msg.id)` devuelve un `Message` con `widgets` igual a esa lista

#### Scenario: Sobrescribir widgets es idempotente por llamada
**Given** un mensaje con widgets ya persistidos
**When** se llama de nuevo `MessagesRepo::set_widgets` con otra lista
**Then** la fila queda con la última lista, sin duplicar
