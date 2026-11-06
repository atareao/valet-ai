# orchestrator/agent — Delta de especificación (change tool-usage-stats)

## ADDED Requirements

### Requirement: El orquestador SHALL persistir las herramientas usadas en `llm_requests.tool_calls`

En cada llamada de chat del bucle ReAct, el orquestador SHALL grabar en `llm_requests.tool_calls` un JSON con los nombres de las herramientas solicitadas por el modelo en esa respuesta, con la forma `[{"name":"<tool>"}, …]`. SHALL aplicar la misma precedencia que la ejecución: los `tool_calls` del evento `Done` y, solo si el `Done` no los trae, los acumulados del stream. Cuando la respuesta no solicite ninguna herramienta, la fila SHALL grabarse con `tool_calls = NULL`. Esto SHALL hacer que `StatsRepo::tools_summary()` (que lee `llm_requests.tool_calls` filtrando `kind='chat'`) refleje el uso real de herramientas.

#### Scenario: Una respuesta con tool_calls queda registrada
**Given** una llamada de chat cuyo evento `Done` trae `tool_calls = [{name:"get_weather"},{name:"search_web"}]`
**When** el orquestador persiste la llamada con `record_request`
**Then** la fila de `llm_requests` tiene `kind='chat'` y `tool_calls = '[{"name":"get_weather"},{"name":"search_web"}]'`
**And** `StatsRepo::tools_summary()` cuenta `get_weather: 1` y `search_web: 1`

#### Scenario: Una respuesta sin tool_calls no ensucia el recuento
**Given** una llamada de chat cuya respuesta no solicita ninguna herramienta
**When** el orquestador persiste la llamada
**Then** la fila tiene `tool_calls = NULL`
**And** `StatsRepo::tools_summary()` no la cuenta

#### Scenario: Precedencia Done sobre stream
**Given** un `Done` sin `tool_calls` y `tool_calls_from_stream` con `[{name:"rss_list_unread"}]`
**When** el orquestador persiste la llamada
**Then** la fila graba `tool_calls = '[{"name":"rss_list_unread"}]'`
