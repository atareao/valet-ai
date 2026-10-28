## REMOVED Requirements

### Requirement: TasksTool SHALL manage tasks through five operations

**Reason**: la herramienta pasa a operar sobre la API de **OpenTodo** en lugar de la tabla local
`tasks`, porque el usuario gestiona sus tareas allí y la lista local no tiene proyectos ni vistas.
El contrato de cinco operaciones sobre `TasksRepo` y la exigencia de `profile_id` dejan de describir
el comportamiento.

**Migration**: el comportamiento local se conserva como **fallback** mientras `opentodo_token` no
esté configurado, y está descrito en el requisito añadido «`tasks` SHALL escribir en OpenTodo…». La
retirada definitiva del backend local es el change `remove-local-tasks`.

### Requirement: TasksTool SHALL require ExplicitApproval only for delete_task

**Reason**: el borrado de OpenTodo admite además la **purga** definitiva, que también exige
aprobación explícita; y las escrituras dejan de ser silenciosas. La tabla de permisos vigente es la
del requisito añadido «`tasks` SHALL declarar sus permisos por operación».

**Migration**: `delete_task` mantiene `ExplicitApproval` (con y sin `purge`); el resto de
operaciones pasa a `NoConfirm` solo en lecturas y a `Notify` en escrituras.

## ADDED Requirements

### Requirement: `tasks` SHALL operar sobre OpenTodo y conservar el backend local solo como fallback

La herramienta `tasks` SHALL despachar por un parámetro `operation` con el enum
`["list_tasks","search_tasks","add_task","update_task","complete_task","delete_task","set_label"]` y
`required: ["operation"]`.

Cuando `opentodo_token` esté configurado, **todas** las operaciones SHALL ejecutarse contra la API de
OpenTodo a través del servicio `Opentodo`. Cuando no lo esté, la herramienta SHALL conservar el
comportamiento local vigente sobre la tabla `tasks` (con `profile_id` inyectado por el orquestador),
de modo que desplegar el change sin configurar el token NO SHALL cambiar el comportamiento.

La herramienta SHALL NOT requerir `profile_id` para operar contra OpenTodo. Un fallo de OpenTodo
SHALL devolver un `ToolError` con un mensaje legible y NO SHALL propagar un pánico ni un `unwrap`.

#### Scenario: Con token, las operaciones van a OpenTodo
- **Given** `opentodo_token` configurado
- **When** se ejecuta `tasks` con `{"operation":"list_tasks"}`
- **Then** la herramienta consulta la API de OpenTodo
- **And** no lee la tabla local `tasks`

#### Scenario: Sin token, el comportamiento local se conserva
- **Given** `opentodo_token` sin configurar
- **When** se ejecuta `tasks` con `{"operation":"add_task","content":"Preparar informe"}`
- **Then** la tarea se crea en la tabla local con estado `inbox`, prioridad `medium` y ámbito `shared`
- **And** se sigue exigiendo `profile_id` como hoy

#### Scenario: Un fallo del servicio externo no rompe el turno
- **Given** `opentodo_token` configurado y el servidor devolviendo `503`
- **When** se ejecuta `tasks` con `{"operation":"list_tasks"}`
- **Then** se devuelve un `ToolError` cuyo mensaje menciona que OpenTodo no responde
- **And** el mensaje no contiene el token

#### Scenario: La operación documenta los obligatorios
- **Given** la definición de la herramienta `tasks`
- **When** se inspecciona la descripción del parámetro `operation`
- **Then** menciona `title` para `add_task`, `id` para `update_task`, `complete_task`, `delete_task`
  y `set_label`, y `query` para `search_tasks`

### Requirement: `tasks` SHALL calcular las vistas en Rust y acotar la salida con un resumen agregado

`list_tasks` SHALL aceptar `view` con el enum
`["all","inbox","today","upcoming","anytime","someday","doing","waiting"]` (por defecto `all`),
`project` y `limit`. `search_tasks` SHALL aceptar `query` y `limit`. El `limit` SHALL ser `20` por
defecto y SHALL estar acotado a `1..=50`; un valor fuera de rango SHALL devolver
`ToolError::InvalidArguments` **sin** salir a la red.

Las vistas SHALL calcularse en Rust con los predicados del servicio `Opentodo`, porque
`GET /api/v1/tasks` no filtra. La herramienta **NO SHALL** devolver nunca la lista cruda: SHALL
devolver como máximo `limit` tareas, cada una con **solo** `id`, `title`, `project`, `due`,
`priority`, `start` y `labels`, más un objeto `summary` con los totales por vista (`by_view`) y por
proyecto (`by_project`) y un `count` con el total tras filtro.

#### Scenario: La salida va acotada
- **Given** una cuenta con 300 tareas
- **When** se ejecuta `tasks` con `{"operation":"list_tasks","view":"all"}`
- **Then** se devuelven como mucho 20 tareas
- **And** `count` informa del total real filtrado
- **And** la respuesta incluye `summary.by_view` y `summary.by_project`

#### Scenario: Un limit fuera de rango es un argumento inválido
- **Given** una llamada con `limit` igual a `0` o a `500`
- **When** se ejecuta `tasks`
- **Then** devuelve `ToolError::InvalidArguments`
- **And** no se envía ninguna petición al servidor

#### Scenario: La vista se calcula, no se delega
- **Given** una tarea abierta con `due.date` de ayer y otra con `due.date` del mes que viene
- **When** se ejecuta `tasks` con `{"operation":"list_tasks","view":"today"}`
- **Then** solo la primera aparece
- **And** no se envía ningún filtro de fecha al servidor

#### Scenario: Filtrar por proyecto
- **Given** tareas de dos proyectos
- **When** se ejecuta `tasks` con `{"operation":"list_tasks","project":"Casa"}`
- **Then** solo se devuelven las del proyecto `Casa`

