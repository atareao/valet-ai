# tools/journal Specification

## Purpose

La herramienta `journal` del agente de Valet: lee, escribe, corrige, lista y borra las entradas de la
bitácora personal (`journal_entries`), con el día como clave y aprobación explícita solo para el
borrado.

## ADDED Requirements

### Requirement: La herramienta `journal` SHALL despachar cinco operaciones sobre la bitácora

La herramienta `journal` SHALL exponer las operaciones `get_entry`, `write_entry`, `update_entry`,
`delete_entry` y `list_entries`, delegan en `JournalRepo` y trabajan sobre la tabla
`journal_entries`. `get_entry` SHALL devolver la entrada de la `date` indicada o, si se omite, la del
**día actual**; si ese día no tiene entrada, SHALL informar de que no existe sin fallar. `write_entry`
SHALL crear o **sobrescribir** la entrada del día (el «apúntame esto»): `date` y `content` son
obligatorios y `mood`, `achievements`, `struggles` y `tags` son opcionales. `update_entry` SHALL ser
**parcial**: SHALL modificar solo los campos que le lleguen y SHALL dejar intactos los demás.
`delete_entry` SHALL borrar la entrada de la `date` y, si no existía, SHALL informar de que no existe
sin fallar. `list_entries` SHALL devolver las entradas de un rango de fechas `[from, to]` de más
reciente a más antigua, acotadas por `limit`. Una fecha que no encaje en `YYYY-MM-DD` o un `mood`
fuera del conjunto cerrado SHALL devolver `ToolError::InvalidArguments` sin tocar la base de datos.
El `profile_id` SHALL ser opcional con valor por defecto `default`, como en las herramientas vecinas.

**Contracts:**

```rust
// src/tools/journal.rs
pub struct JournalTool { db: SqlitePool }

// parameters()
{
  "operation": "get_entry" | "write_entry" | "update_entry" | "delete_entry" | "list_entries",
  "date": "YYYY-MM-DD",           // obligatorio en write_entry/update_entry/delete_entry; opcional en get_entry (hoy por defecto)
  "content": "string",            // obligatorio en write_entry
  "mood": "great" | "good" | "neutral" | "bad" | "terrible",  // opcional
  "achievements": ["string"],     // opcional
  "struggles": ["string"],        // opcional
  "tags": ["string"],             // opcional
  "from": "YYYY-MM-DD",           // obligatorio en list_entries
  "to": "YYYY-MM-DD",             // obligatorio en list_entries
  "limit": 50,                    // opcional en list_entries
  "profile_id": "default"         // opcional
}
```

#### Scenario: `get_entry` sin fecha devuelve la de hoy

- **Given** una entrada guardada con la fecha del día actual
- **When** el agente invoca `journal.get_entry` sin `date`
- **Then** devuelve la entrada de hoy

#### Scenario: `get_entry` de un día sin entrada no falla

- **Given** un día sin entrada en la bitácora
- **When** el agente invoca `journal.get_entry` con esa fecha
- **Then** informa de que no existe
- **And** no devuelve error

#### Scenario: `write_entry` crea la entrada del día

- **Given** un perfil sin entrada para una fecha
- **When** el agente invoca `journal.write_entry` con esa `date`, un `content` y un `mood` válido
- **Then** la entrada queda guardada con ese relato y ese ánimo

#### Scenario: `write_entry` sobrescribe la del mismo día

- **Given** una entrada ya guardada para una fecha
- **When** el agente invoca `journal.write_entry` con la misma fecha y otro relato
- **Then** sigue habiendo una sola entrada para ese día
- **And** su relato es el nuevo

#### Scenario: `update_entry` solo toca lo que le llega

- **Given** una entrada con relato, ánimo y etiquetas
- **When** el agente invoca `journal.update_entry` solo con un ánimo nuevo
- **Then** el ánimo cambia
- **And** el relato y las etiquetas se conservan

#### Scenario: `delete_entry` borra la del día

- **Given** una entrada existente para una fecha
- **When** el agente invoca `journal.delete_entry` con esa fecha y la acción se aprueba
- **Then** la fila desaparece de `journal_entries`

#### Scenario: `delete_entry` de lo que no existe no falla

- **Given** una fecha sin entrada
- **When** el agente invoca `journal.delete_entry` con esa fecha
- **Then** informa de que no existe
- **And** no devuelve error

#### Scenario: `list_entries` devuelve el rango ordenado

- **Given** entradas en varias fechas
- **When** el agente invoca `journal.list_entries` con un `from` y un `to`
- **Then** devuelve solo las del rango
- **And** de más reciente a más antigua

#### Scenario: Una fecha mal formada es un argumento inválido

- **Given** una `date` que no encaja en `YYYY-MM-DD`
- **When** se ejecuta `journal.write_entry` u otra operación que use `date`
- **Then** devuelve `ToolError::InvalidArguments`
- **And** no consulta la base de datos

#### Scenario: Un ánimo fuera del conjunto es un argumento inválido

- **Given** un `mood` que no pertenece a `great`, `good`, `neutral`, `bad` ni `terrible`
- **When** se ejecuta `journal.write_entry` o `journal.update_entry`
- **Then** devuelve `ToolError::InvalidArguments`
- **And** la entrada no se modifica

#### Scenario: Una entrada sin ánimo se guarda igual

- **Given** una llamada a `journal.write_entry` con `content` y sin `mood`
- **When** se ejecuta
- **Then** la entrada se guarda con `mood` a `None`

### Requirement: La herramienta `journal` SHALL exigir aprobación explícita para borrar

La herramienta `journal` SHALL declarar `Permission::NoConfirm` para `get_entry`, `write_entry`,
`update_entry` y `list_entries`, y `Permission::ExplicitApproval` para `delete_entry`. Borrar la
bitácora es destructivo y es el diario del usuario: no se ejecuta sin su confirmación.

#### Scenario: Leer y escribir no piden confirmación

- **Given** la herramienta `journal` en el registry
- **When** se consulta su permiso con `{"operation": "get_entry"}`, `{"operation": "write_entry"}`,
  `{"operation": "update_entry"}` o `{"operation": "list_entries"}`
- **Then** el permiso es `NoConfirm`

#### Scenario: Borrar exige aprobación explícita

- **Given** la herramienta `journal` en el registry
- **When** se consulta su permiso con `{"operation": "delete_entry", "date": "2026-10-18"}`
- **Then** el permiso es `ExplicitApproval`

### Requirement: La definición de `journal` SHALL describirse en español y documentar los obligatorios

La herramienta `journal` SHALL exponer `description()` y las descripciones de sus parámetros en
español, SHALL figurar en `definitions()` con un esquema de parámetros de tipo objeto y SHALL
documentar en la descripción de `operation` qué campos exige cada acción, de modo que el modelo no
invoque una operación sin sus argumentos.

#### Scenario: La herramienta figura en las definiciones

- **Given** el registro de herramientas construido
- **When** se llama a `definitions()`
- **Then** incluye `journal` con las cinco operaciones

#### Scenario: La descripción de `operation` enumera los obligatorios

- **Given** la definición de la herramienta `journal`
- **When** se inspecciona la descripción de su parámetro `operation`
- **Then** menciona que `content` es obligatorio para `write_entry`
- **And** que `date` lo es para `write_entry`, `update_entry` y `delete_entry`

#### Scenario: Las descripciones están en español

- **Given** la definición de la herramienta `journal`
- **When** se inspeccionan `description()` y las descripciones de sus parámetros
- **Then** están redactadas en español
