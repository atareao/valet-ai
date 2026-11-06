# Tasks: tool-usage-stats

## 1. RED (tests que fallan)
- [x] 1.1 Test: tras un turno de streaming cuyo `Done` trae `tool_calls`, existe una fila `llm_requests` con `kind='chat'` y `tool_calls` con los nombres. `cargo test` → RED.
- [x] 1.2 Test: precedencia `Done` > stream para el campo persistido.
- [x] 1.3 Test: una respuesta sin `tool_calls` graba `tool_calls = NULL`.
- [x] 1.4 Test: `StatsRepo::tools_summary()` devuelve el recuento esperado sobre las filas escritas por el orquestador.

## 2. GREEN (implementación mínima)
- [x] 2.1 Serializar los nombres de las herramientas y pasarlos a `StatsRepo::record_request` en la ruta de streaming (`process_message_stream`), con la precedencia `Done` > stream.

## 3. REFACTOR
- [x] 3.1 `cargo fmt --check`, `cargo clippy -- -D warnings`, `cargo test` en verde.

## 4. Cierre
- [ ] 4.1 Release: bump de versión, PR a `main`, tag y despliegue en `co1`.
- [ ] 4.2 Verificación: invocar una herramienta desde el chat y comprobar que **Stats → Sistema → Tool Usage** muestra datos.
- [ ] 4.3 `openspec archive tool-usage-stats`.
