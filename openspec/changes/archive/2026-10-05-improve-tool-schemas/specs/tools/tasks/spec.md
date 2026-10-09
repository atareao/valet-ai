# Spec Delta

## MODIFIED Requirements

### Requirement: TasksTool SHALL manage tasks through five operations

La tool `tasks` SHALL despachar por `operation` y soportar `list_tasks`, `add_task`, `update_task`, `complete_task` y `delete_task` sobre la tabla `tasks` a través de `TasksRepo`. `list_tasks` y `add_task` SHALL requerir `profile_id` (se expone en `parameters()`). `add_task` SHALL crear la tarea con estado `inbox`, prioridad por defecto `medium` y ámbito por defecto `shared`. La descripción del parámetro `operation` SHALL indicar que `content` es obligatorio para `add_task` e `id` para `update_task`, `complete_task` y `delete_task`.

#### Scenario: add_task crea una tarea en inbox

- **Given** una `TasksTool`
- **When** se ejecuta con `{"operation":"add_task","content":"Preparar informe"}`
- **Then** DEBE crear la tarea con estado `inbox`, prioridad `medium` y ámbito `shared`
- **And** DEBE devolver la tarea creada

#### Scenario: list_tasks sin profile_id falla

- **Given** una `TasksTool`
- **When** se ejecuta con `{"operation":"list_tasks"}`
- **Then** DEBE devolver `InvalidArguments` («Missing profile_id»)

#### Scenario: list_tasks filtra por argumentos opcionales

- **Given** una `TasksTool` con `profile_id` "profile-1"
- **When** se ejecuta con `{"operation":"list_tasks","profile_id":"profile-1","status":"inbox"}`
- **Then** DEBE filtrar por `status`, `priority`, `project` y `scope`
- **And** DEBE devolver `[]` si no hay coincidencias

#### Scenario: complete_task completa una tarea

- **Given** una tarea existente
- **When** se ejecuta con `{"operation":"complete_task","id":"<id>"}`
- **Then** DEBE marcarla completada vía `TasksRepo::complete`
- **And** DEBE devolver la tarea actualizada

#### Scenario: delete_task comprueba existencia antes de borrar

- **Given** un `id` inexistente
- **When** se ejecuta con `{"operation":"delete_task","id":"<id>"}`
- **Then** NO DEBE borrar nada y DEBE propagar el error de «no encontrada»

#### Scenario: La operación documenta los obligatorios

- **Given** la definición de la tool `tasks`
- **When** se inspecciona la descripción del parámetro `operation`
- **Then** menciona `content` para `add_task` e `id` para `update_task`, `complete_task` y `delete_task`
