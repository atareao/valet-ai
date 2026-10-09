# tools/web_search Specification

## Purpose
Búsqueda web en tiempo real usando Brave Search API, permitiendo al LLM consultar información actualizada de internet. La API key se configura desde la interfaz de settings (tabla `settings` en DB) con prioridad sobre variable de entorno.

## Requirements

### Requirement: Búsqueda web con Brave Search API

La tool SHALL leer `brave_search_api_key` de settings y llamar a la API de Brave Search con el header `X-Subscription-Token`.

**Given** un `WebSearchTool`
**When** se ejecuta `web_search` con una consulta
**Then** la tool DEBE leer `brave_search_api_key` de la tabla `settings` (SettingsRepo)
**And** DEBE llamar a `GET https://api.search.brave.com/res/v1/web/search?q={query}`
**And** DEBE incluir header `X-Subscription-Token` con la API key
**And** DEBE devolver los resultados formateados como texto en `ToolResult.data`

#### Scenario: Búsqueda exitosa devuelve resultados
**Given** un `WebSearchTool` con API key en settings DB
**When** se ejecuta con `{"query": "últimas noticias inteligencia artificial"}`
**Then** DEBE retornar `ToolResult` con `success: true`
**And** `data` DEBE contener un array de resultados con `title`, `url`, `description`

#### Scenario: Consulta vacía
**Given** un `WebSearchTool`
**When** se ejecuta con `{"query": ""}`
**Then** DEBE retornar `Err(ToolError::InvalidArguments)` con mensaje "Missing query"

#### Scenario: Error de API
**Given** un `WebSearchTool` con API key inválida en settings DB
**When** se ejecuta `web_search`
**Then** DEBE retornar `Err(ToolError::ExecutionError)` con el código de error HTTP

#### Scenario: Sin API key configurada
**Given** un `WebSearchTool` sin API key en settings DB ni en Config/ENV
**When** se ejecuta `web_search`
**Then** DEBE retornar `Err(ToolError::ExecutionError)` indicando que falta la API key

### Requirement: Formato de resultados

La tool SHALL extraer hasta 5 resultados de `web.results`, cada uno con `title`, `url` y `description`.

**Given** una respuesta exitosa de Brave Search
**When** se procesan los resultados
**Then** DEBE extraer hasta 5 resultados del campo `web.results`
**And** Cada resultado DEBE contener: `title`, `url`, `description`

#### Scenario: Resultados vacíos
**Given** una respuesta de Brave Search sin resultados
**When** se procesa
**Then** DEBE retornar `ToolResult` con `data` conteniendo array vacío

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
