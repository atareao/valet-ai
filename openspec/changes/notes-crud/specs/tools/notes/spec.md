## REMOVED Requirements

### Requirement: NotesTool SHALL manage notes through three operations

Eliminado: sustituido por «NotesTool SHALL manage notes through four operations», que añade
`update_note` y la comprobación de pertenencia al perfil en `update_note` y `delete_note`. Los
escenarios del requisito eliminado se conservan en el requisito nuevo.

## ADDED Requirements

### Requirement: NotesTool SHALL manage notes through four operations

La tool `notes` SHALL despachar por `operation` y soportar `create_note`, `list_notes`,
`update_note` y `delete_note` sobre la tabla `notes` a través de `NotesRepo`. `create_note` SHALL
usar `profile_id` por defecto `default` y categoría por defecto `idea`; `list_notes` SHALL permitir
filtrar por `category`. El enum de `operation` en `parameters()` SHALL ser
`["create_note", "list_notes", "update_note", "delete_note"]` y su descripción SHALL indicar los
obligatorios de cada acción (`content` para `create_note`; `id` para `update_note` y `delete_note`;
y que `update_note` exige además al menos uno de `content`, `category` o `tags`). Las categorías
ofrecidas en `parameters()` SHALL ser `idea`, `journal` y `fact`; la categoría `todo` queda fuera
del enum y de la descripción por solapamiento con la tool `tasks`.

`update_note` SHALL aceptar `id` (obligatorio y no vacío) y, opcionalmente, `content`, `category` y
`tags`, y SHALL rechazar con `ToolError::InvalidArguments` un `id` vacío, la ausencia de los tres
campos opcionales y una `category` fuera de las tres permitidas. Un `content` vacío SHALL rechazarse.
`tags: ""` SHALL vaciar las etiquetas de la nota. `update_note` y `delete_note` SHALL comprobar que
la nota pertenece al `profile_id` de la llamada: una nota inexistente o de otro perfil SHALL producir
un error de «nota no encontrada» **sin** escribir en la base de datos.

#### Scenario: create_note usa valores por defecto

- **Given** una `NotesTool`
- **When** se ejecuta con `{"operation":"create_note","content":"Una idea"}`
- **Then** DEBE crear la nota con `profile_id` `default` y categoría `idea`
- **And** DEBE devolver la nota creada

#### Scenario: list_notes filtra por categoría

- **Given** una `NotesTool` con notas de varias categorías
- **When** se ejecuta con `{"operation":"list_notes","category":"journal"}`
- **Then** DEBE devolver solo las notas de esa categoría

#### Scenario: update_note cambia el contenido

- **Given** una nota existente del perfil
- **When** se ejecuta con `{"operation":"update_note","id":"<id>","content":"Corregido"}`
- **Then** el `content` SHALL quedar persistido como `Corregido`
- **And** SHALL devolver la nota actualizada

#### Scenario: update_note exige id y al menos un campo

- **Given** una `NotesTool`
- **When** se ejecuta `update_note` sin `id`, o con `id` y ninguno de `content`/`category`/`tags`
- **Then** SHALL devolver `ToolError::InvalidArguments`

#### Scenario: update_note rechaza categoría y contenido inválidos

- **Given** una nota existente
- **When** se ejecuta `update_note` con `{"operation":"update_note","id":"<id>","category":"otra"}`, o con `content` vacío
- **Then** SHALL devolver `ToolError::InvalidArguments`
- **And** la nota SHALL quedar intacta

#### Scenario: update_note puede vaciar las etiquetas

- **Given** una nota con etiquetas
- **When** se ejecuta con `{"operation":"update_note","id":"<id>","tags":""}`
- **Then** las etiquetas de la nota SHALL quedar vacías

#### Scenario: delete_note borra por id

- **Given** una nota existente del perfil
- **When** se ejecuta con `{"operation":"delete_note","id":"<id>"}`
- **Then** DEBE borrarla vía `NotesRepo::delete` y devolver `{id}`

#### Scenario: update_note y delete_note no actúan sobre notas de otro perfil

- **Given** una nota cuyo `profile_id` no es el de la llamada
- **When** se ejecuta `update_note` (o `delete_note`) con su `id` desde otro perfil
- **Then** SHALL devolver un error de «nota no encontrada»
- **And** la nota SHALL seguir existiendo sin cambios

#### Scenario: La categoría todo no se ofrece y la operación documenta los obligatorios

- **Given** la definición de la tool `notes`
- **When** se inspecciona el enum de `category` y la descripción de `operation`
- **Then** el enum de `category` contiene `idea`, `journal` y `fact` y NO contiene `todo`
- **And** el enum de `operation` contiene las cuatro acciones, incluida `update_note`
- **And** la descripción de `operation` menciona `content` para `create_note` e `id` para
  `update_note`/`delete_note`

## MODIFIED Requirements

### Requirement: NotesTool SHALL be NoConfirm for every operation

La tool `notes` SHALL declarar `NoConfirm` para todas sus operaciones, incluidas `update_note` y
`delete_note` (asimetría intencionada o pendiente de revisar frente a `tasks.delete_task`, que sí
exige aprobación explícita).

**Given** una `NotesTool`
**When** se consulta `permission()` para cualquier `operation`
**Then** DEBE devolver `NoConfirm`

#### Scenario: Permiso NoConfirm en todas las operaciones
**Given** una `NotesTool`
**When** la `operation` es `create_note`, `list_notes`, `update_note` o `delete_note`
**Then** `permission()` DEBE devolver `NoConfirm`
