# tools/tasks Specification

## Purpose
Gestión de tareas personales: listar, crear, actualizar, completar y borrar tareas mediante la tool `tasks`.

## Requirements

### Requirement: TasksTool SHALL manage tasks through five operations

La tool `tasks` SHALL despachar por `operation` y soportar `list_tasks`, `add_task`, `update_task`,
`complete_task` y `delete_task` sobre la tabla `tasks` a través de `TasksRepo`. `list_tasks` y
`add_task` SHALL requerir `profile_id` (se expone en `parameters()`). `add_task` SHALL crear la tarea con
estado `inbox`, prioridad por defecto `medium` y ámbito por defecto `shared`.

**Given** una `TasksTool` con acceso a la BD
**When** se ejecuta `tasks` con una `operation`
**Then** DEBE operar sobre `TasksRepo` y devolver datos serializados

#### Scenario: add_task crea una tarea en inbox
**Given** una `TasksTool`
**When** se ejecuta con `{"operation":"add_task","content":"Preparar informe"}`
**Then** DEBE crear la tarea con estado `inbox`, prioridad `medium` y ámbito `shared`
**And** DEBE devolver la tarea creada

#### Scenario: list_tasks sin profile_id falla
**Given** una `TasksTool`
**When** se ejecuta con `{"operation":"list_tasks"}`
**Then** DEBE devolver `InvalidArguments` («Missing profile_id»)

#### Scenario: list_tasks filtra por argumentos opcionales
**Given** una `TasksTool` con `profile_id` "profile-1"
**When** se ejecuta con `{"operation":"list_tasks","profile_id":"profile-1","status":"inbox"}`
**Then** DEBE filtrar por `status`, `priority`, `project` y `scope`
**And** DEBE devolver `[]` si no hay coincidencias

#### Scenario: complete_task completa una tarea
**Given** una tarea existente
**When** se ejecuta con `{"operation":"complete_task","id":"<id>"}`
**Then** DEBE marcarla completada vía `TasksRepo::complete`
**And** DEBE devolver la tarea actualizada

#### Scenario: delete_task comprueba existencia antes de borrar
**Given** un `id` inexistente
**When** se ejecuta con `{"operation":"delete_task","id":"<id>"}`
**Then** NO DEBE borrar nada y DEBE propagar el error de «no encontrada»

### Requirement: TasksTool SHALL require ExplicitApproval only for delete_task

La tool `tasks` SHALL devolver `ExplicitApproval` para `delete_task` y `NoConfirm` para el resto de
operaciones, conforme al requisito de permisos de `tools/registry`.

**Given** una `TasksTool`
**When** la `operation` es `delete_task`
**Then** `permission()` DEBE devolver `ExplicitApproval`
**And** para `list_tasks`, `add_task`, `update_task` y `complete_task` DEBE devolver `NoConfirm`

#### Scenario: delete_task exige aprobación explícita
**Given** una `TasksTool`
**When** la `operation` es `delete_task`
**Then** `permission()` DEBE devolver `ExplicitApproval`

#### Scenario: El resto de operaciones no requiere confirmación
**Given** una `TasksTool`
**When** la `operation` es `list_tasks`, `add_task`, `update_task` o `complete_task`
**Then** `permission()` DEBE devolver `NoConfirm`
