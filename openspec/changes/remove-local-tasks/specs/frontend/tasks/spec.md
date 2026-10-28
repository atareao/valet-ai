## REMOVED Requirements

### Requirement: Task type and API client

**Reason**: sin TODO local no hay `/api/tasks` que consumir ni tipo `Task` que modelar. El frontend
no gestiona tareas: el usuario trabaja en OpenTodo.

**Migration**: los métodos `listTasks`, `createTask`, `updateTask` y `deleteTask` y la interfaz
`Task` se borran; el acceso pasa a ser un enlace externo (requisito añadido).

### Requirement: Task hooks

**Reason**: `useTasks` y sus mutaciones solo existían para alimentar `TaskView`. Sin vista ni API,
sobran.

**Migration**: se borra `frontend/src/hooks/useTasks.ts` y su test.

### Requirement: TaskView component with Kanban and List views

**Reason**: la vista Kanban/Lista pinta estados GTD que ya no son la verdad. El usuario gestiona sus
tareas en `https://todo.territoriolinux.es` y decidió que Valet no reescriba esa interfaz contra la
API de OpenTodo.

**Migration**: se borran `TaskView.tsx`, `TaskModal.tsx`, `task.constants.ts` y sus tests; en su
lugar queda un enlace externo.

### Requirement: TaskView integrated in AppLayout

**Reason**: sin `TaskView` no hay modal que integrar, ni estado `tasksVisible`, ni evento
`tasks-changed` que refrescar.

**Migration**: el botón del header pasa a ser un enlace a OpenTodo con `target="_blank"` y
`rel="noopener noreferrer"`; el evento `tasks-changed` desaparece de `useMainChat`.

## ADDED Requirements

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
