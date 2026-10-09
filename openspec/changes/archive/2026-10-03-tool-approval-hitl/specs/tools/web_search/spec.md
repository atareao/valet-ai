# Spec Delta: tools/web_search

## Purpose

Búsqueda web en tiempo real usando Brave Search API, permitiendo al LLM consultar información
actualizada de internet. La API key se configura desde la interfaz de settings (tabla `settings` en DB)
con prioridad sobre variable de entorno.

## MODIFIED Requirements

### Requirement: Tool interface

`WebSearchTool` SHALL implementar el trait `Tool` con nombre `web_search`, permiso `NoConfirm` para
cualquier argumento y parámetro requerido `query`.

**Given** `WebSearchTool` implementa el trait `Tool`
**When** se consulta su metadata
**Then** `name()` DEBE ser `"web_search"`
**And** `description()` DEBE describir la búsqueda web
**And** `permission(&args)` DEBE ser `Permission::NoConfirm` independientemente de los args
**And** `parameters()` DEBE requerir `query` como string

#### Scenario: Parámetros correctos
**Given** un `WebSearchTool`
**When** se llama a `parameters()`
**Then** DEBE tener `type: "object"` con propiedad `query: { type: "string" }` requerida
