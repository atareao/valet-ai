# Spec Delta: tools/notes

## ADDED Requirements

### Requirement: NotesTool SHALL manage notes through three operations

La tool `notes` SHALL despachar por `operation` y soportar `create_note`, `list_notes` y
`delete_note` sobre la tabla `notes` a través de `NotesRepo`. `create_note` SHALL usar `profile_id`
por defecto `default` y categoría por defecto `idea`; `list_notes` SHALL permitir filtrar por
`category`.

**Given** una `NotesTool` con acceso a la BD
**When** se ejecuta `notes` con una `operation`
**Then** DEBE operar sobre `NotesRepo` y devolver datos serializados

#### Scenario: create_note usa valores por defecto
**Given** una `NotesTool`
**When** se ejecuta con `{"operation":"create_note","content":"Una idea"}`
**Then** DEBE crear la nota con `profile_id` `default` y categoría `idea`
**And** DEBE devolver la nota creada

#### Scenario: list_notes filtra por categoría
**Given** una `NotesTool` con notas de varias categorías
**When** se ejecuta con `{"operation":"list_notes","category":"journal"}`
**Then** DEBE devolver solo las notas de esa categoría

#### Scenario: delete_note borra por id
**Given** una nota existente
**When** se ejecuta con `{"operation":"delete_note","id":"<id>"}`
**Then** DEBE borrarla vía `NotesRepo::delete` y devolver `{id}`

### Requirement: NotesTool SHALL be NoConfirm for every operation

La tool `notes` SHALL declarar `NoConfirm` para todas sus operaciones, incluida `delete_note`
(asimetría intencionada o pendiente de revisar frente a `tasks.delete_task`, que sí exige
aprobación explícita).

**Given** una `NotesTool`
**When** se consulta `permission()` para cualquier `operation`
**Then** DEBE devolver `NoConfirm`

#### Scenario: Permiso NoConfirm en todas las operaciones
**Given** una `NotesTool`
**When** la `operation` es `create_note`, `list_notes` o `delete_note`
**Then** `permission()` DEBE devolver `NoConfirm`
