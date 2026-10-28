# frontend/tasks Specification

## Purpose
Acceso a las tareas desde el header de Valet: un enlace externo a OpenTodo. Valet no tiene vista de tareas propia, ni modelo local, ni cliente de `/api/tasks`; las tareas se gestionan donde viven.

## Requirements

### Requirement: AppLayout SHALL ofrecer un acceso directo a OpenTodo en la plaza de la vista de tareas

El header de `AppLayout` SHALL mantener un botón con el icono de tareas en la misma posición que
ocupaba la vista de tareas, y ese botón SHALL ser un **enlace** a `https://todo.territoriolinux.es`
que se abra en una pestaña nueva con `target="_blank"` y `rel="noopener noreferrer"`. La URL SHALL
ser una constante del frontend, sin petición ni estado asociados: el enlace no SHALL depender de
`GET /api/opentodo/status`.

Valet NO SHALL renderizar ninguna vista de tareas propia: no SHALL haber `TaskView`, ni modal, ni
`lazy()`, ni estado `tasksVisible` asociados. El frontend NO SHALL conservar el tipo `Task`, los
métodos `listTasks`/`createTask`/`updateTask`/`deleteTask` ni el evento `tasks-changed`.

#### Scenario: El botón abre OpenTodo en otra pestaña

- **Given** el usuario autenticado en Valet
- **When** pulsa el botón de tareas del header
- **Then** el enlace apunta a `https://todo.territoriolinux.es`
- **And** lleva `target="_blank"` y `rel="noopener noreferrer"`
- **And** no se abre ningún modal

#### Scenario: Un turno del agente ya no refresca una vista inexistente

- **Given** un turno en el que el agente completa una operación `tasks`
- **When** llega el resultado de la herramienta
- **Then** no se dispara ningún evento `tasks-changed`
