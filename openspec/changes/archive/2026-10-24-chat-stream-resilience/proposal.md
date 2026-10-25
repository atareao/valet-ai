# Change: Resiliencia del streaming de chat

## Why

El cliente HTTP de OpenRouter se construye con un **timeout total** de 120 s (`src/lib.rs`, `timeout_secs: 120`) que, en `reqwest`, cubre **toda** la petición **incluida la lectura del cuerpo en streaming**. Con modelos lentos y respuestas largas (p. ej. `qwen/qwen3-235b-a22b-2507`, ~4,5 tok/s, `max_tokens` 4096) un turno que fluye con normalidad **supera los 120 s** y el cuerpo se corta a media emisión con `Stream read error: error decoding response body`. Como el texto parcial **no se persiste**, el mensaje desaparece de la interfaz. Y como el timeout está **hardcodeado**, no hay forma de ajustarlo por configuración.

Incidente reproducido en producción (co1) el 2026-10-24: turno con `duration_ms≈119549` y `status=error`; y un cuelgue silencioso previo, en el que el turno murió con `Ok(())` al fallar el `send` tras desconectarse el cliente, sin log ni fila.

## What Changes

- **MODIFIED** el cliente de streaming: sustituir el **timeout total** por un **timeout de inactividad** (tiempo máximo *entre* fragmentos). Una respuesta que sigue produciendo nunca se corta por tiempo; una que se queda muda falla con un error de timeout.
- **Nuevo** `timeout` configurable por variable de entorno (total para peticiones no-streaming; de inactividad para streaming), con los valores actuales como default.
- **Nuevo** en el orquestador: cuando el stream falla a mitad, **preservar el texto parcial** acumulado dentro del evento de error hacia el cliente, en vez de descartarlo.
- **Nuevo** en el frontend: ante un fallo con texto parcial, **mantenerlo visible** con un aviso claro de interrupción, en lugar de borrarlo.

## Capabilities

- **New Capabilities**: (ninguna)
- **Modified Capabilities**:
  - `llm/provider`: el streaming usa timeout de inactividad, nunca uno total.
  - `llm/openrouter`: el timeout del cliente es configurable por entorno.
  - `orchestrator`: preserva el texto parcial cuando el stream falla a mitad.
  - `frontend`: no descarta el texto parcial interrumpido.

## Impact

- **Backend**: `src/llm/openrouter.rs` (cliente y `chat_stream`), `src/llm/ollama.rs` (paridad), `src/lib.rs` (config del proveedor y lectura de entorno), `src/orchestrator/agent.rs` (texto parcial en el evento de error).
- **Frontend**: `src/hooks/useMainChat.ts` / `useSSE.ts` y `ChatView`.
- **Sin migración de base de datos**: el texto parcial no se persiste, viaja en el evento de error del stream.
- **Config**: nuevas variables `OPENROUTER_TIMEOUT_SECS` y `OPENROUTER_STREAM_READ_TIMEOUT_SECS` (y homólogas de Ollama). Defaults: 120 s.
- **Fuera de alcance**: persistir el texto parcial en `messages`, cambiar de modelo, `docker-compose.prod.yml` ni producción.
