# Spec Delta: tools/registry

## REMOVED Requirements

### Requirement: El registry SHALL registrar las herramientas integradas notes y unified_search

## ADDED Requirements

### Requirement: El registry SHALL registrar todas las herramientas integradas sin nombres duplicados

La construcción del registry de producción SHALL registrar todas las herramientas integradas
(incluidas `notes`, `unified_search` y `render_widget`), garantizando nombres únicos.

**Given** la aplicación Valet con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** figuran entre ellas `notes`, `unified_search` y `render_widget`
**And** no hay dos herramientas con el mismo nombre

#### Scenario: El registry de producción incluye todas las herramientas integradas

**Given** la aplicación Valet construida con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** `notes`, `unified_search` y `render_widget` figuran entre ellas
**And** no hay dos herramientas con el mismo nombre

### Requirement: El registry SHALL registrar la herramienta `render_widget`

`build_tool_registry` SHALL registrar una herramienta `render_widget` que el LLM pueda invocar para
solicitar la renderización de un widget interactivo. La herramienta SHALL exponerse en `definitions()`
(los `ToolDef` enviados al modelo) y SHALL declarar `Permission::NoConfirm`, de modo que su invocación
no requiera aprobación.

**Given** la aplicación Valet con su registro de herramientas de producción
**When** se consulta la lista de herramientas registradas
**Then** `render_widget` figura entre ellas

#### Scenario: `render_widget` se ofrece al modelo sin requerir aprobación

**Given** el registry de producción con `render_widget` habilitada
**When** se solicitan las definiciones de herramientas
**Then** la lista contiene una definición `render_widget` con un parámetro `widget_name` de tipo string y un parámetro `data` de tipo objeto
**And** su permiso es `NoConfirm`

### Requirement: La herramienta `render_widget` SHALL gestionarse como el resto desde la tabla `tools`

La sincronización de la tabla `tools` con el registry (`ToolsRepo::sync_from_registry`) SHALL insertar
`render_widget` al arrancar, de modo que `GET /api/tools` la devuelva con su `name`, su `description` y
`enabled: true` por defecto, y la pestaña «Herramientas» la muestre como a cualquier otra. Su estado
habilitado SHALL gobernar si se ofrece al LLM y si su ejecución se rechaza, y alternarlo desde la UI
SHALL surtir efecto en caliente.

#### Scenario: Aparece en la API de herramientas

**Given** una base de datos sin la fila de `render_widget`
**When** la aplicación arranca y sincroniza la tabla `tools` con el registry
**Then** `GET /api/tools` devuelve una tool `render_widget` habilitada

#### Scenario: Deshabilitarla la oculta del LLM

**Given** la tool `render_widget` habilitada
**When** el usuario la deshabilita desde la pestaña «Herramientas»
**Then** las definiciones ofrecidas al LLM omiten `render_widget`
**And** la ejecución de `render_widget` se rechaza

#### Scenario: Habilitarla de nuevo la reexpone

**Given** la tool `render_widget` deshabilitada
**When** el usuario la habilita desde la pestaña «Herramientas»
**Then** las definiciones ofrecidas al LLM vuelven a contener `render_widget`

### Requirement: `render_widget` SHALL validar `widget_name` contra una lista permitida

La ejecución de `render_widget` SHALL aceptar únicamente los nombres de una lista permitida
(`QuickForm`, `Checklist`). Con un nombre desconocido o ausente SHALL devolver
`Err(ToolError::InvalidArguments)` sin efectos. Un `data` ausente o que no sea objeto SHALL
normalizarse a un objeto vacío.

#### Scenario: Nombre permitido

**Given** una invocación de `render_widget` con `widget_name: "Checklist"` y `data` que es un objeto
**When** se ejecuta la herramienta
**Then** devuelve un `ToolResult` con `success: true`

#### Scenario: Nombre no permitido

**Given** una invocación de `render_widget` con `widget_name: "SystemMonitor"`
**When** se ejecuta la herramienta
**Then** devuelve `Err(ToolError::InvalidArguments)`

#### Scenario: `data` ausente se normaliza

**Given** una invocación de `render_widget` con `widget_name: "QuickForm"` y sin `data`
**When** se ejecuta la herramienta
**Then** devuelve un `ToolResult` con `success: true`
