# tools/registry Specification

## Purpose
El catálogo de herramientas que el orquestador de Valet ofrece al LLM: qué herramientas existen,
cuáles se exponen en cada petición y en qué condiciones se permite su ejecución.

## Requirements

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

### Requirement: El registry SHALL registrar todas las herramientas integradas sin nombres duplicados

La construcción del registry de producción SHALL registrar todas las herramientas integradas
(incluidas `notes`, `unified_search`, `render_widget` y las cuatro herramientas `strava_*`),
garantizando nombres únicos.

**Given** la aplicación Valet con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** figuran entre ellas `notes`, `unified_search`, `render_widget` y las cuatro `strava_*`
**And** no hay dos herramientas con el mismo nombre

#### Scenario: El registry de producción incluye todas las herramientas integradas

**Given** la aplicación Valet construida con su registro de herramientas de producción
**When** se consulta el catálogo de herramientas registradas
**Then** `notes`, `unified_search`, `render_widget` y las cuatro `strava_*` figuran entre ellas
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

### Requirement: La definición de `render_widget` SHALL documentar el esquema de `data` de cada widget

La definición de la herramienta `render_widget` (`parameters()`) SHALL documentar, para cada widget permitido, la forma esperada de `data`, y SHALL refinar el esquema de `data` con una unión discriminada por `widget_name` (`oneOf` o `anyOf`), de modo que el modelo emita las claves correctas. `data` SHALL permanecer opcional y NO SHALL figurar en `required`. Para `QuickForm` SHALL documentar `{ title, fields: [{ name, label, type, options?, min?, max? }], submit_label? }` con los tipos de campo admitidos `text`, `textarea`, `number`, `select`, `checkbox` y `slider`. Para `Checklist` SHALL documentar `{ title, items: [{ id, label }] }`. Para `LocationWidget` SHALL documentar `{ title, description?, latitude, longitude, address? }` con `latitude` y `longitude` numéricas.

#### Scenario: La definición documenta el esquema de QuickForm

- **Given** la definición de la tool `render_widget`
- **When** se inspecciona el esquema de su parámetro `data`
- **Then** menciona `fields` con `name`, `label` y `type`
- **And** enumera los tipos de campo `text`, `textarea`, `number`, `select`, `checkbox` y `slider`

#### Scenario: La definición documenta el esquema de Checklist

- **Given** la definición de la tool `render_widget`
- **When** se inspecciona el esquema de su parámetro `data`
- **Then** menciona `items` con `id` y `label`

#### Scenario: La definición documenta el esquema de LocationWidget

- **Given** la definición de la tool `render_widget`
- **When** se inspecciona el esquema de su parámetro `data`
- **Then** menciona `LocationWidget` con `latitude` y `longitude`
- **And** el enum de `widget_name` incluye `LocationWidget`

#### Scenario: data sigue siendo opcional

- **Given** la definición de la tool `render_widget`
- **When** se inspecciona su `required`
- **Then** contiene `["widget_name"]`
- **And** NO contiene `data`

#### Scenario: El esquema de data usa una unión discriminada

- **Given** la definición de la tool `render_widget`
- **When** se inspecciona la propiedad `data`
- **Then** su esquema incluye `oneOf` o `anyOf` con una alternativa por widget
- **And** cada alternativa corresponde a uno de los valores del enum de `widget_name`

### Requirement: Las definiciones de herramientas SHALL estar en español y documentar los obligatorios por operación

Todas las tools integradas SHALL exponer `description()` y las descripciones de sus parámetros en español. Las tools que despachan por un parámetro `operation` SHALL documentar en la descripción de `operation` qué campos son obligatorios para cada acción, de modo que el modelo no invoque una operación sin sus argumentos. `render_widget` SHALL mantener `data` como opcional.

#### Scenario: Las tools en inglés se traducen al español

- **Given** las definiciones de `web_search` y `search_places`
- **When** se inspeccionan `description()` y las descripciones de sus parámetros
- **Then** todas están redactadas en español

#### Scenario: La descripción de operation enumera los obligatorios

- **Given** las definiciones de `notes`, `tasks` y `reminders`
- **When** se inspecciona la descripción del parámetro `operation`
- **Then** menciona qué campos exige cada acción (por ejemplo `id` para borrar o actualizar y `content` al crear)

#### Scenario: Las tools de geolocalización y hora se describen en español

- **Given** las definiciones de `geocode`, `reverse_geocode`, `get_current_location`, `get_current_time` y `unified_search`
- **When** se inspeccionan sus descripciones
- **Then** están redactadas en español y describen qué hacen
- **And** la de `unified_search` aclara que la consulta es en lenguaje natural o palabras clave

#### Scenario: La descripción de weather guía a geocode

- **Given** la definición de la tool `weather`
- **When** se inspecciona su descripción
- **Then** indica que, si solo se dispone del nombre de la ciudad, debe resolverse antes con `geocode`
- **And** `required` sigue siendo `["latitude", "longitude"]`

### Requirement: El registry SHALL exponer las definiciones de todas las herramientas registradas

`definitions()` SHALL devolver las definiciones de **todas** las herramientas registradas, sin filtrar por ningún estado de habilitación por herramienta. El filtrado por dominio SHALL ser responsabilidad del enrutador, que pide el subconjunto de las skills seleccionadas y habilitadas.

#### Scenario: Todas las herramientas registradas se ofrecen por el registro completo
- **Given** un registry con las herramientas `weather` y `tasks`
- **When** se solicitan las definiciones
- **Then** la lista contiene `weather` y `tasks`

### Requirement: El registry SHALL devolver las definiciones de un subconjunto pedido

El registry SHALL exponer un método que devuelva las definiciones correspondientes a una lista de nombres, en un **orden determinista por nombre**, de modo que la petición al LLM sea reproducible. Los nombres desconocidos SHALL ignorarse sin error. El subconjunto lo decide el enrutador (`core ∪ (skills seleccionadas ∩ habilitadas)`); el registry SHALL NOT filtrar por ningún estado de habilitación por herramienta.

#### Scenario: El subconjunto devuelve exactamente las definiciones pedidas
- **Given** un registry con `weather` y `tasks`
- **When** se solicitan las definiciones del subconjunto `weather` y `tasks`
- **Then** la lista contiene ambas

#### Scenario: El orden es estable entre llamadas
- **Given** un subconjunto de varios nombres
- **When** se solicitan sus definiciones dos veces
- **Then** ambas llamadas devuelven el mismo orden
- **And** el orden es el alfabético por nombre

#### Scenario: Un nombre desconocido no es un error
- **Given** un subconjunto que incluye un nombre no registrado
- **When** se solicitan sus definiciones
- **Then** el nombre desconocido se ignora
- **And** el resto se devuelve con normalidad
