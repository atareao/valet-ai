# Proposal

## Why

Toda llamada a un LLM en Valet acaba en `llm_requests` vía `StatsRepo::record_request`, pero la fila no dice **qué tipo de llamada es**. Cinco orígenes comparten la tabla y hoy son indistinguibles salvo por el modelo:

- el turno de **chat** (`OPENROUTER_MODEL`);
- el **router** de skills por turno (`typesafe/jev-1.13`), que desde la decisión D10 de `skill-router` ni siquiera se registra;
- el **archivist** (ficha de memoria episódica, `MEMORY_MODEL`);
- el **consolidator** (memoria persistente, `SEMANTIC_MODEL`);
- el **collapse** (colapso de mensajes, `COLLAPSE_MODEL`).

Consecuencia: la pestaña **Estadísticas → Modelos** mezcla los procesos de fondo con las cifras del chat (esa fila `mistral…` no es chat) y el router (`typesafe/jev-1.13`) no aparece nunca. Ni la latencia ni el coste del chat son honestos, y el gasto real del clasificador (~$0,00005/turno) es invisible.

## What Changes

- Nueva columna `kind` en `llm_requests`: `'chat' | 'router' | 'archivist' | 'consolidator' | 'collapse'`, `NOT NULL DEFAULT 'chat'` (migración). Las filas existentes quedan como `'chat'`.
- `StatsRepo::record_request` recibe un `kind` (`CallKind`); cada punto de llamada pasa su origen.
- Las agregaciones de chat (`summary`, `by_model`, `by_day`, `tools_summary`) filtran `kind='chat'`: el panel de chat cuenta solo turnos reales.
- Nuevo `StatsRepo::background_summary(pool)` + endpoint `GET /api/stats/llm/background`, con los totales por origen no-chat.
- Nueva tarjeta «Procesos de fondo» en **Estadísticas → Modelos**: una fila por origen (router, archivist, consolidator, collapse) con llamadas, tokens, coste, latencia y errores.
- El export CSV añade la columna `kind`.
- `last_api_call` sigue describiendo solo el chat.

## Capabilities

### Modified Capabilities
- `db/schema`: `llm_requests` gana la columna `kind`.
- `db/repos`: las agregaciones de chat filtran `kind='chat'`; `record_request` recibe el origen; nueva agregación por origen.
- `orchestrator/skill-router`: la decisión expone su telemetría; las estadísticas de chat no se alteran.
- `orchestrator/agent`: el orquestador persiste la llamada al clasificador con `kind='router'`.
- `workers`: collapse, archivist y consolidator registran su llamada con su propio `kind`.
- `frontend`: nuevo panel «Procesos de fondo».

## Impact

- Nueva migración `migrations/20261009000001_llm_requests_kind.sql`.
- `src/models/stats.rs` (`CallKind`, `BackgroundStats`), `src/db/repos/stats.rs`, `src/handlers/stats.rs`, `src/routes/stats.rs`.
- `src/orchestrator/skill_router.rs`, `src/orchestrator/agent.rs`, `src/workers/collapse.rs`, `src/workers/episodic_memory.rs`.
- `frontend/src/types/index.ts`, `frontend/src/api/client.ts`, `frontend/src/components/stats/BackgroundCard.tsx`, `frontend/src/pages/StatsDashboard.tsx`.
- Sin cambios en `docker-compose*.yml`, contrato SSE ni OIDC.
