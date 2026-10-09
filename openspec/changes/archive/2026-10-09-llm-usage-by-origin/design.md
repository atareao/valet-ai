# Design

## D1. Una columna `kind` para el origen de cada llamada

`llm_requests.provider` está siempre a NULL, así que no sirve de discriminador sin forzarlo. Se añade una columna dedicada `kind TEXT NOT NULL DEFAULT 'chat' CHECK (kind IN ('chat','router','archivist','consolidator','collapse'))`. La migración es aditiva: las filas existentes heredan `'chat'`.

## D2. Un único `record_request(pool, kind, …)`

Se añade `kind` a la firma existente en lugar de crear una segunda función casi idéntica. El tipo es un enum cerrado `CallKind { Chat, Router, Archivist, Consolidator, Collapse }` en `models/stats.rs` con `as_str()`. Cada punto de llamada pasa el suyo: chat (agente), archivist (`call_llm`), consolidator (`call_semantic_chat`), collapse (`CollapseWorker`) y router (agente, tras enrutar). Los dos puntos muertos de `agent.rs::analyze` se actualizan con `CallKind::Chat` (nadie llama a `analyze`, pero debe compilar).

## D3. El panel de chat cuenta solo `kind='chat'`

Toda agregación de chat añade `WHERE kind='chat'` (el `by_day` añade `AND kind='chat'` a su `WHERE`). Resultado: el chat deja de incluir el pipeline de memoria, que hasta ahora aparecía como una fila `mistral…`.

## D4. El router expone, el agente persiste

`skill_router` sigue puro (sin `SqlitePool`): lo comparten el agente y el arnés `valet-route-eval`, que nunca escribe. El router devuelve en `Selection` una telemetría opcional (`RouterUsage { model, input_tokens, output_tokens, cost, duration_ms, status }`), poblada solo cuando hubo llamada al clasificador (fuente `Router` o `Error`). El **agente** la inserta con `record_request(…, CallKind::Router, …)`. Las fuentes `Disabled` y `NoRoutableSkills` no generan fila.

## D5. Los fallos se graban

Un fallo (timeout o error HTTP) es señal de calibración y, en su caso, gasto. Se graba con `status='error'`, `cost=0`, `duration_ms` real y sin tokens, para todos los orígenes.

## D6. UI: «Procesos de fondo» en «Modelos»

Una tarjeta con una fila por origen no-chat (router, archivist, consolidator, collapse): llamadas, tokens, coste, latencia media y errores. Las tarjetas de chat quedan intactas. Se descarta una pestaña nueva para no fragmentar.

## D7. `last_api_call` intacto

El indicador «Última llamada» sigue describiendo el chat. Ningún origen de fondo lo toca.

## Alternativas descartadas

- **Reutilizar `provider`**: semántica engañosa y frágil.
- **Escribir sin marca en `llm_requests`**: contamina percentiles y totales del chat.
- **Tabla nueva por origen**: duplica esquema y consultas; la columna `kind` cubre el caso.
- **Registrar desde el propio router**: acopla dominio a persistencia y rompe la pureza del arnés.
