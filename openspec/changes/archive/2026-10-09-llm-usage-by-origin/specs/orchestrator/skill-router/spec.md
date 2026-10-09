## MODIFIED Requirements

### Requirement: El enrutador SHALL registrar su decisión y su coste sin alterar las estadísticas del chat

Cada decisión SHALL registrarse con `tracing`, incluyendo las skills seleccionadas, sus probabilidades, la fuente (clasificador o fallo abierto), la latencia y el coste devuelto por el servicio. Además, cuando hubo llamada al clasificador (fuente `Router` o `Error`), la selección SHALL exponer su telemetría —modelo, tokens de entrada y salida, coste, latencia y estado— para que el orquestador la persista (ver `orchestrator/agent`). El enrutador SHALL NOT escribir por sí mismo en la tabla de estadísticas ni alterar `last_api_call`; las agregaciones del chat excluyen las filas que no son `kind='chat'`, de modo que las estadísticas del chat no se contaminan.

#### Scenario: La decisión queda registrada
- **Given** un turno enrutado
- **When** se inspecciona el log
- **Then** figuran las skills seleccionadas, sus probabilidades, la fuente, la latencia y el coste
- **And** la selección expone la telemetría de la llamada al clasificador

#### Scenario: Las estadísticas no se contaminan
- **Given** un turno con enrutado y una llamada de chat
- **When** se consultan las estadísticas
- **Then** la petición del clasificador figura como una fila `kind='router'`
- **And** las agregaciones de chat (resumen, por modelo y por día) no la incluyen
- **And** `last_api_call` sigue describiendo el chat
