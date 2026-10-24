# Design: Resiliencia del streaming de chat

## Context

El proveedor OpenRouter (`src/llm/openrouter.rs`) construye **un único** `reqwest::Client` con `.timeout(Duration::from_secs(config.timeout_secs))` y `config.timeout_secs` fijo a `120` en `src/lib.rs`. En `reqwest`, `ClientBuilder::timeout` es un **timeout total** de la petición que **incluye la lectura del cuerpo**; por tanto corta también las respuestas en streaming. `chat_stream()` lanza una tarea que lee `response.chunk()` en bucle y reenvía por un `mpsc` de 64; si `chunk()` devuelve `Err`, envía `LLMError::HttpError("Stream read error: …")` y termina, pero **el contenido acumulado hasta ese punto se descarta**. En el orquestador (`src/orchestrator/agent.rs`) el bucle del turno acumula `content_buffer` con los `StreamEvent::Chunk`; al recibir el error sale del bucle y no escribe mensaje de asistente.

Evidencia en producción (co1, 2026-10-24): turno con `duration_ms=119549`, `status=error`, `error_message="HTTP error: Stream read error: error decoding response body"`, tras ~118 s de deltas continuos; el texto que el usuario vio escribirse desapareció al no persistirse.

## Goals / Non-Goals

**Goals**

- Que una respuesta larga que fluye no se corte por tiempo.
- Que un stream mudo falle con un error claro y acotado en el tiempo.
- Que el texto parcial ya emitido no desaparezca de la interfaz.
- Que los timeouts sean ajustables sin recompilar.

**Non-Goals**

- Persistir el texto parcial en la base de datos.
- Cambiar de modelo o de `max_tokens`.
- Cola de turnos o reconexión automática del turno.
- `docker-compose.prod.yml` ni producción.

## Decisions

### D1 — Timeout de inactividad en el cliente de streaming, no total

El cliente SHALL construirse con `connect_timeout` y `read_timeout` (timeout **entre lecturas** del cuerpo) y **sin** `timeout` total, de modo que una respuesta que sigue produciendo no se aborte por tiempo y una que se queda muda durante más que `read_timeout` falle. Para las llamadas **no**-streaming (`chat()`), donde un tiempo total sí es útil, SHALL mantenerse un timeout total (por petición con `RequestBuilder::timeout`, o un segundo cliente). La API exacta de `reqwest 0.13` (`read_timeout`) SHALL verificarse contra su documentación antes de implementar.

### D2 — Configuración por entorno

`OpenRouterConfig` SHALL leer de entorno el timeout total (no-streaming) y el de inactividad (streaming), con **120 s** como default de ambos (el valor actual). Variables propuestas: `OPENROUTER_TIMEOUT_SECS` y `OPENROUTER_STREAM_READ_TIMEOUT_SECS`. Paridad para Ollama: `OLLAMA_TIMEOUT_SECS` y `OLLAMA_STREAM_READ_TIMEOUT_SECS`.

### D3 — El texto parcial viaja en el evento de error y no se persiste

Cuando el stream falla a mitad y hay contenido acumulado, el orquestador SHALL emitir al cliente un evento de error que **incluya el texto parcial** acumulado. El frontend SHALL mostrar ese texto con un aviso de interrupción, en vez de descartarlo. El parcial **no** se persiste en `messages`: evita una migración y no contamina el historial. Una recarga lo pierde. Persistirlo queda fuera de alcance y podría ser un change posterior.

### D4 — Un stream mudo debe fallar rápido y quedar registrado

El fallo por timeout del stream SHALL registrarse en `llm_requests` con `status='timeout'` y un mensaje distinguible (hoy cae en `error` con `Stream read error`), para poder diagnosticarlo sin `RUST_LOG=debug`.

## Risks / Trade-offs

- `read_timeout` es un timeout de **inactividad**, no total: una respuesta que produzca un byte cada minuto y tarde horas no se cortará. Es el comportamiento deseado para un chat, pero implica que un proveedor que gotea no falla nunca; el límite real pasa a ser `max_tokens`.
- El parcial no persistido se pierde al recargar: se acepta explícitamente.
