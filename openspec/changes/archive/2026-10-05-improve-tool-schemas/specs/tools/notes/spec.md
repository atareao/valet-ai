# Spec Delta

## MODIFIED Requirements

### Requirement: NotesTool SHALL manage notes through three operations

La tool `notes` SHALL despachar por `operation` y soportar `create_note`, `list_notes` y `delete_note` sobre la tabla `notes` a través de `NotesRepo`. `create_note` SHALL usar `profile_id` por defecto `default` y categoría por defecto `idea`; `list_notes` SHALL permitir filtrar por `category`. Las categorías ofrecidas en `parameters()` SHALL ser `idea`, `journal` y `fact`; la categoría `todo` queda fuera del enum y de la descripción por solapamiento con la tool `tasks`. La descripción del parámetro `operation` SHALL indicar que `content` es obligatorio para `create_note` y `id` para `delete_note`.

#### Scenario: create_note usa valores por defecto

- **Given** una `NotesTool`
- **When** se ejecuta con `{"operation":"create_note","content":"Una idea"}`
- **Then** DEBE crear la nota con `profile_id` `default` y categoría `idea`
- **And** DEBE devolver la nota creada

#### Scenario: list_notes filtra por categoría

- **Given** una `NotesTool` con notas de varias categorías
- **When** se ejecuta con `{"operation":"list_notes","category":"journal"}`
- **Then** DEBE devolver solo las notas de esa categoría

#### Scenario: delete_note borra por id

- **Given** una nota existente
- **When** se ejecuta con `{"operation":"delete_note","id":"<id>"}`
- **Then** DEBE borrarla vía `NotesRepo::delete` y devolver `{id}`

#### Scenario: La categoría todo no se ofrece y la operación documenta los obligatorios

- **Given** la definición de la tool `notes`
- **When** se inspecciona el enum de `category` y la descripción de `operation`
- **Then** el enum contiene `idea`, `journal` y `fact` y NO contiene `todo`
- **And** la descripción de `operation` menciona `content` para `create_note` e `id` para `delete_note`
