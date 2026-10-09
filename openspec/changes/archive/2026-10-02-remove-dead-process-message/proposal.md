# Proposal: Retirar la ruta muerta `process_message`

## Why

`Orchestrator::process_message` es la entrada **sin streaming** del bucle ReAct.
Producción no la usa: el único camino real es `process_message_stream`, que es el
que sirve `POST /api/chat/stream`. A `process_message` solo la invocan los tests.

Mantener dos copias del mismo bucle (~340 líneas) ya ha causado una deriva real:

| | `process_message` | `process_message_stream` |
|---|---|---|
| prompt de sistema | sí | sí |
| bloque episódico | sí | sí |
| fecha/hora/ubicación | **no** | sí |

Y bloquea el reagrupado del prompt en un único mensaje de sistema, porque obliga a
arrastrar el lenguaje de «las dos rutas» en specs y tests. Quitando la ruta muerta,
ese cambio queda sobre un solo camino.

## What Changes

- **D1 — Eliminar `Orchestrator::process_message`** y el código que quede sin uso.
- **D2 — Conservar la cobertura única y borrar la duplicada.** De los 7 tests que
  la invocan, 5 tienen gemelo en streaming y se borran; 2 se migran.
- **D3 — Preservar el registro de stats en error sin tragárselo.** Hallazgo: hoy
  **solo** `process_message` escribe la fila de `llm_requests` con status `"error"`;
  el stream propaga el fallo sin registrarlo (y la ruta HTTP solo lo loguea). Para no
  perder una conducta documentada al borrar la ruta muerta, `process_message_stream`
  SHALL registrar la fila de error (y `save_last_call` con status `"error"`) cuando
  falle `chat_stream()` o el propio stream, **y acto seguido SHALL propagar el error
  al llamante**: registrar no SHALL tragar, sustituir ni enmascarar el fallo. El
  error llega al cliente igual que hoy; lo único que se añade es la fila de
  observabilidad. Es el único cambio de comportamiento de este cambio.
- **D4 — Reescribir tres requisitos de `orchestrator/agent`** que citan la ruta
  muerta, para que el resto de la spec siga diciendo la verdad.
- **D5 — Sin tocar el contrato HTTP** ni la ruta de streaming.

### Fuera de alcance

- El reagrupado del prompt en un único mensaje de sistema: es el change siguiente,
  que se apoyará en la ruta única que deja este.

## Impact

- **Código**: `src/orchestrator/agent.rs` — se van ~340 líneas de método y 5 tests;
  2 tests se migran; el manejo de error del stream gana el registro de stats.
- **Specs**: `orchestrator/agent` — 3 requisitos eliminados y readicionados sin los
  escenarios de la ruta muerta.
- **Sin migración** y sin cambios de esquema.
