# Change: tool-usage-stats

## Why

El panel **Stats → Sistema → Tool Usage** muestra siempre «No data yet». Diagnóstico contra la base de datos real de producción (2026-11-06, v0.31.0):

- `llm_requests` tiene **1166 filas** `kind='chat'` y **0** con `tool_calls` poblado.
- `StatsRepo::tools_summary()` lee exactamente `llm_requests.tool_calls` filtrando `kind='chat'`, y es el único origen del gráfico.
- La ruta de chat (`process_message_stream`) llama a `StatsRepo::record_request(..., tool_calls: None, ...)` aunque el evento `Done` ya trae los `tool_calls` en scope. La columna nunca se escribe.
- El lector cumple su spec; **el escritor no la alimenta**. Es un desajuste de contrato entre `StatsRepo SHALL provide tool call frequency` (db/repos) y `Orchestrator records stats after each streamed LLM call` (orchestrator/agent).

## What Changes

- El orquestador SHALL persistir en `llm_requests.tool_calls` los nombres de las herramientas solicitadas por el modelo en cada llamada de chat, con la forma JSON `[{"name":"<tool>"}, …]`, usando la misma precedencia que ya rige la ejecución: los `tool_calls` del evento `Done` y, solo si el `Done` no los trae, los acumulados del stream.
- Cuando la respuesta no solicite herramientas, la fila SHALL grabarse con `tool_calls = NULL`.
- **Sin cambios** en `StatsRepo::tools_summary()` (el lector ya es correcto) ni en el frontend (la tarjeta se puebla sola cuando llegan datos).
- **Sin backfill del histórico** (recomendado): los turnos previos quedan sin datos de herramientas y el gráfico se puebla a partir del despliegue. DECISIÓN ABIERTA para el usuario.

## Impact

- Specs: `orchestrator/agent` (requirement nuevo). No se modifica `db/repos`.
- Código: `src/orchestrator/agent.rs` (ruta de streaming del bucle ReAct).
- Tests: unitarios del orquestador.
- Base de datos: sin migración (la columna `tool_calls` ya existe). Sin cambios en la API ni en el frontend.