### Requirement: `tasks` SHALL escribir en OpenTodo con prioridad explícita 1:1 y fechas delegadas en quick-add

`add_task` SHALL requerir `title` y aceptar `description`, `project`, `priority` y `due`.
`update_task` SHALL requerir `id` y aceptar los mismos campos más `start`. `complete_task` SHALL
requerir `id`. `delete_task` SHALL requerir `id` y aceptar `purge` (por defecto `false`), que usa el
endpoint de purga definitiva.

La prioridad SHALL mapearse 1:1 y SHALL escribirse **siempre explícita**: `critical → 1`,
`high → 2`, `medium → 3`, `low → 4`. Cuando se omita, SHALL enviarse `4`, que es el valor por defecto
del servidor, para que la lectura posterior sea inequívoca. La herramienta SHALL aceptar los cuatro
valores y NUNCA SHALL inventar un quinto estado «sin prioridad».

`due` SHALL aceptar `YYYY-MM-DD`, `YYYY-MM-DDTHH:MM` o una expresión natural; en este último caso la
conversión SHALL delegarse en `POST /api/v1/quick-add/parse` del servidor y NO SHALL hacerla el modelo.
La prioridad NO SHALL tomarse nunca de ese parseo, porque quick-add no la reconoce.

Los nombres de proyecto SHALL resolverse a `project_id` en el mismo turno; si el proyecto no existe,
la herramienta SHALL responder con la lista real de proyectos en lugar de inventar un id.

#### Scenario: La prioridad se escribe explícita y 1:1
- **Given** una llamada `add_task` con `priority: "critical"`
- **When** se envía a OpenTodo
- **Then** el cuerpo lleva `priority: 1`

#### Scenario: Sin prioridad se envía el valor por defecto del servidor
- **Given** una llamada `add_task` sin `priority`
- **When** se envía a OpenTodo
- **Then** el cuerpo lleva `priority: 4`
- **And** la lectura posterior la devuelve como `low`

#### Scenario: Las fechas naturales se delegan en quick-add
- **Given** una llamada `add_task` con `due: "el viernes a las 10"`
- **When** se ejecuta
- **Then** se llama a `POST /api/v1/quick-add/parse`
- **And** el `due` enviado es el `{date, time}` que devuelve el servidor

#### Scenario: Una fecha ISO no pasa por quick-add
- **Given** una llamada `add_task` con `due: "2026-10-30"`
- **When** se ejecuta
- **Then** no se llama a `quick-add/parse`
- **And** el `due` enviado es `{date: "2026-10-30"}`

#### Scenario: Un proyecto inexistente no se inventa
- **Given** una llamada `add_task` con `project: "Inexistente"` en una cuenta sin proyectos
- **When** se ejecuta
- **Then** no se crea la tarea con un id inventado
- **And** el resultado incluye la lista real de proyectos

#### Scenario: El borrado definitivo usa la purga
- **Given** una llamada `delete_task` con `{"id":"…","purge":true}`
- **When** se ejecuta
- **Then** se llama a `DELETE /api/v1/tasks/{id}/purge`

### Requirement: `tasks` SHALL exponer `set_label` con grupo y valor y no aceptar ids de etiqueta del modelo

`set_label` SHALL requerir `id` y `group` (`state`|`scope`) y SHALL aceptar `value` con los valores
`doing`, `waiting`, `shared`, `personal` o `null`. La herramienta NO SHALL aceptar un array de ids de
etiqueta ni un nombre libre de etiqueta: el modelo indica **grupo y valor** y la fusión la hace el
servicio `Opentodo` según el requisito `set_label` de la spec `opentodo-service`.

Un `group` desconocido o un `value` que no pertenezca a ese grupo SHALL devolver
`ToolError::InvalidArguments` sin salir a la red.

#### Scenario: Poner un estado no borra el ámbito
- **Given** una tarea con la etiqueta `personal`
- **When** se ejecuta `{"operation":"set_label","id":"…","group":"state","value":"doing"}`
- **Then** la tarea queda con `doing` y `personal`

#### Scenario: Un valor fuera del grupo es inválido
- **Given** una llamada `{"operation":"set_label","id":"…","group":"state","value":"personal"}`
- **When** se ejecuta
- **Then** devuelve `ToolError::InvalidArguments`
- **And** no se envía ninguna petición al servidor

#### Scenario: El modelo no puede mandar ids de etiqueta
- **Given** la definición de parámetros de `tasks`
- **When** se inspecciona
- **Then** no existe ningún parámetro que acepte ids de etiqueta
- **And** `set_label` solo acepta `group` y `value`

### Requirement: `tasks` SHALL declarar sus permisos por operación

La herramienta `tasks` SHALL devolver `Permission::NoConfirm` para `list_tasks` y `search_tasks`,
`Permission::Notify` para `add_task`, `update_task`, `complete_task` y `set_label`, y
`Permission::ExplicitApproval` para `delete_task`, tanto con `purge: false` como con `purge: true`
(el token de OpenTodo es `full` y la purga es irreversible).

#### Scenario: Las lecturas no piden confirmación
- **Given** una `TasksTool`
- **When** la `operation` es `list_tasks` o `search_tasks`
- **Then** `permission()` devuelve `NoConfirm`

#### Scenario: Las escrituras avisan
- **Given** una `TasksTool`
- **When** la `operation` es `add_task`, `update_task`, `complete_task` o `set_label`
- **Then** `permission()` devuelve `Notify`

#### Scenario: El borrado exige aprobación explícita también con purga
- **Given** una `TasksTool`
- **When** la `operation` es `delete_task`
- **Then** `permission()` devuelve `ExplicitApproval`
- **And** también la devuelve cuando `purge` es `true`
