## REMOVED Requirements

### Requirement: `tasks` SHALL operar sobre OpenTodo y conservar el backend local solo como fallback

**Reason**: la fase 1 conservó el backend local como red de seguridad para poder desplegar sin token.
Ese despliegue ya ocurrió, el token está configurado en producción y la E2E está verificada; el
fallback solo mantiene viva una segunda fuente de verdad que nadie actualiza.

**Migration**: las 8 tareas locales se migran a OpenTodo antes del release con un script de una sola
ejecución (no versionado; la constancia queda en `plans/PLAN-011.md`) y la tabla `tasks` se elimina.
Sin token, la herramienta devuelve un error legible en vez de caer al almacén local.

## ADDED Requirements

### Requirement: `tasks` SHALL operar exclusivamente sobre OpenTodo

La herramienta `tasks` SHALL despachar por un parámetro `operation` con el enum
`["list_tasks","search_tasks","add_task","update_task","complete_task","delete_task","set_label"]` y
`required: ["operation"]`.

**Todas** las operaciones SHALL ejecutarse contra la API de OpenTodo a través del servicio
`Opentodo`. Sin `opentodo_token` configurado, la herramienta SHALL devolver un `ToolError` legible que
indique que hay que configurarlo en Ajustes → Integraciones → OpenTodo, y NO SHALL leer ni escribir
ninguna tabla de Valet.

La herramienta SHALL NOT requerir `profile_id` ni depender de la tabla `tasks`. Un fallo de OpenTodo
SHALL devolver un `ToolError` con un mensaje legible y NO SHALL propagar un pánico ni un `unwrap`.

#### Scenario: Sin token la herramienta lo dice y no toca la base de datos

- **Given** `opentodo_token` sin configurar
- **When** se ejecuta `tasks` con `{"operation":"add_task","title":"Preparar informe"}`
- **Then** se devuelve un `ToolError` cuyo mensaje menciona OpenTodo y dónde configurar el token
- **And** no se escribe ninguna fila en ninguna tabla

#### Scenario: Con token, las operaciones van a OpenTodo

- **Given** `opentodo_token` configurado
- **When** se ejecuta `tasks` con `{"operation":"list_tasks"}`
- **Then** la herramienta consulta la API de OpenTodo

#### Scenario: Un fallo del servicio externo no rompe el turno

- **Given** `opentodo_token` configurado y el servidor devolviendo `503`
- **When** se ejecuta `tasks` con `{"operation":"list_tasks"}`
- **Then** se devuelve un `ToolError` cuyo mensaje menciona que OpenTodo no responde
- **And** el mensaje no contiene el token
